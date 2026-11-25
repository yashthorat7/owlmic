package com.owlmic.core.link

import com.owlmic.core.proto.Crypto
import com.owlmic.core.proto.FragmentHeader
import com.owlmic.core.proto.Frames
import com.owlmic.core.proto.MediaHeader
import com.owlmic.core.proto.Stream
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class PacketsTest {
    private val key = ByteArray(32) { it.toByte() }

    @Test
    fun sealedAudioOpensOnceAndReplaysAreDropped() {
        val out = Packetizer(Crypto.Sealing(key))
        val inbound = Unpacker(Crypto.Sealing(key))
        val data = byteArrayOf(1, 2, 3, 4)
        val p0 = out.audio(Stream.SPEAKER, 1_000, data, 0, data.size)!!
        val p1 = out.audio(Stream.SPEAKER, 11_000, data, 0, data.size)!!
        assertEquals(MediaHeader.LEN + data.size + 16, p0.size)

        val (h1, b1) = inbound.open(p1, p1.size)!!
        assertEquals(1L, h1.seq)
        assertArrayEquals(data, b1)
        assertNotNull(inbound.open(p0, p0.size))
        assertNull(inbound.open(p0, p0.size))
    }

    @Test
    fun aTamperedOrForeignPacketIsDropped() {
        val out = Packetizer(Crypto.Sealing(key))
        val p = out.audio(Stream.SPEAKER, 0, byteArrayOf(9, 9), 0, 2)!!
        val tampered = p.copyOf().also { it[it.size - 1] = (it[it.size - 1] + 1).toByte() }
        assertNull(Unpacker(Crypto.Sealing(key)).open(tampered, tampered.size))
        assertNull(Unpacker(Crypto.Sealing(ByteArray(32))).open(p, p.size))
    }

    @Test
    fun unsealedOnCablesAndSeqPerStream() {
        val out = Packetizer(null)
        val a = out.audio(Stream.MIC, 0, byteArrayOf(7), 0, 1)!!
        val b = out.audio(Stream.SPEAKER, 0, byteArrayOf(7), 0, 1)!!
        assertEquals(MediaHeader.LEN + 1, a.size)
        assertEquals(0L, MediaHeader.decode(a, a.size)!!.seq)
        assertEquals(0L, MediaHeader.decode(b, b.size)!!.seq)
    }

    @Test
    fun aPictureIsSplitIntoNumberedFragments() {
        val out = Packetizer(null)
        val picture = ByteArray(Frames.MAX_FRAGMENT_PAYLOAD * 2 + 5) { it.toByte() }
        val packets = mutableListOf<ByteArray>()
        assertTrue(out.video(0, true, picture, 0, picture.size) { packets += it })
        assertEquals(3, packets.size)
        val joined = packets.flatMapIndexed { i, p ->
            val h = MediaHeader.decode(p, p.size)!!
            assertTrue(h.keyframe)
            assertEquals(i.toLong(), h.seq)
            val f = FragmentHeader.decode(p.copyOfRange(MediaHeader.LEN, p.size))!!
            assertEquals(FragmentHeader(0, i, 3), f)
            p.copyOfRange(MediaHeader.LEN + FragmentHeader.LEN, p.size).toList()
        }
        assertArrayEquals(picture, joined.toByteArray())

        val next = mutableListOf<ByteArray>()
        out.video(0, false, picture, 0, 10) { next += it }
        assertEquals(1, FragmentHeader.decode(next.single().copyOfRange(MediaHeader.LEN, next.single().size))!!.frame)
    }

    @Test
    fun aPictureOverMaxFragmentsIsNotSent() {
        val out = Packetizer(null)
        val huge = ByteArray(Frames.MAX_FRAGMENT_PAYLOAD * Frames.MAX_FRAGMENTS + 1)
        var sent = 0
        assertFalse(out.video(0, true, huge, 0, huge.size) { sent++ })
        assertEquals(0, sent)
    }

    @Test
    fun aBigKeyframeOfThreeHundredFragmentsIsSent() {
        val out = Packetizer(null)
        val keyframe = ByteArray(Frames.MAX_FRAGMENT_PAYLOAD * 300)
        var sent = 0
        assertTrue(out.video(0, true, keyframe, 0, keyframe.size) { sent++ })
        assertEquals(300, sent)
    }

    @Test
    fun sealedFragmentsOpenToThePicture() {
        val out = Packetizer(Crypto.Sealing(key))
        val inbound = Unpacker(Crypto.Sealing(key))
        val picture = ByteArray(Frames.MAX_FRAGMENT_PAYLOAD + 7) { (it * 3).toByte() }
        val packets = mutableListOf<ByteArray>()
        assertTrue(out.video(5, true, picture, 0, picture.size) { packets += it })
        val joined = packets.flatMap { p ->
            val (h, payload) = inbound.open(p, p.size)!!
            assertTrue(h.keyframe)
            payload.copyOfRange(FragmentHeader.LEN, payload.size).toList()
        }
        assertArrayEquals(picture, joined.toByteArray())
        assertEquals(MediaHeader.LEN + FragmentHeader.LEN + 7 + Crypto.TAG, packets.last().size)
    }

    @Test
    fun aSealedLinkStopsBeforeItsSeqWouldWrap() {
        val out = Packetizer(Crypto.Sealing(key), firstSeq = Packetizer.SEQ_LIMIT - 1)
        assertNotNull(out.audio(Stream.MIC, 0, byteArrayOf(1), 0, 1))
        assertFalse(out.wornOut)
        // The next packet would cross the limit: nothing goes out under a nonce that could repeat.
        assertNull(out.audio(Stream.MIC, 0, byteArrayOf(1), 0, 1))
        assertTrue(out.wornOut)
        assertFalse(out.video(0, true, byteArrayOf(1), 0, 1) {})
    }

    @Test
    fun aCableLinkWrapsItsSeqFreely() {
        val out = Packetizer(null, firstSeq = 0xFFFFFFFFL)
        out.audio(Stream.MIC, 0, byteArrayOf(1), 0, 1)
        val wrapped = out.audio(Stream.MIC, 0, byteArrayOf(1), 0, 1)!!
        assertEquals(0L, MediaHeader.decode(wrapped, wrapped.size)!!.seq)
        assertFalse(out.wornOut)
    }

    @Test
    fun theCarrierHelloIsInTheClear() {
        val session = ByteArray(16) { 1 }
        val mac = ByteArray(16) { 2 }
        val p = Packetizer(Crypto.Sealing(key)).carrierHello(session, mac)
        assertEquals(Stream.CARRIER_HELLO, MediaHeader.decode(p, p.size)!!.stream)
        assertArrayEquals(session + mac, p.copyOfRange(MediaHeader.LEN, p.size))
    }

    @Test
    fun sharedFrameCounterKeepsVideoFrameIdsMonotonicAcrossPacketizers() {
        val router = MediaRouter()
        val first = Packetizer(null, nextFrame = router::nextVideoFrame)
        val second = Packetizer(null, nextFrame = router::nextVideoFrame)
        val emitted = mutableListOf<Pair<Long, Int>>()
        fun capture(p: ByteArray) {
            val h = MediaHeader.decode(p, p.size)!!
            val f = FragmentHeader.decode(p.copyOfRange(MediaHeader.LEN, p.size))!!
            emitted += h.seq to f.frame
        }
        assertTrue(first.video(10, true, byteArrayOf(1), 0, 1, ::capture))
        assertTrue(first.video(20, false, byteArrayOf(2), 0, 1, ::capture))
        assertTrue(second.video(30, true, byteArrayOf(3), 0, 1, ::capture))
        assertTrue(second.video(40, false, byteArrayOf(4), 0, 1, ::capture))
        assertEquals(listOf(0L to 0, 1L to 1, 0L to 2, 1L to 3), emitted)
    }
}
