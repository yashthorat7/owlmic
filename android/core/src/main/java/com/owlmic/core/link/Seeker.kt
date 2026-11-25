package com.owlmic.core.link

import android.net.Network
import com.owlmic.core.hub.LinkKind
import com.owlmic.core.proto.Answer
import com.owlmic.core.proto.Probe
import com.owlmic.core.proto.Proto
import com.owlmic.core.toHex
import java.io.Closeable
import java.io.IOException
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.Socket
import java.net.SocketTimeoutException
import java.util.concurrent.TimeUnit
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.thread
import kotlin.concurrent.withLock

interface SeekerApi : Closeable {
    var pace: Seeker.Pace
    fun burst()
    fun tunnelReleased()
}

/**
 * Finds PCs (section 14.2): the phone asks, the PC answers. A burst of probes at 0, 100 and 300 ms on every interface,
 * then one a second while searching and one every 2 s while connected (to notice better links). Alongside, it tries
 * the adb tunnel on 127.0.0.1:7653. Probes to a PC on the Wi-Fi go out on a socket bound to the Wi-Fi network
 * ([networkFor]); the rest use Android's normal routing. [onFound] runs on the Seeker's threads.
 */
class Seeker(
    private val phoneId: ByteArray,
    private val phoneName: () -> String,
    private val manualAddresses: () -> List<String>,
    private val networkFor: (InetAddress) -> Network?,
    private val onFound: (Candidate) -> Unit,
) : SeekerApi {
    enum class Pace { SEARCHING, CONNECTED, CONNECTED_BY_USB_DEBUGGING }

    @Volatile override var pace = Pace.SEARCHING

    @Volatile private var open = true

    @Volatile private var burstAt = 0L

    /** Set while a found tunnel is being tried, so the PC isn't dialled again meanwhile. */
    @Volatile private var tunnelBusy = false

    private val resolved = HashMap<String, Pair<InetAddress?, Long>>()

    /** The probe socket bound to the current Wi-Fi network, with its own receive thread. Probe thread only. */
    @Volatile private var bound: Pair<Network, DatagramSocket>? = null
    private val wake = ReentrantLock()
    private val woken = wake.newCondition()

    init {
        thread(name = "owlmic-seeker", isDaemon = true) { probeLoop() }
        thread(name = "owlmic-seeker-usb", isDaemon = true) { tunnelLoop() }
    }

    /** Starts a new burst: the app opened, or a network came or went. */
    override fun burst() {
        burstAt = System.currentTimeMillis()
        wake.withLock { woken.signalAll() }
    }

    /** The Link Hub is done with the tunnel it was handed (used or failed): look again. */
    override fun tunnelReleased() {
        tunnelBusy = false
        wake.withLock { woken.signalAll() }
    }

    private fun probeLoop() {
        burst()
        var lastProbe = 0L
        var burstSent = 0
        var lastBurst = -1L
        try {
            DatagramSocket().use { socket ->
                socket.broadcast = true
                socket.soTimeout = 100
                val buf = ByteArray(1_024)
                val packet = DatagramPacket(buf, buf.size)
                while (open) {
                    val now = System.currentTimeMillis()
                    if (burstAt != lastBurst) {
                        lastBurst = burstAt
                        burstSent = 0
                    }
                    val sinceBurst = now - lastBurst
                    val due = when {
                        burstSent < BURST_MS.size -> sinceBurst >= BURST_MS[burstSent]
                        else -> now - lastProbe >= if (pace == Pace.SEARCHING) SEARCH_EVERY_MS else CONNECTED_EVERY_MS
                    }
                    if (due) {
                        sendProbes(socket)
                        lastProbe = now
                        if (burstSent < BURST_MS.size) burstSent++
                    }
                    try {
                        packet.length = buf.size
                        socket.receive(packet)
                        answerFrom(packet)?.let(onFound)
                    } catch (_: SocketTimeoutException) {
                        // Nothing this round.
                    }
                }
            }
        } catch (_: IOException) {
            // The socket closed: the Seeker is shutting down.
        }
    }

    private fun sendProbes(socket: DatagramSocket) {
        val probe = Probe(phoneId, phoneName()).encode()
        for (nic in probeInterfaces()) {
            runCatching { socketFor(nic.address, socket).send(DatagramPacket(probe, probe.size, nic.broadcast, Proto.PORT_DISCOVERY)) }
        }
        for (address in manualAddresses()) {
            val host = resolve(address) ?: continue
            runCatching { socketFor(host, socket).send(DatagramPacket(probe, probe.size, host, Proto.PORT_DISCOVERY)) }
        }
    }

    /** [plain], or for a destination on the Wi-Fi network a socket bound to it, made when the network changes. */
    private fun socketFor(destination: InetAddress, plain: DatagramSocket): DatagramSocket {
        val network = networkFor(destination) ?: return plain
        bound?.let { (n, s) ->
            if (n == network && !s.isClosed) return s
            s.close()
        }
        val s = try {
            DatagramSocket().apply {
                broadcast = true
                network.bindSocket(this)
            }
        } catch (_: IOException) {
            bound = null
            return plain
        }
        bound = network to s
        thread(name = "owlmic-seeker-wifi", isDaemon = true) { receive(s) }
        return s
    }

    /** Answers to probes sent on [socket], until it closes. */
    private fun receive(socket: DatagramSocket) {
        val buf = ByteArray(1_024)
        val packet = DatagramPacket(buf, buf.size)
        while (open && !socket.isClosed) {
            try {
                packet.length = buf.size
                socket.receive(packet)
                answerFrom(packet)?.let(onFound)
            } catch (_: IOException) {
                // Closed: the network went or the Seeker is shutting down.
            }
        }
    }

    /** Names are looked up at most every 30 s, since a lookup can take seconds. */
    private fun resolve(address: String): InetAddress? {
        val now = System.currentTimeMillis()
        resolved[address]?.let { (host, at) -> if (now - at < RESOLVE_EVERY_MS) return host }
        val host = runCatching { InetAddress.getByName(address) }.getOrNull()
        resolved[address] = host to now
        return host
    }

    private fun answerFrom(packet: DatagramPacket): Candidate? {
        val a = Answer.decode(packet.data, packet.length) ?: return null
        val via = probeInterfaces().firstOrNull { it.contains(packet.address) }
        val link = if (via?.kind == LinkKind.USB_TETHERING) {
            LinkKind.USB_TETHERING
        } else {
            LinkKind.of(a.link)?.takeIf { it == LinkKind.USB_TETHERING || it == LinkKind.WIFI } ?: via?.kind ?: LinkKind.WIFI
        }
        return Candidate(
            pcId = a.pcId.toHex(),
            name = a.name,
            link = link,
            host = packet.address,
            tcpPort = a.tcpPort,
            mediaPort = a.mediaPort,
            busy = a.busy,
            approvalRequired = a.approvalRequired,
            hotspot = via?.hotspot == true,
            keyHint = a.keyHint.toHex(),
        )
    }

    private fun tunnelLoop() {
        while (open) {
            val every = when (pace) {
                Pace.SEARCHING -> TUNNEL_SEARCH_EVERY_MS
                Pace.CONNECTED -> TUNNEL_CONNECTED_EVERY_MS
                Pace.CONNECTED_BY_USB_DEBUGGING -> null
            }
            if (every != null && !tunnelBusy) {
                val socket = Socket()
                try {
                    socket.tcpNoDelay = true
                    socket.connect(InetSocketAddress(InetAddress.getLoopbackAddress(), Proto.PORT_CONTROL), TUNNEL_CONNECT_MS)
                    tunnelBusy = true
                    onFound(Candidate(pcId = null, name = "", link = LinkKind.USB_DEBUGGING, tcpPort = Proto.PORT_CONTROL, opened = socket))
                } catch (_: IOException) {
                    runCatching { socket.close() }
                }
            }
            wake.withLock {
                val waitMs = every ?: TUNNEL_CONNECTED_EVERY_MS
                if (open && waitMs > 0) runCatching { woken.await(waitMs, TimeUnit.MILLISECONDS) }
            }
        }
    }

    override fun close() {
        open = false
        bound?.second?.close()
        wake.withLock { woken.signalAll() }
    }

    private companion object {
        val BURST_MS = longArrayOf(0, 100, 300)
        const val SEARCH_EVERY_MS = 1_000L
        const val CONNECTED_EVERY_MS = 2_000L
        const val TUNNEL_SEARCH_EVERY_MS = 500L
        const val TUNNEL_CONNECTED_EVERY_MS = 1_000L
        const val TUNNEL_CONNECT_MS = 800
        const val RESOLVE_EVERY_MS = 30_000L
    }
}
