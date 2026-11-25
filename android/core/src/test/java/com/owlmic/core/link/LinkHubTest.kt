package com.owlmic.core.link

import com.owlmic.core.fromBase64
import com.owlmic.core.hexToBytes
import com.owlmic.core.hub.Connection
import com.owlmic.core.hub.LinkKind
import com.owlmic.core.proto.AckStatus
import com.owlmic.core.proto.Bye
import com.owlmic.core.proto.Crypto
import com.owlmic.core.proto.Frames
import com.owlmic.core.proto.Hello
import com.owlmic.core.proto.HelloAck
import com.owlmic.core.proto.Message
import com.owlmic.core.proto.Ping
import com.owlmic.core.proto.Pong
import com.owlmic.core.proto.Proto
import com.owlmic.core.proto.Reject
import com.owlmic.core.proto.RejectReason
import com.owlmic.core.proto.Switch
import com.owlmic.core.proto.Welcome
import com.owlmic.core.settings.KeyWrapper
import com.owlmic.core.settings.Store
import com.owlmic.core.toBase64
import com.owlmic.core.toHex
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNotSame
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.net.DatagramSocket
import java.net.InetAddress
import java.net.ServerSocket
import java.net.Socket
import java.net.SocketTimeoutException
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit
import kotlin.concurrent.thread

/** The Link Hub against a fake PC on loopback that plays protocol section 4 on each link it is offered. */
class LinkHubTest {
    @get:Rule val folder = TemporaryFolder()

    private val loopback = InetAddress.getLoopbackAddress()
    private val server = ServerSocket(0, 2, loopback)
    private val media = DatagramSocket(0, loopback)
    private val pcKeys = Crypto.generate()
    private val pcId = ByteArray(16) { 9 }.toHex()
    private val sessionId = "00112233445566778899aabbccddeeff"
    private val open = mutableListOf<Socket>()
    private val events = LinkedBlockingQueue<LinkEvent>()
    private val hellos = LinkedBlockingQueue<Hello>()

    @Volatile private var clock = 1_000_000L

    @After
    fun close() {
        open.forEach { runCatching { it.close() } }
        server.close()
        media.close()
    }

    /** [timed]: the store reads the test's clock, for what it records about time. */
    private fun store(timed: Boolean = false) = Store(
        folder.root.resolve("s.json"),
        object : KeyWrapper {
            override fun wrap(secret: ByteArray) = secret

            override fun unwrap(blob: ByteArray) = blob
        },
        now = if (timed) ({ clock }) else System::currentTimeMillis,
        write = Runnable::run,
    )

    private fun candidate(link: LinkKind) =
        Candidate(pcId, "DESKTOP-A", link, host = loopback, tcpPort = server.localPort, mediaPort = media.localPort)

    /** The PC's side of one link's handshake: HELLO, HELLO_ACK, PROOF, WELCOME into the one session. */
    private fun welcomeNext(): ControlChannel {
        val s = server.accept().also { open += it }
        check(s.getInputStream().read() == Frames.CHANNEL_CONTROL)
        val ch = ControlChannel(s.getInputStream(), s.getOutputStream(), s)
        val helloIn = ch.read() as Incoming.Control
        val hello = helloIn.message as Hello
        hellos.put(hello)
        val eph = Crypto.generate()
        val nonce = Crypto.randomBytes(32)
        val ackBytes = ch.send(HelloAck(Proto.VERSION, pcId, "DESKTOP-A", pcKeys.public.toBase64(), eph.public.toBase64(), nonce.toBase64(), AckStatus.KNOWN, null))
        val k = Crypto.pairingKey(pcKeys.agree(hello.staticPub.fromBase64()!!)!!, hello.phoneId.hexToBytes(), pcId.hexToBytes())
        val t = Crypto.transcript(helloIn.payload, ackBytes)
        val keys = Crypto.sessionKeys(Crypto.sessionMaster(k, eph.agree(hello.ephPub.fromBase64()!!)!!, hello.nonce.fromBase64()!!, nonce))
        ch.read()
        ch.send(Welcome(sessionId, Crypto.proof(keys.auth, Crypto.Role.PC, t).toBase64(), emptyMap(), emptyList()))
        if (LinkKind.of(hello.link)!!.isWireless) ch.sealing = ControlSealing(keys.pcToPhone, keys.phoneToPc)
        return ch
    }

    /** Answers the phone's heartbeats, so a link stays alive until the test ends. */
    private fun keepAlive(ch: ControlChannel) = thread(isDaemon = true) {
        runCatching {
            while (true) {
                val m = (ch.read() as? Incoming.Control)?.message
                if (m is Ping) ch.send(Pong(m.t))
            }
        }
    }

    private inline fun <reified E : LinkEvent> next(match: (E) -> Boolean = { true }): E {
        val deadline = System.currentTimeMillis() + 5_000
        val seen = mutableListOf<Any>()
        while (System.currentTimeMillis() < deadline) {
            val e = events.poll(100, TimeUnit.MILLISECONDS) ?: continue
            seen += (e as? LinkEvent.ConnectionChanged)?.connection ?: e
            if (e is E && match(e)) return e
        }
        error("no ${E::class.simpleName} in $seen")
    }

    private fun connectedOver(link: LinkKind) =
        next<LinkEvent.ConnectionChanged> { (it.connection as? Connection.Connected)?.link == link }

    private val noopSeeker = object : SeekerApi {
        override var pace = Seeker.Pace.SEARCHING
        override fun burst() {}
        override fun tunnelReleased() {}
        override fun close() {}
    }

    private fun hub(store: Store = store()) =
        LinkHub(store, MediaRouter(), null, null, { "Pixel" }, "Pixel 8", 34, { null }, { false }, { events.put(it) }, { clock }, seekerFactory = { noopSeeker })

    @Test
    fun aPcWhoseHandshakesKeepFailingIsNamedAfterTenSeconds() {
        // A PC that answers discovery but hangs up on every handshake.
        thread(isDaemon = true) { runCatching { while (true) server.accept().close() } }
        val hub = hub()
        hub.post(LinkMsg.Found(candidate(LinkKind.WIFI)))
        next<LinkEvent.ConnectionChanged> { it.connection is Connection.Connecting }
        next<LinkEvent.ConnectionChanged> { it.connection == Connection.Searching() }
        // It keeps answering, so the phone retries every 2 s; ten seconds in, the status names it.
        var unreachable: String? = null
        var tries = 0
        while (unreachable == null && tries++ < 8) {
            clock += 2_000
            hub.post(LinkMsg.Found(candidate(LinkKind.WIFI)))
            unreachable = (next<LinkEvent.ConnectionChanged> { it.connection is Connection.Searching }.connection as Connection.Searching).unreachable
        }
        assertEquals("DESKTOP-A", unreachable)
        assertEquals(5, tries)
        hub.close()
    }

    @Test
    fun aPairedPcWithAnotherKeyIsNotTrusted() {
        val store = store()
        store.remember(pcId, "DESKTOP-A", Crypto.generate().public, ByteArray(32), null)
        thread(isDaemon = true) { runCatching { welcomeNext() } }
        val hub = hub(store)
        hub.post(LinkMsg.Found(candidate(LinkKind.WIFI)))
        next<LinkEvent.ConnectionChanged> { it.connection == Connection.KeyChanged("DESKTOP-A") }
        hub.close()
    }

    @Test
    fun aNewerPcAsksThisPhoneToUpdate() {
        thread(isDaemon = true) {
            val s = server.accept().also { open += it }
            s.getInputStream().read()
            val ch = ControlChannel(s.getInputStream(), s.getOutputStream(), s)
            ch.read()
            ch.send(Reject(RejectReason.VERSION, proto = Proto.VERSION + 1))
        }
        val hub = hub()
        hub.post(LinkMsg.Found(candidate(LinkKind.WIFI)))
        next<LinkEvent.ConnectionChanged> { it.connection == Connection.UpdateNeeded("DESKTOP-A", phoneOutdated = true) }
        hub.close()
    }

    @Test
    fun theSwitchArrivesOnTheNewLinkAndMediaMovesThere() {
        val router = MediaRouter()
        val hub = LinkHub(store(), router, null, null, { "Pixel" }, "Pixel 8", 34, { null }, { false }, { events.put(it) }, seekerFactory = { noopSeeker })
        thread(isDaemon = true) {
            keepAlive(welcomeNext())
            // The PC moves the session by sending SWITCH on the link it moves to (protocol section 4).
            val cable = welcomeNext()
            cable.send(Switch(LinkKind.USB_TETHERING.wire))
            keepAlive(cable)
        }

        hub.post(LinkMsg.Found(candidate(LinkKind.WIFI)))
        connectedOver(LinkKind.WIFI)
        val wifiPath = router.active
        assertNotNull(wifiPath)

        // A better link for the same PC: the phone proves it as a standby and waits for the PC.
        hub.post(LinkMsg.Found(candidate(LinkKind.USB_TETHERING)))
        connectedOver(LinkKind.USB_TETHERING)
        assertEquals(LinkKind.USB_TETHERING, next<LinkEvent.Switched>().link)
        assertNotSame(wifiPath, router.active)
        hub.close()
    }

    @Test
    fun theLastConnectedTimeStaysFreshWhileConnectedAndIsSetWhenTheLinkGoes() {
        val store = store(timed = true)
        thread(isDaemon = true) {
            runCatching {
                keepAlive(welcomeNext())
                welcomeNext()
            }
        }
        val hub = hub(store)
        hub.post(LinkMsg.Found(candidate(LinkKind.WIFI)))
        connectedOver(LinkKind.WIFI)
        assertEquals(1_000_000L, store.known().getValue(pcId).lastUsedAt)

        // A minute on, still connected: saved again, so a crash now leaves a close value.
        clock += 61_000
        val deadline = System.currentTimeMillis() + 5_000
        while (store.known().getValue(pcId).lastUsedAt != 1_061_000L && System.currentTimeMillis() < deadline) Thread.sleep(50)
        assertEquals(1_061_000L, store.known().getValue(pcId).lastUsedAt)

        // The link goes: connected until now.
        clock += 20_000
        hub.post(LinkMsg.Reconnect)
        next<LinkEvent.Held>()
        assertEquals(1_081_000L, store.known().getValue(pcId).lastUsedAt)
        hub.close()
    }

    @Test
    fun aSwitchKeepsTheNewLinkAndAddressInTheHistory() {
        val store = store()
        thread(isDaemon = true) {
            keepAlive(welcomeNext())
            val cable = welcomeNext()
            cable.send(Switch(LinkKind.USB_TETHERING.wire))
            keepAlive(cable)
        }
        val hub = hub(store)
        hub.post(LinkMsg.Found(candidate(LinkKind.WIFI)))
        connectedOver(LinkKind.WIFI)
        assertEquals(LinkKind.WIFI, store.known().getValue(pcId).lastLink)
        assertEquals("${loopback.hostAddress}:${server.localPort}", store.known().getValue(pcId).address)

        hub.post(LinkMsg.Found(candidate(LinkKind.USB_TETHERING)))
        next<LinkEvent.Switched>()
        assertEquals(LinkKind.USB_TETHERING, store.known().getValue(pcId).lastLink)
        hub.close()
    }

    @Test
    fun blockingTheConnectedPcSaysByeAndStaysAway() {
        val store = store()
        val got = LinkedBlockingQueue<Message>()
        thread(isDaemon = true) {
            val ch = welcomeNext()
            runCatching {
                while (true) {
                    val m = (ch.read() as? Incoming.Control)?.message ?: continue
                    if (m is Ping) ch.send(Pong(m.t))
                    got.put(m)
                }
            }
        }
        val hub = hub(store)
        hub.post(LinkMsg.Found(candidate(LinkKind.WIFI)))
        connectedOver(LinkKind.WIFI)

        hub.post(LinkMsg.Block(pcId, "DESKTOP-A"))
        next<LinkEvent.Ended>()
        next<LinkEvent.ConnectionChanged> { it.connection is Connection.Searching }
        assertTrue(generateSequence { got.poll(2, TimeUnit.SECONDS) }.any { it is Bye })
        assertTrue(store.known().getValue(pcId).blocked)

        // It keeps answering discovery, but the phone never dials it again.
        server.soTimeout = 1_500
        hub.post(LinkMsg.Found(candidate(LinkKind.WIFI)))
        assertThrows(SocketTimeoutException::class.java) { server.accept() }
        hub.close()
    }

    @Test
    fun anUnblockedPcIsDialledAgain() {
        val store = store()
        store.block(pcId, "DESKTOP-A", null)
        val hub = hub(store)
        hub.post(LinkMsg.Found(candidate(LinkKind.WIFI)))
        server.soTimeout = 700
        assertThrows(SocketTimeoutException::class.java) { server.accept() }
        server.soTimeout = 0

        thread(isDaemon = true) { runCatching { keepAlive(welcomeNext()) } }
        hub.post(LinkMsg.Unblock(pcId))
        connectedOver(LinkKind.WIFI)
        assertTrue(store.known().getValue(pcId).approved && !store.known().getValue(pcId).blocked)
        hub.close()
    }

    @Test
    fun reconnectWhileConnectedResumesTheSessionOverAFreshHandshake() {
        thread(isDaemon = true) {
            runCatching {
                keepAlive(welcomeNext())
                keepAlive(welcomeNext())
            }
        }
        val hub = hub()
        hub.post(LinkMsg.Found(candidate(LinkKind.WIFI)))
        connectedOver(LinkKind.WIFI)
        assertNull(hellos.poll(1, TimeUnit.SECONDS).resume)

        hub.post(LinkMsg.Reconnect)
        next<LinkEvent.Held>()
        connectedOver(LinkKind.WIFI)
        assertEquals(sessionId, hellos.poll(1, TimeUnit.SECONDS).resume)
        hub.close()
    }

    @Test
    fun reconnectWhileSearchingRetriesWithoutWaitingOutTheBackoff() {
        // A PC that answers discovery but hangs up on every handshake.
        thread(isDaemon = true) { runCatching { while (true) server.accept().close() } }
        val hub = hub()
        hub.post(LinkMsg.Found(candidate(LinkKind.WIFI)))
        next<LinkEvent.ConnectionChanged> { it.connection is Connection.Connecting }
        next<LinkEvent.ConnectionChanged> { it.connection == Connection.Searching() }

        // Without Reconnect, the next answer waits out the backoff (the clock doesn't move here).
        hub.post(LinkMsg.Found(candidate(LinkKind.WIFI)))
        Thread.sleep(300)
        assertTrue(events.none { (it as? LinkEvent.ConnectionChanged)?.connection is Connection.Connecting })

        hub.post(LinkMsg.Reconnect)
        next<LinkEvent.ConnectionChanged> { it.connection is Connection.Connecting }
        hub.close()
    }

    @Test
    fun aBlockedPcBehindTheUsbTunnelIsLetGoAndNotDialledInALoop() {
        val store = store()
        store.block(pcId, "DESKTOP-A", null)
        thread(isDaemon = true) { runCatching { welcomeNext() } }
        val hub = hub(store)
        fun tunnel() = Candidate(null, "", LinkKind.USB_DEBUGGING, opened = Socket(loopback, server.localPort).also { open += it })
        hub.post(LinkMsg.Found(tunnel()))
        next<LinkEvent.ConnectionChanged> { it.connection is Connection.Connecting }
        next<LinkEvent.ConnectionChanged> { it.connection is Connection.Searching }

        // The cable is still in and the tunnel answers again: the phone waits instead of shaking hands again.
        hub.post(LinkMsg.Found(tunnel()))
        Thread.sleep(500)
        assertTrue(events.none { (it as? LinkEvent.ConnectionChanged)?.connection is Connection.Connecting })
        assertEquals(1, hellos.size)
        hub.close()
    }

    @Test
    fun pcByeEndsSessionImmediatelyWithoutHoldingFor30s() {
        var ch: ControlChannel? = null
        thread(isDaemon = true) { ch = welcomeNext() }
        val hub = hub()
        hub.post(LinkMsg.Found(candidate(LinkKind.WIFI)))
        connectedOver(LinkKind.WIFI)

        ch!!.send(Bye("quit"))
        next<LinkEvent.Ended>()
        next<LinkEvent.ConnectionChanged> { it.connection is Connection.Searching }
        assertTrue(events.none { it is LinkEvent.Held })
        hub.close()
    }

    @Test
    fun activeWifiUpgradesToUsbDebuggingWhenTunnelAppears() {
        val router = MediaRouter()
        val hub = LinkHub(store(), router, null, null, { "Pixel" }, "Pixel 8", 34, { null }, { false }, { events.put(it) }, { clock }, seekerFactory = { noopSeeker })
        thread(isDaemon = true) {
            runCatching {
                keepAlive(welcomeNext())
                val control = welcomeNext()
                val mediaSock = server.accept().also { open += it }
                check(mediaSock.getInputStream().read() == Frames.CHANNEL_MEDIA)
                control.send(Switch(LinkKind.USB_DEBUGGING.wire))
                keepAlive(control)
            }
        }

        hub.post(LinkMsg.Found(candidate(LinkKind.WIFI)))
        connectedOver(LinkKind.WIFI)

        val tunnel = Candidate(null, "", LinkKind.USB_DEBUGGING, tcpPort = server.localPort, opened = Socket(loopback, server.localPort).also { open += it })
        hub.post(LinkMsg.Found(tunnel))
        connectedOver(LinkKind.USB_DEBUGGING)
        assertEquals(LinkKind.USB_DEBUGGING, next<LinkEvent.Switched>().link)
        hub.close()
    }

    @Test
    fun unusedUsbTunnelSocketIsDroppedDuringProbationAndRecoversAfterCableProbation() {
        val released = java.util.concurrent.atomic.AtomicInteger(0)
        val countingSeeker = object : SeekerApi {
            override var pace = Seeker.Pace.SEARCHING
            override fun burst() {}
            override fun tunnelReleased() {
                released.incrementAndGet()
            }
            override fun close() {}
        }
        val hub = LinkHub(store(), MediaRouter(), null, null, { "Pixel" }, "Pixel 8", 34, { null }, { false }, { events.put(it) }, { clock }, seekerFactory = { countingSeeker })
        thread(isDaemon = true) {
            runCatching {
                keepAlive(welcomeNext())
                // First standby tunnel attempt fails immediately.
                server.accept().close()
                // Second socket opened during 1 s cable probation is dropped by LinkHub before sending any bytes.
                server.accept().use { s -> s.getInputStream().read() }
                // Third valid standby tunnel succeeds after cable probation (1 s).
                val control = welcomeNext()
                val mediaSock = server.accept().also { open += it }
                check(mediaSock.getInputStream().read() == Frames.CHANNEL_MEDIA)
                control.send(Switch(LinkKind.USB_DEBUGGING.wire))
                keepAlive(control)
            }
        }

        hub.post(LinkMsg.Found(candidate(LinkKind.WIFI)))
        connectedOver(LinkKind.WIFI)

        fun tunnelSocket() = Socket(loopback, server.localPort).also { open += it }
        hub.post(LinkMsg.Found(Candidate(null, "", LinkKind.USB_DEBUGGING, tcpPort = server.localPort, opened = tunnelSocket())))
        val deadline = System.currentTimeMillis() + 5_000
        while (released.get() < 1 && System.currentTimeMillis() < deadline) Thread.sleep(20)
        assertTrue(released.get() >= 1)

        // While within 1 s cable probation, a pre-opened tunnel socket must be dropped immediately instead of kept stale.
        val duringProbation = tunnelSocket()
        hub.post(LinkMsg.Found(Candidate(null, "", LinkKind.USB_DEBUGGING, tcpPort = server.localPort, opened = duringProbation)))
        val dropDeadline = System.currentTimeMillis() + 5_000
        while (!duringProbation.isClosed && System.currentTimeMillis() < dropDeadline) Thread.sleep(20)
        assertTrue(duringProbation.isClosed)

        // After 1 s cable probation, a fresh tunnel connects and switches cleanly.
        clock += 1_100
        hub.post(LinkMsg.Found(Candidate(null, "", LinkKind.USB_DEBUGGING, tcpPort = server.localPort, opened = tunnelSocket())))
        connectedOver(LinkKind.USB_DEBUGGING)
        assertEquals(LinkKind.USB_DEBUGGING, next<LinkEvent.Switched>().link)
        hub.close()
    }
}
