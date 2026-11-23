package com.owlmic.media.camera

import android.media.MediaCodec
import android.media.MediaCodecInfo
import android.media.MediaCodecInfo.CodecProfileLevel
import android.media.MediaFormat
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.HandlerThread
import android.view.Surface

/**
 * Hardware H.264 with the settings of section 17.2: High profile when the encoder has it, else Constrained Baseline; no
 * B-frames; CBR (VBR when CBR is missing); a keyframe every 2 s and on request; real-time priority; one frame of
 * latency; the last frame repeated after 100 ms of a still scene, so the stream never stops. Encoded frames reach
 * [onFrame] on the encoder's own thread, as Annex B with SPS and PPS before every keyframe.
 */
class VideoEncoder(
    val width: Int,
    val height: Int,
    fps: Int,
    bitrate: Int,
    private val onFrame: (ptsUs: Long, keyframe: Boolean, data: ByteArray, length: Int) -> Unit,
    private val onError: () -> Unit,
) {
    private val thread = HandlerThread("owlmic-encoder").apply { start() }
    private val handler = Handler(thread.looper)
    private val codec = MediaCodec.createEncoderByType(MIME)
    private var config = ByteArray(0)
    private var buffer = ByteArray(256 * 1024)

    @Volatile private var released = false

    val inputSurface: Surface

    private val callback = object : MediaCodec.Callback() {
        override fun onInputBufferAvailable(codec: MediaCodec, index: Int) = Unit

        override fun onOutputBufferAvailable(codec: MediaCodec, index: Int, info: MediaCodec.BufferInfo) {
            if (released) return
            try {
                val out = codec.getOutputBuffer(index)
                if (out != null && info.size > 0) {
                    out.position(info.offset)
                    out.limit(info.offset + info.size)
                    val key = info.flags and MediaCodec.BUFFER_FLAG_KEY_FRAME != 0
                    if (info.flags and MediaCodec.BUFFER_FLAG_CODEC_CONFIG != 0 && !key) {
                        config = ByteArray(info.size).also { out.get(it) }
                    } else {
                        val prefix = if (key && !startsWithSps(out, info.offset)) config else ByteArray(0)
                        val length = prefix.size + info.size
                        if (buffer.size < length) buffer = ByteArray(length * 2)
                        System.arraycopy(prefix, 0, buffer, 0, prefix.size)
                        out.get(buffer, prefix.size, info.size)
                        onFrame(info.presentationTimeUs, key, buffer, length)
                    }
                }
                codec.releaseOutputBuffer(index, false)
            } catch (_: IllegalStateException) {
                // Released meanwhile.
            }
        }

        override fun onError(codec: MediaCodec, e: MediaCodec.CodecException) = this@VideoEncoder.onError()

        override fun onOutputFormatChanged(codec: MediaCodec, format: MediaFormat) = Unit
    }

    init {
        configure(fps, bitrate)
        inputSurface = codec.createInputSurface()
        codec.start()
    }

    private fun configure(fps: Int, bitrate: Int) {
        val caps = codec.codecInfo.getCapabilitiesForType(MIME)
        val cbr = caps.encoderCapabilities?.isBitrateModeSupported(MediaCodecInfo.EncoderCapabilities.BITRATE_MODE_CBR) == true
        val high = caps.profileLevels.filter { it.profile == CodecProfileLevel.AVCProfileHigh }.maxByOrNull { it.level }
        val constrained = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O_MR1) {
            caps.profileLevels.filter { it.profile == CodecProfileLevel.AVCProfileConstrainedBaseline }.maxByOrNull { it.level }
        } else {
            null
        }
        val profile = high ?: constrained
        // Some encoders refuse an explicit profile they list, or headers on sync frames (we prepend those ourselves
        // anyway); each refusal falls back to the encoder's own default for that key.
        val attempts = listOf(profile to true, null to true, profile to false, null to false).distinct()
        var last: Exception? = null
        for ((p, headers) in attempts) {
            try {
                // reset() drops the callback, so each attempt sets it again.
                codec.setCallback(callback, handler)
                codec.configure(format(fps, bitrate, cbr, p, headers), null, null, MediaCodec.CONFIGURE_FLAG_ENCODE)
                return
            } catch (e: Exception) {
                last = e
                codec.reset()
            }
        }
        throw last ?: IllegalStateException("can't configure the encoder")
    }

    private fun format(fps: Int, bitrate: Int, cbr: Boolean, profile: CodecProfileLevel?, headers: Boolean) = MediaFormat.createVideoFormat(MIME, width, height).apply {
        setInteger(MediaFormat.KEY_COLOR_FORMAT, MediaCodecInfo.CodecCapabilities.COLOR_FormatSurface)
        setInteger(MediaFormat.KEY_BIT_RATE, bitrate)
        setInteger(MediaFormat.KEY_FRAME_RATE, fps)
        setInteger(MediaFormat.KEY_I_FRAME_INTERVAL, KEYFRAME_EVERY_S)
        setInteger(
            MediaFormat.KEY_BITRATE_MODE,
            if (cbr) MediaCodecInfo.EncoderCapabilities.BITRATE_MODE_CBR else MediaCodecInfo.EncoderCapabilities.BITRATE_MODE_VBR,
        )
        if (profile != null) {
            setInteger(MediaFormat.KEY_PROFILE, profile.profile)
            setInteger(MediaFormat.KEY_LEVEL, profile.level)
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            setInteger(MediaFormat.KEY_MAX_B_FRAMES, 0)
            if (headers) setInteger(MediaFormat.KEY_PREPEND_HEADER_TO_SYNC_FRAMES, 1)
        }
        setInteger(MediaFormat.KEY_PRIORITY, 0)
        setInteger(MediaFormat.KEY_LATENCY, 1)
        setLong(MediaFormat.KEY_REPEAT_PREVIOUS_FRAME_AFTER, REPEAT_AFTER_US)
    }

    fun requestKeyframe() = parameter(MediaCodec.PARAMETER_KEY_REQUEST_SYNC_FRAME, 0)

    fun setBitrate(bps: Int) = parameter(MediaCodec.PARAMETER_KEY_VIDEO_BITRATE, bps)

    private fun parameter(key: String, value: Int) {
        if (released) return
        runCatching { codec.setParameters(Bundle().apply { putInt(key, value) }) }
    }

    fun release() {
        released = true
        runCatching { codec.stop() }
        runCatching { codec.release() }
        inputSurface.release()
        thread.quitSafely()
    }

    private companion object {
        const val MIME = MediaFormat.MIMETYPE_VIDEO_AVC
        const val KEYFRAME_EVERY_S = 1
        const val REPEAT_AFTER_US = 100_000L

        /** True when the access unit opens with a sequence parameter set (NAL type 7) after its start code. */
        fun startsWithSps(b: java.nio.ByteBuffer, at: Int): Boolean {
            val n = b.limit() - at
            val skip = when {
                n > 4 && b.get(at).toInt() == 0 && b.get(at + 1).toInt() == 0 && b.get(at + 2).toInt() == 0 && b.get(at + 3).toInt() == 1 -> 4
                n > 3 && b.get(at).toInt() == 0 && b.get(at + 1).toInt() == 0 && b.get(at + 2).toInt() == 1 -> 3
                else -> return false
            }
            return b.get(at + skip).toInt() and 0x1F == 7
        }
    }
}
