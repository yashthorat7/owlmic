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
            val p480 = quality == "480p"
            val p360 = quality == "360p"
            val f = fps.coerceIn(24, 60)
            val (wifi, wired) = when {
                p1080 && f >= 60 -> 10_000 to 24_000
                p1080 && f >= 30 -> 6_000 to 16_000
                p1080 -> 5_000 to 13_000
                p480 && f >= 60 -> 2_600 to 5_500
                p480 && f >= 30 -> 1_600 to 3_600
                p480 -> 1_200 to 2_800
                p360 && f >= 60 -> 1_600 to 3_200
                p360 && f >= 30 -> 1_000 to 2_200
                p360 -> 750 to 1_600
                f >= 60 -> 6_000 to 14_000
                f >= 30 -> 4_000 to 10_000
                else -> 3_000 to 8_000
            }
            val kbps = if (cable) wired else wifi
            return when {
                p1080 -> VideoPlan(1920, 1080, f, kbps * 1_000)
                p480 -> VideoPlan(848, 480, f, kbps * 1_000)
                p360 -> VideoPlan(640, 360, f, kbps * 1_000)
                else -> VideoPlan(1280, 720, f, kbps * 1_000)
            }
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

/**
 * Picks the hardware AE FPS range for [targetFps] without letting auto-exposure drop below 24 fps
 * (e.g. `[30, 30]` / `[24, 60]` always beats `[15, 60]`).
 */
fun selectFpsRange(ranges: List<Pair<Int, Int>>, targetFps: Int): Pair<Int, Int>? {
    if (ranges.isEmpty()) return null
    val norm = ranges.map { (lo, hi) ->
        if (hi > 1000) (lo / 1000) to (hi / 1000) else lo to hi
    }
    val req = targetFps.coerceIn(24, 60)
    val bestIdx = norm.indices.minWithOrNull(
        compareBy<Int>(
            { if (norm[it].second >= 24) 0 else 1 },
            { if (norm[it].first >= 24) 0 else 1 },
            { if (norm[it].second >= req) 0 else 1 },
            { kotlin.math.abs(norm[it].second - req) },
            { -norm[it].first },
        ),
    ) ?: return null
    return ranges[bestIdx]
}

data class StreamAdjustment(
    val plan: VideoPlan,
    val resolutionChanged: Boolean,
    val fpsChanged: Boolean,
    val bitrateChanged: Boolean,
)

/**
 * Adaptive stream controller maintaining >= 24 FPS smoothness as top priority:
 * 1. When 60 fps is saturated, steps FPS 60 -> 30 first to preserve crisp resolution.
 * 2. Trims bitrate within a resolution tier before stepping resolution down (1080p -> 720p -> 480p).
 * 3. Never drops below 24 FPS under network or encoder pressure.
 * 4. Smoothly climbs bitrate, resolution (up to ceiling / 1080p), and FPS back up when headroom holds.
 */
class StreamOptimizer(
    val quality: String,
    targetFps: Int,
    val link: LinkKind,
    private val now: () -> Long = System::currentTimeMillis,
) {
    private val requestedFps = targetFps.coerceIn(24, 60)
    private var thermalStatus = 0

    // Ideal is 720p unless the user explicitly requested 1080p.
    private val startTier: Int = when (quality) {
        "360p" -> TIER_360
        "480p" -> TIER_480
        "1080p" -> TIER_1080
        else -> TIER_720
    }
    private val maxTierCeiling: Int = startTier

    private var tier: Int = startTier
    private var fps: Int = requestedFps
    private var bitratePct: Int = if (link.isCable) 100 else 75
    private var baselineRtt = 0
    private var lastChangeAt = 0L
    private var lastPromoteAt = 0L
    private var promoteCooldownUntil = 0L
    private var cleanSince = now()

    val current: VideoPlan get() = buildPlan(tier, fps, bitratePct)

    fun setThermal(status: Int): StreamAdjustment? {
        if (status == thermalStatus) return null
        thermalStatus = status
        return enforceBounds()
    }

    fun report(
        lossPct: Double,
        rttMs: Int,
        camLossPct: Double? = null,
        camFps: Int? = null,
        camDropPct: Double? = null,
        measuredOutFps: Int? = null,
        congested: Boolean = false,
    ): StreamAdjustment? {
        val t = now()
        if (baselineRtt == 0 || rttMs in 1 until baselineRtt) baselineRtt = rttMs
        val effectiveLoss = maxOf(lossPct, camLossPct ?: 0.0)
        val risingRtt = baselineRtt > 0 && rttMs > baselineRtt * 3 / 2 && rttMs > baselineRtt + 30
        val dropHigh = (camDropPct ?: 0.0) >= 3.5
        val pcSlow = camFps != null && fps >= 24 && camFps in 1 until (fps * 78 / 100)
        val captureSlow = measuredOutFps != null && measuredOutFps in 1 until (fps * 78 / 100)
        val stressed = effectiveLoss > 2.0 || dropHigh || risingRtt || congested || pcSlow || captureSlow

        if (stressed) {
            cleanSince = t
            if (lastPromoteAt > 0 && t - lastPromoteAt < PROMOTE_BACKOFF_WINDOW_MS) {
                promoteCooldownUntil = t + PROMOTE_COOLDOWN_MS
            }
            if (t - lastChangeAt < DOWN_EVERY_MS) return null
            val prevPlan = current
            val hard = effectiveLoss > 8.0 || (camDropPct ?: 0.0) > 10.0 || pcSlow || captureSlow || congested
            stepDown(hard)
            return diff(prevPlan, t)
        }

        val veryClean = effectiveLoss < 1.0 && (camDropPct ?: 0.0) < 1.5 &&
            (camFps == null || camFps >= fps * 88 / 100) &&
            (measuredOutFps == null || measuredOutFps >= fps * 88 / 100)

        val holdMs = if (tier < effectiveMaxTier() && bitratePct >= 100) UP_RES_EVERY_MS else UP_EVERY_MS
        if (veryClean && t - cleanSince >= holdMs) {
            if (t < promoteCooldownUntil && tier < effectiveMaxTier() && bitratePct >= 100) {
                return null
            }
            val prevPlan = current
            val upgraded = stepUp(t)
            if (upgraded) {
                cleanSince = t
                return diff(prevPlan, t)
            }
        }
        return null
    }

    fun onCongested(): StreamAdjustment? {
        val t = now()
        cleanSince = t
        if (t - lastChangeAt < DOWN_EVERY_MS) return null
        val prev = current
        stepDown(hard = true)
        return diff(prev, t)
    }

    fun refused(): StreamAdjustment? {
        val t = now()
        cleanSince = t
        val prev = current
        bitratePct = maxOf(MIN_BITRATE_PCT, bitratePct * 3 / 4)
        if (bitratePct <= MIN_BITRATE_PCT && tier > TIER_360) {
            tier -= 1
            bitratePct = 85
        }
        return diff(prev, t)
    }

    private fun enforceBounds(): StreamAdjustment? {
        val prev = current
        val maxF = effectiveMaxFps()
        val maxT = effectiveMaxTier()
        var changed = false
        if (fps > maxF) {
            fps = maxF
            changed = true
        }
        if (tier > maxT) {
            tier = maxT
            changed = true
        }
        return if (changed) diff(prev, now()) else null
    }

    private fun stepDown(hard: Boolean) {
        // 1. High frame rate (>30, e.g. 60 fps): step to 30 fps first to keep current resolution smooth.
        if (fps > 30) {
            fps = 30
            return
        }
        // 2. Hard decode/throughput shortfall: drop resolution tier immediately (720p -> 480p -> 360p) before going < 24 fps.
        if (hard && tier > TIER_360) {
            tier -= 1
            bitratePct = 85
            return
        }
        // 3. Moderate pressure: trim bitrate cleanly without restarting encoder.
        if (bitratePct > 60) {
            bitratePct = maxOf(60, bitratePct - 20)
            return
        }
        // 4. Lower resolution before touching the 24 fps floor.
        if (tier > TIER_360) {
            tier -= 1
            bitratePct = 85
            return
        }
        // 5. Lowest resolution: can drop 30 -> 24, never below 24 fps, then trim bitrate to floor.
        if (fps > 24) {
            fps = 24
            return
        }
        bitratePct = maxOf(MIN_BITRATE_PCT, bitratePct - 15)
    }

    private fun stepUp(t: Long): Boolean {
        if (bitratePct < 100) {
            bitratePct = minOf(100, bitratePct + 15)
            return true
        }
        val maxT = effectiveMaxTier()
        if (tier < maxT && t >= promoteCooldownUntil) {
            tier += 1
            bitratePct = 85
            lastPromoteAt = t
            return true
        }
        val maxF = effectiveMaxFps()
        if (fps < maxF) {
            fps = if (fps < 30 && maxF >= 30) 30 else maxF
            lastPromoteAt = t
            return true
        }
        return false
    }

    private fun effectiveMaxFps(): Int =
        if (thermalStatus >= THERMAL_SEVERE) minOf(requestedFps, 24) else requestedFps

    private fun effectiveMaxTier(): Int =
        if (thermalStatus >= THERMAL_CRITICAL) minOf(maxTierCeiling, TIER_480) else maxTierCeiling

    private fun buildPlan(t: Int, f: Int, pct: Int): VideoPlan {
        val base = VideoPlan.of(tierName(t), f, link)
        val bps = (base.bitrate.toLong() * pct / 100).toInt().coerceAtLeast(350_000)
        return base.copy(bitrate = bps)
    }

    private fun diff(prev: VideoPlan, t: Long): StreamAdjustment? {
        val next = current
        if (next == prev) return null
        lastChangeAt = t
        return StreamAdjustment(
            plan = next,
            resolutionChanged = next.longSide != prev.longSide || next.shortSide != prev.shortSide,
            fpsChanged = next.fps != prev.fps,
            bitrateChanged = next.bitrate != prev.bitrate,
        )
    }

    companion object {
        const val TIER_360 = 0
        const val TIER_480 = 1
        const val TIER_720 = 2
        const val TIER_1080 = 3
        private const val MIN_BITRATE_PCT = 35
        private const val DOWN_EVERY_MS = 2_000L
        private const val UP_EVERY_MS = 6_000L
        private const val UP_RES_EVERY_MS = 8_000L
        private const val PROMOTE_BACKOFF_WINDOW_MS = 10_000L
        private const val PROMOTE_COOLDOWN_MS = 20_000L

        private fun tierName(t: Int) = when (t) {
            TIER_1080 -> "1080p"
            TIER_720 -> "720p"
            TIER_480 -> "480p"
            else -> "360p"
        }
    }
}
