package com.owlmic.core.link

import com.owlmic.core.proto.Crypto
import com.owlmic.core.proto.FragmentHeader
import com.owlmic.core.proto.Frames
import com.owlmic.core.proto.MediaHeader
import com.owlmic.core.proto.Stream

/**
 * Builds the media packets of one session direction (protocol section 6). Each stream has its own producer thread
 * and its own seq; on wireless links the seq is also the AES-GCM counter. Each packet is one allocation: the header,
 * the payload and the tag are written straight into it.
 */
class Packetizer(private val sealing: Crypto.Sealing?, firstSeq: Long = 0) {
    private val seq = LongArray(4) { firstSeq }
    private var frame = 0

    /**
     * True once a stream's seq is about to wrap on a sealed link. A wrapped seq would repeat a GCM nonce under the
     * same key, so from here nothing more is sent, and the Link Hub closes the link and handshakes again for new keys.
     */
    @Volatile var wornOut = false
        private set

    /** Null once [wornOut]. */
    fun audio(stream: Int, timestampUs: Long, data: ByteArray, offset: Int, length: Int): ByteArray? =
        if (room(stream, 1)) packet(stream, false, timestampUs, data, offset, length, false, 0, 0, 0) else null

    /**
     * Splits one encoded picture into fragments of at most [Frames.MAX_FRAGMENT_PAYLOAD] bytes and hands each packet to
     * [emit]. False, and nothing sent, for a picture over [Frames.MAX_FRAGMENTS] fragments or once [wornOut].
     */
    fun video(
        timestampUs: Long,
        keyframe: Boolean,
        data: ByteArray,
        offset: Int,
        length: Int,
        emit: (ByteArray) -> Unit,
    ): Boolean {
        val count = fragmentCount(length)
        if (count > Frames.MAX_FRAGMENTS || !room(Stream.CAMERA, count)) return false
        val f = frame
        frame = (frame + 1) and 0xFFFF
        for (i in 0 until count) {
            val start = i * Frames.MAX_FRAGMENT_PAYLOAD
            val size = minOf(Frames.MAX_FRAGMENT_PAYLOAD, length - start)
            emit(packet(Stream.CAMERA, keyframe, timestampUs, data, offset + start, size, true, f, i, count))
        }
        return true
    }

    /** Stream 0: session id and its MAC. Never sealed: the MAC already proves it, and the PC learns our address from it. */
    fun carrierHello(sessionId: ByteArray, mac: ByteArray): ByteArray =
        MediaHeader(Stream.CARRIER_HELLO, false, 0, 0).encode() + sessionId + mac

    /** Whether [stream] can send [count] more packets before its seq wraps. */
    private fun room(stream: Int, count: Int): Boolean {
        if (sealing == null) return true
        if (seq[stream] + count > SEQ_LIMIT) wornOut = true
        return !wornOut
    }

    private fun packet(
        stream: Int,
        keyframe: Boolean,
        timestampUs: Long,
        data: ByteArray,
        offset: Int,
        length: Int,
        fragmented: Boolean,
        frameIdx: Int,
        fragIdx: Int,
        fragCount: Int,
    ): ByteArray {
        val s = seq[stream]
        seq[stream] = (s + 1) and 0xFFFFFFFFL
        val headerLength = if (fragmented) FragmentHeader.LEN else 0
        val bodyLength = headerLength + length
        val out = ByteArray(MediaHeader.LEN + bodyLength + if (sealing != null) Crypto.TAG else 0)
        MediaHeader.encodeInto(out, 0, stream, keyframe, s, timestampUs and 0xFFFFFFFFL)
        if (fragmented) {
            FragmentHeader.encodeInto(out, MediaHeader.LEN, frameIdx, fragIdx, fragCount)
        }
        System.arraycopy(data, offset, out, MediaHeader.LEN + headerLength, length)
        sealing?.sealInPlace(stream, s, out, MediaHeader.LEN, bodyLength)
        return out
    }

    companion object {
        /** Well short of 2^32, so a whole keyframe still fits before the wrap. */
        const val SEQ_LIMIT = 0xFFFFFFFFL - 2 * Frames.MAX_FRAGMENTS

        fun fragmentCount(length: Int) = maxOf(1, (length + Frames.MAX_FRAGMENT_PAYLOAD - 1) / Frames.MAX_FRAGMENT_PAYLOAD)
    }
}

/** Opens the PC's media packets: checks the seal on wireless links and drops replays (protocol section 6). */
class Unpacker(private val sealing: Crypto.Sealing?) {
    private val windows = Array(4) { Crypto.ReplayWindow() }

    /** The header and payload, or null for a packet that fails its seal, repeats, or is malformed. */
    fun open(packet: ByteArray, length: Int): Pair<MediaHeader, ByteArray>? {
        val header = MediaHeader.decode(packet, length) ?: return null
        if (header.stream !in 1..3) return null
        val payload = sealing?.openPacket(header.stream, header.seq, packet, MediaHeader.LEN, length)
            ?: if (sealing == null) packet.copyOfRange(MediaHeader.LEN, length) else return null
        if (!windows[header.stream].accept(header.seq)) return null
        return header to payload
    }
}
