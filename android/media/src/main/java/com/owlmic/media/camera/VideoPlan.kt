package com.owlmic.media.camera

import com.owlmic.core.hub.LinkKind
import kotlin.math.roundToInt

/** Size (as the 16:9 frame), frame rate and starting bitrate for the camera stream (section 17.2). */
data class VideoPlan(val longSide: Int, val shortSide: Int, val fps: Int, val bitrate: Int) {
    companion object {
        /**
         * [quality] and [fps] are the settings' values. "Auto" means 1080p on a cable and 720p on Wi-Fi. Starting
         * bitrates are the design's table; the 720p 60 and 1080p 24 rows, which it doesn't list, sit between their neighbours.
         */
        fun of(quality: String, fps: Int, link: LinkKind): VideoPlan {
            val cable = link.isCable
            val p1080 = quality == "1080p" || (quality == "auto" && cable)
            val f = fps.coerceIn(15, 60)
            val (wifi, wired) = when {
                p1080 && f >= 60 -> 10_000 to 24_000
                p1080 && f >= 30 -> 6_000 to 16_000
                p1080 -> 5_000 to 13_000
                f >= 60 -> 6_000 to 14_000
                f >= 30 -> 4_000 to 10_000
                else -> 3_000 to 8_000
            }
            val kbps = if (cable) wired else wifi
            return if (p1080) VideoPlan(1920, 1080, f, kbps * 1_000) else VideoPlan(1280, 720, f, kbps * 1_000)
        }
    }
}

/**
 * The picture's shape. [FULL], the default, is everything the sensor captures at its own aspect, upright for whoever
 * holds the phone (4:3 sideways, 3:4 upright on most phones): nothing is cut. The others are exactly what the PC
 * receives, [wide]:[tall], however the phone is held, and cut from the camera's frame only because the user picked them.
 */
enum class Ratio(val value: String, val wide: Int, val tall: Int) {
    FULL("full", 0, 0),
    WIDE("16:9", 16, 9),
    CLASSIC("4:3", 4, 3),
    CINEMA("2:1", 2, 1),
    SQUARE("1:1", 1, 1),
    PORTRAIT_CINEMA("1:2", 1, 2),
    PORTRAIT_CLASSIC("3:4", 3, 4),
    TALL("9:16", 9, 16),
    ;

    companion object {
        fun of(value: String?) = entries.firstOrNull { it.value == value } ?: FULL
    }
}

/**
 * Which of the sensor's frames the camera is bound for: its 4:3 one, or its 16:9 one. [FULL] takes the sensor's own
 * ([sensorWide] for the rare sensor whose whole area is 16:9); a preset takes the frame it loses the least of.
 */
fun boundFourByThree(ratio: Ratio, sensorWide: Boolean): Boolean = when (ratio) {
    Ratio.FULL -> !sensorWide
    Ratio.CLASSIC, Ratio.PORTRAIT_CLASSIC, Ratio.SQUARE -> true
    Ratio.WIDE, Ratio.TALL, Ratio.CINEMA, Ratio.PORTRAIT_CINEMA -> false
}

/**
 * The encoded width and height of [plan] in [ratio]: the plan's short side stays, the other follows the ratio (or
 * [plan.longSide] caps 2:1 / 1:2 so encoders stay within 1080p/720p bounds). For [Ratio.FULL] it follows [frame], the
 * camera's upright picture (width to height), and is null until that is known.
 */
fun outputSize(plan: VideoPlan, ratio: Ratio, frame: Pair<Int, Int>? = null): Pair<Int, Int>? = when (ratio) {
    Ratio.FULL -> frame?.takeIf { it.first > 0 && it.second > 0 }?.let { fullSize(plan.shortSide, it) }
    Ratio.WIDE -> plan.longSide to plan.shortSide
    Ratio.TALL -> plan.shortSide to plan.longSide
    Ratio.CINEMA -> plan.longSide to plan.longSide / 2
    Ratio.PORTRAIT_CINEMA -> plan.longSide / 2 to plan.longSide
    Ratio.CLASSIC -> plan.shortSide * 4 / 3 to plan.shortSide
    Ratio.PORTRAIT_CLASSIC -> plan.shortSide to plan.shortSide * 4 / 3
    Ratio.SQUARE -> plan.shortSide to plan.shortSide
}

/**
 * The whole [frame] (upright width to height) at [short] pixels on its short side. The long side is rounded to a
 * multiple of 16, as encoders like it: at most 8 pixels off the exact shape, too little to see.
 */
fun fullSize(short: Int, frame: Pair<Int, Int>): Pair<Int, Int> {
    val (w, h) = frame
    val long = (short.toDouble() * maxOf(w, h) / minOf(w, h) / 16).roundToInt().coerceAtLeast(1) * 16
    return if (w >= h) long to short else short to long
}

/** The starting bitrate for a [width]×[height] picture: the plan's, in proportion to the pixels it carries. */
fun bitrateFor(plan: VideoPlan, width: Int, height: Int): Int =
    (plan.bitrate.toLong() * width * height / (plan.longSide.toLong() * plan.shortSide)).toInt()

/** The size the camera is bound for, in the sensor's own landscape terms: CameraX compares it with sizes as the sensor gives them. */
fun boundSize(plan: VideoPlan, fourByThree: Boolean): Pair<Int, Int> =
    if (fourByThree) plan.shortSide * 4 / 3 to plan.shortSide else plan.longSide to plan.shortSide

/** Whether a sensor's active area of [width]×[height] is 16:9 rather than 4:3 (or close to them). */
fun sensorIsWide(width: Int, height: Int): Boolean =
    width > 0 && height > 0 && maxOf(width, height).toDouble() / minOf(width, height) > WIDE_SENSOR

/** Halfway between 4:3 (1.33) and 16:9 (1.78). */
private const val WIDE_SENSOR = 1.55

/**
 * Bitrate adaptation (section 17.2): loss above 2% or a rising round trip lowers the bitrate by 20%, at most every 2 s;
 * 5 clean seconds raise it by 10%, never above where it started. Pure, so it is tested without a network.
 */
class BitrateController(val start: Int, private val now: () -> Long = System::currentTimeMillis) {
    var current = start
        private set

    private var lastChange = 0L
    private var cleanSince = now()
    private var baselineRtt = 0

    /** Returns the new bitrate when it changes, else null. */
    fun report(lossPct: Double, rttMs: Int): Int? {
        val t = now()
        if (baselineRtt == 0 || rttMs in 1 until baselineRtt) baselineRtt = rttMs
        val rising = baselineRtt > 0 && rttMs > baselineRtt * 3 / 2 && rttMs > baselineRtt + 30
        if (lossPct > 2.0 || rising) {
            cleanSince = t
            if (t - lastChange < DOWN_EVERY_MS) return null
            return change(maxOf(start / 4, current * 4 / 5), t)
        }
        if (t - cleanSince >= CLEAN_FOR_MS && current < start) {
            cleanSince = t
            return change(minOf(start, current * 11 / 10), t)
        }
        return null
    }

    /** A picture too big to send: halve the bitrate at once, never below a quarter of the start. */
    fun refused(): Int? = change(maxOf(start / 4, current / 2), now()).also { cleanSince = now() }

    private fun change(to: Int, t: Long): Int? {
        if (to == current) return null
        current = to
        lastChange = t
        return to
    }

    private companion object {
        const val DOWN_EVERY_MS = 2_000L
        const val CLEAN_FOR_MS = 5_000L
    }
}

/** Frame rate under heat (section 17.2): severe caps at 24, critical and worse at 15. Android's thermal status values. */
fun thermalFps(wanted: Int, thermalStatus: Int): Int = when {
    thermalStatus >= THERMAL_CRITICAL -> minOf(wanted, 15)
    thermalStatus >= THERMAL_SEVERE -> minOf(wanted, 24)
    else -> wanted
}

/** Quality under heat (section 19, "lower fps, then quality"): at critical, 1080p drops to 720p and its bitrate with it. */
fun thermalPlan(plan: VideoPlan, thermalStatus: Int): VideoPlan =
    if (thermalStatus >= THERMAL_CRITICAL && plan.shortSide > 720) plan.copy(longSide = 1280, shortSide = 720, bitrate = plan.bitrate * 2 / 3) else plan

private const val THERMAL_SEVERE = 3
private const val THERMAL_CRITICAL = 4

/**
 * Lets camera frames through to the encoder at [FrameGate.pass]'s fps on average: a frame goes when it is due, and the
 * next is due one period later, so a 30 fps camera feeding a 24 fps stream drops one frame in five. A frame a little
 * early (up to a quarter period) still goes, which absorbs the camera's timing jitter. GL thread only.
 */
class FrameGate {
    private var due = Long.MIN_VALUE

    fun pass(timestampNs: Long, fps: Int): Boolean {
        val period = 1_000_000_000L / fps.coerceAtLeast(1)
        // The first frame, or one after a gap (a pause, a stalled camera): start counting again from here.
        if (due == Long.MIN_VALUE || timestampNs - due > period) {
            due = timestampNs + period
            return true
        }
        if (timestampNs < due - period / 4) return false
        due += period
        return true
    }
}

/** How much of an upright w×h picture to keep, centred, to fill an output of aspect ow:oh. Returns (x scale, y scale), each ≤ 1. */
fun centerCrop(w: Int, h: Int, ow: Int, oh: Int): Pair<Float, Float> {
    val src = w.toFloat() / h
    val dst = ow.toFloat() / oh
    return if (src > dst) (dst / src) to 1f else 1f to (src / dst)
}

/**
 * What the preview's backdrop shows of an upright [w]×[h] picture: the part that is sent (centred, [ow]:[oh]), and of
 * that, the centre that covers a [cw]:[ch] area. A blurred duplicate of the shown picture, never what was cut from it.
 * Returns (x scale, y scale) of the whole picture, each ≤ 1.
 */
fun coverCrop(w: Int, h: Int, ow: Int, oh: Int, cw: Int, ch: Int): Pair<Float, Float> {
    val (sx, sy) = centerCrop(w, h, ow, oh)
    val (cx, cy) = centerCrop(ow, oh, cw, ch)
    return sx * cx to sy * cy
}
