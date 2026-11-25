package com.owlmic.core.proto

/** Control frames, media packets and video fragments (protocol/README.md, sections 3, 4 and 6). */
object Frames {
    /** The first byte of every TCP connection to the control port. */
    const val CHANNEL_CONTROL = 0x01
    const val CHANNEL_MEDIA = 0x02

    const val MAX_CONTROL_PAYLOAD = 1 shl 20

    /** Starts a media packet on a stream carrier; control types are always below it. */
    const val MEDIA_MARKER = 0x80
    const val MAX_FRAGMENT_PAYLOAD = 1200

    /** The most fragments one picture may have; more is treated as a bad packet. */
    const val MAX_FRAGMENTS = 4096

    /** Appends [packet] the way stream carriers send it: the media marker and a 2-byte length. */
    fun wrapForStream(packet: ByteArray): ByteArray {
        require(packet.size <= 0xFFFF) { "media packets fit in 64 KiB" }
        return byteArrayOf(MEDIA_MARKER.toByte(), (packet.size shr 8).toByte(), packet.size.toByte()) + packet
    }
}

/** Control message types. */
object Kind {
    const val HELLO = 0x01
    const val HELLO_ACK = 0x02
    const val PROOF = 0x03
    const val PENDING = 0x04
    const val WELCOME = 0x05
    const val REJECT = 0x06
    const val PING = 0x10
    const val PONG = 0x11
    const val REPORT = 0x12
    const val STATE = 0x20
    const val SETTINGS = 0x21
    const val STREAM_START = 0x22
    const val STREAM_STOP = 0x23
    const val KEYFRAME_REQUEST = 0x24
    const val RESTART_STREAM = 0x25
    const val SWITCH = 0x30
    const val BYE = 0x3F
}

/** Media streams. */
object Stream {
    const val CARRIER_HELLO = 0
    const val MIC = 1
    const val CAMERA = 2
    const val SPEAKER = 3
}

data class ControlHeader(val kind: Int, val length: Int) {
    fun encode() = byteArrayOf(kind.toByte(), (length shr 16).toByte(), (length shr 8).toByte(), length.toByte())

    companion object {
        const val LEN = 4

        /** Null for a media marker or a payload over 1 MiB. */
        fun decode(b: ByteArray): ControlHeader? {
            val kind = b[0].toInt() and 0xFF
            val length = ((b[1].toInt() and 0xFF) shl 16) or b.u16(2)
            return if (kind < Frames.MEDIA_MARKER && length <= Frames.MAX_CONTROL_PAYLOAD) ControlHeader(kind, length) else null
        }
    }
}

data class MediaHeader(val stream: Int, val keyframe: Boolean, val seq: Long, val timestampUs: Long) {
    fun encode() = ByteArray(LEN).also { encodeInto(it) }

    fun encodeInto(out: ByteArray, at: Int = 0) {
        out[at] = stream.toByte()
        out[at + 1] = (if (keyframe) 1 else 0).toByte()
        out.put32(at + 2, seq)
        out.put32(at + 6, timestampUs)
    }

    companion object {
        const val LEN = 10

        fun encodeInto(out: ByteArray, at: Int = 0, stream: Int, keyframe: Boolean, seq: Long, timestampUs: Long) {
            out[at] = stream.toByte()
            out[at + 1] = (if (keyframe) 1 else 0).toByte()
            out.put32(at + 2, seq)
            out.put32(at + 6, timestampUs)
        }

        fun decode(b: ByteArray, length: Int = b.size): MediaHeader? {
            if (length < LEN) return null
            return MediaHeader(b[0].toInt() and 0xFF, b[1].toInt() and 1 != 0, b.u32(2), b.u32(6))
        }
    }
}

data class FragmentHeader(val frame: Int, val index: Int, val count: Int) {
    fun encode() = ByteArray(LEN).also { encodeInto(it) }

    fun encodeInto(out: ByteArray, at: Int = 0) {
        out.put16(at, frame)
        out.put16(at + 2, index)
        out.put16(at + 4, count)
    }

    companion object {
        const val LEN = 6

        fun encodeInto(out: ByteArray, at: Int = 0, frame: Int, index: Int, count: Int) {
            out.put16(at, frame)
            out.put16(at + 2, index)
            out.put16(at + 4, count)
        }

        /** Null when the index is outside the count or the count is over [Frames.MAX_FRAGMENTS]. */
        fun decode(b: ByteArray, length: Int = b.size): FragmentHeader? {
            if (length < LEN) return null
            val h = FragmentHeader(b.u16(0), b.u16(2), b.u16(4))
            return if (h.index < h.count && h.count <= Frames.MAX_FRAGMENTS) h else null
        }
    }
}

private fun ByteArray.put16(at: Int, v: Int) {
    this[at] = (v shr 8).toByte()
    this[at + 1] = v.toByte()
}

private fun ByteArray.put32(at: Int, v: Long) {
    this[at] = (v shr 24).toByte()
    this[at + 1] = (v shr 16).toByte()
    this[at + 2] = (v shr 8).toByte()
    this[at + 3] = v.toByte()
}
