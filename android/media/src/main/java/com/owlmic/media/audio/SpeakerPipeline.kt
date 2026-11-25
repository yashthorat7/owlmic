package com.owlmic.media.audio

import android.os.Process
import com.owlmic.core.hub.Health
import com.owlmic.core.link.SpeakerSink
import com.owlmic.media.codec.OpusDecoder
import kotlin.concurrent.thread
import kotlin.math.floor
import kotlin.math.roundToInt

/** What the PC said in STREAM_START for stream 3. */
data class SpeakerFormat(val codec: String, val channels: Int, val frameMs: Int)

/**
 * Smoothly adapts speaker playback speed by up to ±0.4% to track the incoming buffer target across
 * independent PC/phone sample clocks without audible clicks or frame drops, and handles seamless
 * fade-in / fade-out across underrun boundaries.
 */
internal class StereoDriftResampler(private val channels: Int) {
    private val prev = ShortArray(channels)
    private var phase = 0.0
    private var depthAvgMs = 0f
    private var fadeInLeft = 0
    var fadedOut = true
        private set

    fun reset() {
        prev.fill(0)
        phase = 0.0
        depthAvgMs = 0f
        fadeInLeft = 0
        fadedOut = true
    }

    fun ensurePlaying() {
        if (fadedOut) {
            fadedOut = false
            fadeInLeft = FADE_FRAMES
            depthAvgMs = 0f
        }
    }

    fun stepFor(bufferedMs: Float, targetMs: Float, frameMs: Int): Double {
        if (depthAvgMs <= 0f) depthAvgMs = bufferedMs
        val alpha = (frameMs / DEPTH_AVG_MS).coerceIn(0.01f, 0.25f)
        depthAvgMs += (bufferedMs - depthAvgMs) * alpha
        val error = (depthAvgMs - targetMs) / maxOf(targetMs, 10f)
        val adjust = (error * DRIFT_GAIN).coerceIn(-MAX_DRIFT, MAX_DRIFT)
        return 1.0 + adjust
    }

    fun resample(input: ShortArray, inputFrames: Int, out: ShortArray, step: Double): Int {
        if (inputFrames <= 0) return 0
        var written = 0
        val maxOut = out.size / channels
        while (phase < inputFrames && written < maxOut) {
            val pos = phase - 1.0
            val i = floor(pos).toInt()
            val t = (pos - i).toFloat()
            val iNext = (i + 1).coerceIn(0, inputFrames - 1)
            val fadeGain = if (fadeInLeft > 0) 1f - fadeInLeft.toFloat() / FADE_FRAMES else 1f
            for (c in 0 until channels) {
                val a = if (i < 0) prev[c].toFloat() else input[i * channels + c].toFloat()
                val b = input[iNext * channels + c].toFloat()
                val sample = (a + (b - a) * t) * fadeGain
                out[written * channels + c] = sample.roundToInt().coerceIn(-32768, 32767).toShort()
            }
            if (fadeInLeft > 0) fadeInLeft--
            written++
            phase += step
        }
        for (c in 0 until channels) prev[c] = input[(inputFrames - 1) * channels + c]
        phase -= inputFrames
        return written
    }

    /** Produces a smooth 2.5 ms tail from the last output sample down to silence so pauses never click. */
    fun fadeOutTail(out: ShortArray): Int {
        if (fadedOut) return 0
        fadedOut = true
        phase = 0.0
        val n = minOf(FADE_FRAMES, out.size / channels)
        for (i in 0 until n) {
            val g = (n - 1 - i).toFloat() / n
            for (c in 0 until channels) {
                out[i * channels + c] = (prev[c] * g).roundToInt().toShort()
            }
        }
        prev.fill(0)
        return n
    }

    private companion object {
        const val FADE_FRAMES = 120
        const val DEPTH_AVG_MS = 500f
        const val DRIFT_GAIN = 0.004f
        const val MAX_DRIFT = 0.004f
    }
}

/**
 * The PC's sound on the phone (section 17.3): packets go into the [Smoother] on the carrier's thread; a playback thread
 * pulls one frame at a time, decodes or conceals it, drift-resamples to keep buffer depth locked without drops, and
 * feeds Oboe's real-time output ring. Usage Media for full quality, Voice Communication while voice isolation needs
 * Android's echo canceller to hear what plays. Each frame's level goes to [farEnd], for the mic's echo suppressor.
 */
class SpeakerPipeline(private val onHealth: (Health) -> Unit, private val farEnd: FarEnd = FarEnd()) : SpeakerSink {
    @Volatile private var smoother: Smoother? = null

    @Volatile private var format: SpeakerFormat? = null

    @Volatile private var voiceCommunication = false

    @Volatile private var wireless = true

    @Volatile private var reopen = false

    /** Bumped by every start and stop: a playback thread runs only while its own generation is current. */
    @Volatile private var generation = 0
    private var running = false
    private var worker: Thread? = null

    fun start(format: SpeakerFormat, voiceCommunication: Boolean, wireless: Boolean) {
        this.voiceCommunication = voiceCommunication
        this.wireless = wireless
        configure(format)
        if (running) return
        running = true
        val mine = ++generation
        val previous = worker
        worker = thread(name = "owlmic-speaker", isDaemon = true) {
            // The previous thread closes its output first; the caller (the hub thread) never waits for it.
            previous?.join(STOP_WAIT_MS)
            loop(mine)
        }
    }

    /** A new STREAM_START (new stream or codec after a link switch). */
    fun configure(format: SpeakerFormat) {
        this.format = format
        val prev = smoother
        smoother = if (wireless) Smoother(format.frameMs, 20, 80) else Smoother(format.frameMs, 20, 40)
        prev?.wakeup()
        reopen = true
    }

    fun setVoiceCommunication(on: Boolean) {
        if (on != voiceCommunication) {
            voiceCommunication = on
            reopen = true
        }
    }

    fun setWireless(on: Boolean): Boolean {
        if (on == wireless) return false
        wireless = on
        format?.let(::configure)
        return true
    }

    override fun packet(seq: Long, timestampUs: Long, data: ByteArray) {
        smoother?.push(seq, data)
    }

    /** Returns at once; the playback thread wakes and closes its output. */
    fun stop() {
        running = false
        generation++
        val prev = smoother
        smoother = null
        prev?.wakeup()
    }

    private fun loop(mine: Int) {
        Process.setThreadPriority(Process.THREAD_PRIORITY_URGENT_AUDIO)
        var handle = 0L
        var decoder: OpusDecoder? = null
        var current: SpeakerFormat? = null
        var pcm = ShortArray(0)
        var resampled = ShortArray(0)
        var resampler = StereoDriftResampler(2)
        var healthy: Boolean? = null
        var errors = 0
        var lowWaterFrames = 240
        while (generation == mine) {
            val f = format ?: break
            if (reopen || handle == 0L || f != current) {
                reopen = false
                if (handle != 0L) Oboe.close(handle)
                decoder?.close()
                current = f
                handle = Oboe.openOutput(
                    f.channels,
                    if (voiceCommunication) Oboe.USAGE_VOICE_COMMUNICATION else Oboe.USAGE_MEDIA,
                    if (voiceCommunication) Oboe.CONTENT_SPEECH else Oboe.CONTENT_MUSIC,
                    lowLatency = !voiceCommunication,
                )
                decoder = if (f.codec == "opus") OpusDecoder(f.channels) else null
                val frameSamples = 48 * f.frameMs
                pcm = ShortArray(frameSamples * f.channels)
                resampled = ShortArray((frameSamples + RESAMPLE_HEADROOM) * f.channels)
                resampler = StereoDriftResampler(f.channels)
                if (handle == 0L) {
                    if (healthy != false) onHealth(Health.Degraded("speaker restarting")).also { healthy = false }
                    Thread.sleep(300)
                    continue
                }
                lowWaterFrames = maxOf(frameSamples / 2, Oboe.burstFrames(handle) / 2)
            }
            val frameSamples = 48 * f.frameMs
            val s = smoother ?: break
            // Let Oboe's ring drain toward its burst floor before treating an empty jitter buffer as a lost packet.
            if (s.buffered() == 0 && Oboe.queuedFrames(handle) > lowWaterFrames) {
                s.awaitData(AWAIT_DATA_MS)
                continue
            }
            val outFrames: Int
            when (val p = s.pull()) {
                is Smoother.Pull.Frame -> {
                    resampler.ensurePlaying()
                    val ok = if (decoder == null) {
                        pcmFromBytes(p.data, pcm)
                        true
                    } else if (decoder.decode(p.data, pcm, frameSamples) > 0) {
                        errors = 0
                        true
                    } else {
                        conceal(decoder, null, pcm, frameSamples)
                        if (++errors >= MAX_DECODE_ERRORS) {
                            errors = 0
                            decoder.close()
                            decoder = OpusDecoder(f.channels)
                            if (healthy != false) onHealth(Health.Degraded("speaker decoder restarting")).also { healthy = false }
                        }
                        true
                    }
                    outFrames = if (ok) {
                        val step = resampler.stepFor(s.bufferedMs().toFloat(), s.targetMs.toFloat(), f.frameMs)
                        resampler.resample(pcm, frameSamples, resampled, step)
                    } else 0
                }
                is Smoother.Pull.Lost -> {
                    if (decoder != null) {
                        resampler.ensurePlaying()
                        conceal(decoder, p.next, pcm, frameSamples)
                        outFrames = resampler.resample(pcm, frameSamples, resampled, 1.0)
                    } else {
                        outFrames = resampler.fadeOutTail(resampled)
                    }
                }
                Smoother.Pull.Silence -> {
                    val tail = resampler.fadeOutTail(resampled)
                    if (tail > 0) {
                        val written = Oboe.write(handle, resampled, tail, WRITE_TIMEOUT_MS)
                        if (written < 0) {
                            reopen = true
                            if (healthy != false) onHealth(Health.Degraded("speaker restarting")).also { healthy = false }
                        }
                    }
                    if (healthy != true) {
                        onHealth(Health.Ok)
                        healthy = true
                    }
                    s.awaitReady(SILENCE_WAIT_MS)
                    continue
                }
            }
            if (outFrames <= 0) continue
            val written = Oboe.write(handle, resampled, outFrames, WRITE_TIMEOUT_MS)
            if (written > 0) farEnd.played(levelDb(resampled, written * f.channels))
            if (written < 0) {
                reopen = true
                if (healthy != false) onHealth(Health.Degraded("speaker restarting")).also { healthy = false }
            } else if (healthy != true) {
                onHealth(Health.Ok)
                healthy = true
            }
        }
        if (handle != 0L) Oboe.close(handle)
        decoder?.close()
    }

    private fun conceal(decoder: OpusDecoder, next: ByteArray?, pcm: ShortArray, frameSamples: Int) {
        if (decoder.conceal(next, pcm, frameSamples) == 0) pcm.fill(0)
    }

    private fun pcmFromBytes(data: ByteArray, out: ShortArray) {
        val n = minOf(out.size, data.size / 2)
        for (i in 0 until n) out[i] = ((data[2 * i].toInt() and 0xFF) or (data[2 * i + 1].toInt() shl 8)).toShort()
        if (n < out.size) out.fill(0, n, out.size)
    }

    private companion object {
        const val WRITE_TIMEOUT_MS = 100
        const val AWAIT_DATA_MS = 2L
        const val SILENCE_WAIT_MS = 10L
        const val STOP_WAIT_MS = 1_000L
        const val MAX_DECODE_ERRORS = 3
        const val RESAMPLE_HEADROOM = 16
    }
}
