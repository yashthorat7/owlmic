package com.owlmic.core.link

import com.owlmic.core.proto.Stream

/** Where encoded media goes. Media pipelines call this from their own threads; it never passes through a hub. */
interface MediaOut {
    fun audio(stream: Int, timestampUs: Long, data: ByteArray, offset: Int, length: Int)

    /** False when the picture was refused (too big, or the link wore out): the camera lowers its bitrate and starts a keyframe. */
    fun video(timestampUs: Long, keyframe: Boolean, data: ByteArray, offset: Int, length: Int): Boolean

    fun videoCongested(): Boolean = false
}

/** Takes the PC's speaker audio, on a carrier's receive thread. */
fun interface SpeakerSink {
    fun packet(seq: Long, timestampUs: Long, data: ByteArray)
}

/** One session's media: its carrier and both directions of packet handling. */
class MediaPath(val carrier: MediaCarrier, val out: Packetizer, val inbound: Unpacker)

/**
 * The data plane (section 11.2, rule 6): points media at the active link's path and swaps it at a frame boundary
 * when the Link Hub switches links. Packets from a path being drained are still delivered. Each stream has one
 * producer thread (mic, camera), so sending needs no lock.
 */
class MediaRouter : MediaOut {
    @Volatile private var path: MediaPath? = null

    @Volatile var speaker: SpeakerSink? = null

    /** What the speaker stream looks like on arrival, for the Monitor's report. */
    val speakerStats = InboundStats()

    fun use(p: MediaPath?) {
        path = p
    }

    val active: MediaPath? get() = path

    override fun audio(stream: Int, timestampUs: Long, data: ByteArray, offset: Int, length: Int) {
        val p = path ?: return
        p.out.audio(stream, timestampUs, data, offset, length)?.let(p.carrier::send)
    }

    override fun video(timestampUs: Long, keyframe: Boolean, data: ByteArray, offset: Int, length: Int): Boolean {
        val p = path ?: return true
        return p.out.video(timestampUs, keyframe, data, offset, length) { p.carrier.send(it) }
    }

    override fun videoCongested(): Boolean = path?.carrier?.congested == true

    /** A packet from [from]'s carrier. */
    fun received(from: MediaPath, packet: ByteArray, length: Int) {
        val (header, payload) = synchronized(from.inbound) { from.inbound.open(packet, length) } ?: return
        if (header.stream == Stream.SPEAKER) {
            speakerStats.add(header.seq, header.timestampUs, payload.size)
            speaker?.packet(header.seq, header.timestampUs, payload)
        }
    }
}
