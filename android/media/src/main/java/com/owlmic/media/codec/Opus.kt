package com.owlmic.media.codec

/** libopus encoder for 48 kHz PCM (cpp/opus_jni.c). One instance per stream, used from one thread. */
class OpusEncoder(channels: Int, application: Application, bitrate: Int, fec: Boolean) : AutoCloseable {
    enum class Application(val code: Int) {
        /** Speech, with in-band FEC: the mic. */
        VOIP(2048),

        /** Music and everything else: the speaker. */
        AUDIO(2049),
    }

    /** The last packet: valid for the length [encode] returned, until the next call. */
    val packet = ByteArray(MAX_PACKET)

    private var handle = create(SAMPLE_RATE, channels, application.code, bitrate, COMPLEXITY, fec)

    init {
        check(handle != 0L) { "Can't create the Opus encoder" }
    }

    /** [pcm] holds one frame, interleaved. Returns the packet length, or 0 if encoding failed. */
    fun encode(pcm: ShortArray, frameSamples: Int): Int = encode(handle, pcm, frameSamples, packet).coerceAtLeast(0)

    /** The loss the PC reports: Opus spends more on FEC as it rises. */
    fun setPacketLoss(percent: Int) = setPacketLoss(handle, percent.coerceIn(0, 100))

    override fun close() {
        if (handle != 0L) destroy(handle)
        handle = 0
    }

    private companion object {
        const val SAMPLE_RATE = 48_000
        const val COMPLEXITY = 8
        const val MAX_PACKET = 1275

        init {
            try {
                System.loadLibrary("oboe")
            } catch (_: Throwable) {
            }
            System.loadLibrary("owlmic")
        }

        @JvmStatic external fun create(sampleRate: Int, channels: Int, application: Int, bitrate: Int, complexity: Int, fec: Boolean): Long

        @JvmStatic external fun setPacketLoss(handle: Long, percent: Int)

        @JvmStatic external fun encode(handle: Long, pcm: ShortArray, frameSamples: Int, out: ByteArray): Int

        @JvmStatic external fun destroy(handle: Long)
    }
}

/** libopus decoder for the PC's speaker stream. */
class OpusDecoder(private val channels: Int) : AutoCloseable {
    private var handle = create(48_000, channels)

    init {
        check(handle != 0L) { "Can't create the Opus decoder" }
    }

    /** Decodes [data] into [out] (interleaved). Returns samples per channel, or 0 on error. */
    fun decode(data: ByteArray, out: ShortArray, frameSamples: Int): Int =
        decode(handle, channels, data, data.size, out, frameSamples, false).coerceAtLeast(0)

    /** A lost frame: recovered from the next packet's FEC when there is one, otherwise concealed. */
    fun conceal(next: ByteArray?, out: ShortArray, frameSamples: Int): Int =
        decode(handle, channels, next, next?.size ?: 0, out, frameSamples, next != null).coerceAtLeast(0)

    override fun close() {
        if (handle != 0L) destroy(handle)
        handle = 0
    }

    private companion object {
        init {
            try {
                System.loadLibrary("oboe")
            } catch (_: Throwable) {
            }
            System.loadLibrary("owlmic")
        }

        @JvmStatic external fun create(sampleRate: Int, channels: Int): Long

        @JvmStatic external fun decode(handle: Long, channels: Int, data: ByteArray?, length: Int, out: ShortArray, frameSamples: Int, fec: Boolean): Int

        @JvmStatic external fun destroy(handle: Long)
    }
}
