package com.owlmic.core.link

import com.owlmic.core.hub.Connection
import com.owlmic.core.hub.Hub
import com.owlmic.core.hub.LinkKind
import com.owlmic.core.hub.PcChoice
import com.owlmic.core.hub.SearchStage
import com.owlmic.core.proto.Bye
import com.owlmic.core.proto.Crypto
import com.owlmic.core.proto.Frames
import com.owlmic.core.proto.KeyframeRequest
import com.owlmic.core.proto.Message
import com.owlmic.core.proto.Ping
import com.owlmic.core.proto.Pong
import com.owlmic.core.proto.Proto
import com.owlmic.core.proto.RejectReason
import com.owlmic.core.proto.Report
import com.owlmic.core.proto.RestartStream
import com.owlmic.core.proto.Settings
import com.owlmic.core.proto.State
import com.owlmic.core.proto.Stream
import com.owlmic.core.proto.StreamStart
import com.owlmic.core.proto.StreamStop
import com.owlmic.core.proto.Switch
import com.owlmic.core.settings.Store
import com.owlmic.core.toHex
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.Socket
import kotlin.concurrent.thread

sealed interface LinkMsg {
    class Found(val candidate: Candidate) : LinkMsg

    data object Tick : LinkMsg

    class AttemptPending(val attempt: Int, val pc: String, val code: String) : LinkMsg

    class AttemptDone(val attempt: Int, val result: Handshake.Result, val session: LinkSession?, val tunnel: Boolean) : LinkMsg

    class ControlIn(val session: LinkSession, val message: Message) : LinkMsg

    class Closed(val session: LinkSession) : LinkMsg

    class MediaRecreated(val session: LinkSession, val path: MediaPath) : LinkMsg

    class Send(val message: Message) : LinkMsg

    class Choose(val pcId: String) : LinkMsg

    data object AskAgain : LinkMsg

    class Forget(val pcId: String) : LinkMsg

    /** Never connect to [pcId] again until unblocked; [name] names a PC never paired. */
    class Block(val pcId: String, val name: String) : LinkMsg

    class Unblock(val pcId: String) : LinkMsg

    /** The user's hard reconnect: drop the link and dial again at once, or retry a search without waiting. */
    data object Reconnect : LinkMsg

    class Allowed(val links: Set<LinkKind>) : LinkMsg

    /** The app came to the screen, or a network came or went: look right away. */
    data object Opened : LinkMsg

    class SpeakerExpected(val on: Boolean) : LinkMsg

    /** The user granted Bluetooth after "Try Bluetooth". */
    data object TryBluetooth : LinkMsg
}

sealed interface LinkEvent {
    class ConnectionChanged(val connection: Connection) : LinkEvent

    class Choices(val pcs: List<PcChoice>) : LinkEvent

    class BluetoothOffer(val show: Boolean) : LinkEvent

    class TetherHint(val show: Boolean) : LinkEvent

    class Welcomed(val pcId: String, val pcName: String, val link: LinkKind, val settings: Map<String, String>, val resumed: Boolean) : LinkEvent

    class Switched(val link: LinkKind) : LinkEvent

    /** A control message for the rest of the app: STATE, SETTINGS, STREAM_START, STREAM_STOP, KEYFRAME_REQUEST, REPORT. */
    class Control(val message: Message) : LinkEvent

    /** The link dropped: the session is held for 30 s. */
    data object Held : LinkEvent

    /** 30 s without the PC: let go of the mic and camera, keep looking. */
    data object Release : LinkEvent

    /** 5 minutes without the PC, or it was forgotten: everything off. */
    data object Ended : LinkEvent

    /** First step of the recovery ladder: restart [stream] at its source. */
    class RestartSource(val stream: Int) : LinkEvent

    data object KnownChanged : LinkEvent
}

/**
 * The Transporter (section 14): finds PCs, runs handshakes, keeps the active link and a warm standby, follows the PC's
 * SWITCH, recovers stalls and holds the session through drops (section 7.6). Every decision happens here, on the hub
 * dispatcher; sockets live on the attempt and session threads. [usbPlugged] says whether a cable to a computer is in.
 */
class LinkHub(
    private val store: Store,
    private val router: MediaRouter,
    private val bluetooth: Bluetooth?,
    private val wifi: WifiNetwork?,
    private val phoneName: () -> String,
    private val model: String,
    private val sdk: Int,
    private val thermal: () -> Int?,
    private val usbPlugged: () -> Boolean,
    private val emit: (LinkEvent) -> Unit,
    private val now: () -> Long = System::currentTimeMillis,
    seekerFactory: ((onFound: (Candidate) -> Unit) -> SeekerApi)? = null,
) : Hub<LinkMsg>("link") {
    private enum class Phase { SEARCHING, CONNECTING, APPROVING, ACTIVE, HELD, WAITING, DENIED, BUSY, UPDATE, KEY_CHANGED }

    private enum class Purpose { PRIMARY, STANDBY }

    private class Attempt(val id: Int, val candidate: Candidate, val purpose: Purpose, val resume: String?)

    private var phase = Phase.SEARCHING
    private var searchStartedAt = now()
    private var heldAt = 0L
    private var connection: Connection? = null
    private val seeker: SeekerApi = seekerFactory?.invoke { post(LinkMsg.Found(it)) }
        ?: Seeker(store.phoneId, phoneName, store::addresses, { wifi?.forHost(it) }) { post(LinkMsg.Found(it)) }
    private val candidates = LinkedHashMap<String, Pair<Candidate, Long>>()
    private val failures = HashMap<String, Pair<Int, Long>>()

    /** Links that died lately, by candidate key: not chosen for an upgrade until the time given (section 14.7). */
    private val probation = HashMap<String, Long>()
    private var attempt: Attempt? = null
    private var attemptIds = 0
    private var active: LinkSession? = null
    private var standby: LinkSession? = null
    private var pcId: String? = null
    private var pcName = ""
    private var heldSessionId: String? = null
    private var deniedPc: String? = null
    private var keyChangedPc: String? = null
    private var heldOffPc: String? = null
    private var allowed: Set<LinkKind> = LinkKind.entries.toSet()
    private var bluetoothTried = false
    private var bluetoothOffered = false
    private var tetherHinted = false

    /** Handshakes with one PC keep failing: since when, and when last (section 19, "Can't reach"). */
    private var unreachable: String? = null
    private var unreachableSince = 0L
    private var lastFailureAt = 0L
    private var speakerSince = 0L
    private var lastPing = 0L

    /** When the connected PC's last-connected time was last saved; it is saved again each [CONNECTED_SAVE_MS]. */
    private var lastConnectedSaved = 0L
    private var lastStandbyPing = 0L
    private val reconnector = Reconnector(now)

    init {
        scope.launch {
            while (isActive) {
                delay(TICK_MS)
                post(LinkMsg.Tick)
            }
        }
        setConnection(Connection.Searching())
        wifi?.listen { post(LinkMsg.Opened) }
    }

    /** A tick or a repeated discovery answer can go; a handed-over tunnel connection can't, or it would leak. */
    override fun droppable(message: LinkMsg) = message == LinkMsg.Tick || (message is LinkMsg.Found && message.candidate.opened == null)

    override fun handle(message: LinkMsg) {
        when (message) {
            is LinkMsg.Found -> found(message.candidate)
            LinkMsg.Tick -> tick()
            is LinkMsg.AttemptPending -> if (attempt?.id == message.attempt && attempt?.purpose == Purpose.PRIMARY) {
                phase = Phase.APPROVING
                setConnection(Connection.Approving(message.pc, message.code, attempt!!.candidate.link))
            }
            is LinkMsg.AttemptDone -> attemptDone(message)
            is LinkMsg.ControlIn -> control(message.session, message.message)
            is LinkMsg.Closed -> closed(message.session)
            is LinkMsg.MediaRecreated -> if (message.session === active && !message.session.closed) {
                val old = message.session.path
                message.session.path = message.path
                router.use(message.path)
                runCatching { old.carrier.close() }
            } else {
                runCatching { message.path.carrier.close() }
            }
            is LinkMsg.Send -> active?.send(message.message)
            is LinkMsg.Choose -> {
                heldOffPc = null
                fresh().firstOrNull { it.pcId == message.pcId }?.let { connect(it, Purpose.PRIMARY, null) }
            }
            LinkMsg.AskAgain -> deniedPc?.let {
                store.clearRefusal(it)
                deniedPc = null
                searching()
                decide()
            }
            is LinkMsg.Forget -> forget(message.pcId)
            is LinkMsg.Block -> block(message.pcId, message.name)
            is LinkMsg.Unblock -> {
                store.unblock(message.pcId)
                emit(LinkEvent.KnownChanged)
                if (phase == Phase.SEARCHING) decide()
            }
            LinkMsg.Reconnect -> reconnect()
            is LinkMsg.Allowed -> {
                val gained = (message.links - allowed).isNotEmpty()
                allowed = message.links
                candidates.entries.removeAll { (_, v) -> (v.first.link !in allowed).also { gone -> if (gone) drop(v.first) } }
                standby?.takeIf { it.link !in allowed }?.close()
                active?.takeIf { it.link !in allowed }?.let { s ->
                    // Let any in-flight Settings change reach the PC before closing the link.
                    later(DRAIN_MS) {
                        if (s === active && s.link !in allowed) {
                            if (standby?.let { !it.closed && it.link in allowed } == true) s.close() else endedByPeer(s, "quit")
                        }
                    }
                }
                if (gained) {
                    seeker.burst()
                    if (phase == Phase.ACTIVE) upgrade() else decide()
                }
            }
            LinkMsg.Opened -> {
                heldOffPc = null
                probation.clear()
                seeker.burst()
                if (phase == Phase.ACTIVE) upgrade() else decide()
            }
            is LinkMsg.SpeakerExpected -> speakerSince = if (message.on) now() else 0
            LinkMsg.TryBluetooth -> {
                heldOffPc = null
                bluetoothTried = true
                offerBluetooth(false)
                decide()
            }
        }
    }

    private fun key(c: Candidate) = "${c.pcId ?: "tunnel"}:${c.link}:${c.host?.hostAddress ?: c.btAddress ?: ""}"

    private fun found(c: Candidate) {
        if (c.link !in allowed) {
            drop(c)
            return
        }
        val k = key(c)
        if (c.link == LinkKind.USB_DEBUGGING && c.opened != null && (hasTunnel() || (failures[k]?.second ?: 0) > now())) {
            drop(c)
            return
        }
        val previous = candidates.put(k, c to now())?.first
        previous?.takeIf { it.opened !== c.opened }?.let(::drop)
        // The other phone left: try at once rather than after the busy backoff.
        if (previous?.busy == true && !c.busy) failures.remove(k)
        when (phase) {
            Phase.SEARCHING, Phase.HELD, Phase.WAITING, Phase.BUSY -> decide()
            Phase.ACTIVE -> upgrade()
            else -> Unit
        }
    }

    /** An unused tunnel connection from the Seeker is closed, and the Seeker may dial again. */
    private fun drop(c: Candidate) {
        val opened = c.opened ?: return
        runCatching { opened.close() }
        seeker.tunnelReleased()
    }

    private fun fresh(): List<Candidate> {
        val t = now()
        return candidates.values.filter { (c, seen) ->
            t - seen <= CANDIDATE_TTL_MS && c.link in allowed && (failures[key(c)]?.second ?: 0) <= t && c.pcId?.let(::isBlocked) != true
        }.map { it.first }
    }

    private fun isBlocked(id: String) = store.pc(id)?.blocked == true

    private fun addressOf(c: Candidate): String? = c.host?.hostAddress?.let { if (c.tcpPort > 0) "$it:${c.tcpPort}" else it } ?: c.btAddress

    private fun decide() {
        if (attempt != null) return
        var options = fresh()
        if (options.none { it.link != LinkKind.BLUETOOTH } && bluetoothUsable()) {
            val known = store.known()
            options = options + bluetooth!!.candidates(store.btAddresses()).map { c ->
                val pc = known.values.firstOrNull { store.pc(it.id)?.btAddress == c.btAddress?.uppercase() }
                if (pc != null) c.copy(pcId = pc.id, name = pc.name) else c
            }
        }
        // While holding, only the held PC (or the cable, which is its own proof of intent) will do.
        if (phase == Phase.HELD || phase == Phase.WAITING) options = options.filter { it.pcId == pcId || it.pcId == null }
        if (heldOffPc != null) options = options.filter { it.pcId != heldOffPc || it.link.isCable }
        when (val d = Ranker.decide(options, store.known(), now())) {
            is Decision.Connect -> {
                val resume = heldSessionId?.takeIf { d.candidate.pcId == pcId || d.candidate.pcId == null }
                connect(d.candidate, Purpose.PRIMARY, resume)
            }
            is Decision.Choose -> if (phase == Phase.SEARCHING) {
                emit(LinkEvent.Choices(d.candidates.mapNotNull { c -> c.pcId?.let { PcChoice(it, c.name, addressOf(c), c.link) } }))
                setConnection(Connection.Choosing(d.candidates.size))
            }
            Decision.Wait -> Unit
        }
    }

    /**
     * Bluetooth is the last resort: tried only when no IP link answers, 5 s into a search or a hold, so a quick Wi-Fi
     * or cable comeback never waits behind a slow RFCOMM dial. "Try Bluetooth" skips the wait while searching.
     */
    private fun bluetoothUsable(): Boolean {
        val b = bluetooth ?: return false
        if (LinkKind.BLUETOOTH !in allowed || !b.permitted()) return false
        val holding = phase == Phase.HELD || phase == Phase.WAITING
        return (bluetoothTried && !holding) || now() - (if (holding) heldAt else searchStartedAt) >= BLUETOOTH_AFTER_MS
    }

    private fun connect(c: Candidate, purpose: Purpose, resume: String?) {
        val a = Attempt(++attemptIds, c, purpose, resume)
        attempt = a
        // A tunnel connection the Seeker handed over now belongs to this attempt; expiring the candidate must not close it.
        if (c.opened != null) candidates.remove(key(c))
        if (purpose == Purpose.PRIMARY && phase != Phase.HELD && phase != Phase.WAITING) {
            phase = Phase.CONNECTING
            // While "Can't reach" shows for this PC, each retry would flash "Connecting": the message stays.
            if ((connection as? Connection.Searching)?.unreachable?.let { it == c.name } != true) {
                setConnection(Connection.Connecting(c.name.ifEmpty { null }, c.link))
            }
        }
        thread(name = "owlmic-connect", isDaemon = true) { run(a) }
    }

    /** On the attempt's own thread: dial, handshake, open the media carrier. */
    private fun run(a: Attempt) {
        val c = a.candidate
        var channel: ControlChannel? = null
        var session: LinkSession? = null
        val result = try {
            channel = when (c.link) {
                LinkKind.USB_DEBUGGING -> {
                    val pre = c.opened as? Socket
                    tcp(if (pre != null && !pre.isClosed) pre else dial(InetAddress.getLoopbackAddress(), Proto.PORT_CONTROL))
                }
                LinkKind.USB_TETHERING, LinkKind.WIFI -> tcp(dial(c.host ?: error("no address"), c.tcpPort))
                LinkKind.BLUETOOTH -> bluetooth!!.connect(c.btAddress ?: error("no address"), BLUETOOTH_DIAL_MS).let { s -> ControlChannel(s.inputStream, s.outputStream, s) }
            }
            val hs = Handshake(store.identity, store.phoneId, phoneName(), model, blocked = ::isBlocked, remembered = store::key)
            val r = hs.run(channel, c.link, a.resume, Watchdog.on(channel)) { pc, code -> post(LinkMsg.AttemptPending(a.id, pc, code)) }
            if (r is Handshake.Result.Welcomed) session = open(c, channel, r)
            r
        } catch (e: Exception) {
            Handshake.Result.Failed(e.message ?: e.javaClass.simpleName)
        }
        if (session == null) channel?.close()
        post(LinkMsg.AttemptDone(a.id, result, session, c.link == LinkKind.USB_DEBUGGING))
    }

    private fun dial(host: InetAddress, port: Int) = Socket().apply {
        wifi?.forHost(host)?.bindSocket(this)
        tcpNoDelay = true
        connect(InetSocketAddress(host, port), DIAL_MS)
    }

    private fun tcp(socket: Socket) = ControlChannel(socket.getInputStream(), socket.getOutputStream(), socket).also {
        it.writeChannelByte(Frames.CHANNEL_CONTROL)
    }

    /** Opens the media carrier for a welcomed handshake and greets the PC on it (protocol section 6, stream 0). */
    private fun open(c: Candidate, channel: ControlChannel, w: Handshake.Result.Welcomed): LinkSession {
        val wireless = c.link.isWireless
        if (wireless) channel.sealing = ControlSealing(w.keys.phoneToPc, w.keys.pcToPhone)
        val path = newPath(c, channel, w, Packetizer(if (wireless) Crypto.Sealing(w.keys.phoneToPc) else null), Unpacker(if (wireless) Crypto.Sealing(w.keys.pcToPhone) else null))
        return LinkSession(c, c.link, channel, w, path, c.hotspot)
    }

    private fun newPath(c: Candidate, channel: ControlChannel, w: Handshake.Result.Welcomed, out: Packetizer, inbound: Unpacker): MediaPath {
        var holder: MediaPath? = null
        val receive = { b: ByteArray, n: Int -> holder?.let { router.received(it, b, n) } ?: Unit }
        val carrier = when (c.link) {
            LinkKind.USB_DEBUGGING -> TunnelCarrier(Proto.PORT_CONTROL, receive) { holder?.let { p -> sessionOf(p)?.let { post(LinkMsg.Closed(it)) } } }
            LinkKind.USB_TETHERING, LinkKind.WIFI -> UdpCarrier(c.host!!, c.mediaPort, wifi?.forHost(c.host), receive)
            LinkKind.BLUETOOTH -> SharedStreamCarrier(channel) { holder?.let { p -> sessionOf(p)?.let { post(LinkMsg.Closed(it)) } } }
        }
        val path = MediaPath(carrier, out, inbound)
        holder = path
        val hello = out.carrierHello(w.sessionId, Crypto.carrierMac(w.keys.auth, w.sessionId))
        carrier.send(hello)
        if (carrier is UdpCarrier) {
            // UDP may drop one; send two spaced copies without blocking the connect thread.
            scope.launch {
                delay(100)
                runCatching { carrier.send(hello) }
                delay(200)
                runCatching { carrier.send(hello) }
            }
        }
        return path
    }

    private fun sessionOf(p: MediaPath) = listOfNotNull(active, standby).firstOrNull { it.path === p }

    private fun attemptDone(m: LinkMsg.AttemptDone) {
        val a = attempt
        if (a == null || a.id != m.attempt) {
            m.session?.close()
            if (m.tunnel) seeker.tunnelReleased()
            return
        }
        attempt = null
        val c = a.candidate
        when (val r = m.result) {
            is Handshake.Result.Welcomed -> {
                failures.remove(key(c))
                unreachable = null
                val s = m.session!!
                if (isBlocked(r.pcId)) {
                    // Blocked while the handshake ran: the session it opened says BYE and goes. Started, so its close
                    // comes back as Closed and a tunnel is released the usual way.
                    startReading(s)
                    s.send(Bye(BYE_REASON))
                    later(DRAIN_MS) { s.close() }
                    blockedAttempt(a)
                } else if (a.purpose == Purpose.PRIMARY) {
                    store.remember(r.pcId, r.pcName, r.pcStaticPub, r.pairingKey, r.btAddr, addressOf(s), s.link)
                    val resumed = (phase == Phase.HELD || phase == Phase.WAITING) && r.pcId == pcId
                    activate(s, resumed)
                } else if (active != null && r.pcId == pcId && s.link != active!!.link) {
                    // The new link waits here, proven, for the PC's SWITCH (protocol section 4, "Links of one session").
                    val prev = standby
                    standby = s
                    startReading(s)
                    prev?.close()
                } else {
                    s.close()
                    if (m.tunnel) releaseTunnelIfIdle()
                }
            }
            is Handshake.Result.Rejected -> {
                if (c.link == LinkKind.USB_DEBUGGING) releaseTunnelIfIdle()
                unreachable = null
                if (a.purpose == Purpose.STANDBY) return
                val name = r.pcName ?: c.name
                when (r.reason) {
                    RejectReason.DENIED, RejectReason.BLOCKED -> {
                        r.pcId?.let { store.refused(it, name) }
                        deniedPc = r.pcId
                        phase = Phase.DENIED
                        setConnection(Connection.Denied(name))
                        emit(LinkEvent.KnownChanged)
                    }
                    RejectReason.BUSY -> {
                        backoff(c, BUSY_RETRY_MS)
                        phase = Phase.BUSY
                        setConnection(Connection.Busy(name, r.owner.orEmpty()))
                    }
                    RejectReason.VERSION -> {
                        phase = Phase.UPDATE
                        setConnection(Connection.UpdateNeeded(name, phoneOutdated = (r.pcProto ?: 0) > Proto.VERSION))
                    }
                }
            }
            is Handshake.Result.KeyChanged -> {
                if (c.link == LinkKind.USB_DEBUGGING) seeker.tunnelReleased()
                backoff(c, KEY_CHANGED_RETRY_MS)
                if (a.purpose == Purpose.STANDBY || phase == Phase.HELD || phase == Phase.WAITING) return
                keyChangedPc = r.pcId
                phase = Phase.KEY_CHANGED
                setConnection(Connection.KeyChanged(r.pcName))
            }
            is Handshake.Result.Blocked -> {
                if (c.link == LinkKind.USB_DEBUGGING) releaseTunnelIfIdle()
                blockedAttempt(a)
            }
            is Handshake.Result.Failed -> {
                if (c.link == LinkKind.USB_DEBUGGING) releaseTunnelIfIdle()
                backoff(c, null)
                if (a.purpose == Purpose.STANDBY) {
                    probation[key(c)] = now() + PROBATION_MS
                } else if (phase != Phase.HELD && phase != Phase.WAITING) {
                    failing(c)
                    searching(fresh = false)
                }
            }
        }
        if (attempt == null) {
            when (phase) {
                Phase.ACTIVE -> upgrade()
                Phase.SEARCHING, Phase.HELD, Phase.WAITING -> decide()
                else -> Unit
            }
        }
    }

    /** An attempt that reached a blocked PC: its way in waits a long while, and the search goes on without it. */
    private fun blockedAttempt(a: Attempt) {
        backoff(a.candidate, BLOCKED_RETRY_MS)
        if (a.purpose == Purpose.PRIMARY && phase != Phase.HELD && phase != Phase.WAITING) searching(fresh = false)
    }

    /** Remembers that handshakes with [c]'s PC keep failing, for "Can't reach" after 10 s. */
    private fun failing(c: Candidate) {
        val t = now()
        val name = c.name.ifEmpty { return }
        if (unreachable != name || t - lastFailureAt > CANDIDATE_TTL_MS) {
            unreachable = name
            unreachableSince = t
        }
        lastFailureAt = t
    }

    /** A failed candidate waits 0.5 s, then 1 s, then 2 s before it is tried again (section 14.5). */
    private fun backoff(c: Candidate, fixedMs: Long?) {
        val k = key(c)
        val count = (failures[k]?.first ?: 0) + 1
        val delay = fixedMs ?: RETRY_MS[minOf(count - 1, RETRY_MS.lastIndex)]
        failures[k] = count to now() + delay
    }

    private fun activate(s: LinkSession, resumed: Boolean) {
        val w = s.welcome
        active = s
        pcId = w.pcId
        pcName = w.pcName
        heldSessionId = null
        deniedPc = null
        keyChangedPc = null
        phase = Phase.ACTIVE
        reconnector.recovered()
        router.use(s.path)
        startReading(s)
        pace()
        lastPing = 0
        lastConnectedSaved = now()
        // Connected first: the App Hub answers Welcomed with STATE and STREAM_STARTs, which go out only while connected.
        setConnection(Connection.Connected(w.pcName, s.link, s.hotspot, address = addressOf(s)))
        emit(LinkEvent.Welcomed(w.pcId, w.pcName, s.link, w.settings, resumed))
        emit(LinkEvent.Choices(emptyList()))
        emit(LinkEvent.KnownChanged)
        offerBluetooth(false)
        hintTethering(false)
        if (s.link != LinkKind.USB_DEBUGGING) seeker.burst()
    }

    private fun startReading(s: LinkSession) {
        s.start(router, onControl = { post(LinkMsg.ControlIn(s, it)) }, onClosed = { post(LinkMsg.Closed(s)) })
    }

    private fun pace() {
        seeker.pace = when (active?.link) {
            null -> Seeker.Pace.SEARCHING
            LinkKind.USB_DEBUGGING -> Seeker.Pace.CONNECTED_BY_USB_DEBUGGING
            else -> Seeker.Pace.CONNECTED
        }
    }

    private fun hasTunnel(): Boolean =
        active?.link == LinkKind.USB_DEBUGGING ||
            standby?.link == LinkKind.USB_DEBUGGING ||
            attempt?.candidate?.link == LinkKind.USB_DEBUGGING

    private fun releaseTunnelIfIdle() {
        if (!hasTunnel()) seeker.tunnelReleased()
    }

    private fun closed(s: LinkSession) {
        s.close()
        if (s.link == LinkKind.USB_DEBUGGING) releaseTunnelIfIdle()
        if (s === active || s === standby) {
            probation[key(s.candidate)] = now() + if (s.link.isCable) CABLE_PROBATION_MS else PROBATION_MS
        }
        if (s === standby) {
            standby = null
            return
        }
        if (s !== active) return
        active = null
        val next = standby?.takeIf { !it.closed }
        standby = null
        if (next != null) {
            promote(next)
            return
        }
        router.use(null)
        hold(s)
    }

    private fun promote(s: LinkSession) {
        active = s
        // Its last answer may be a standby heartbeat old; the active link's stricter deadline starts now.
        s.monitor.heard()
        router.use(s.path)
        pace()
        setConnection(Connection.Connected(pcName, s.link, s.hotspot, address = addressOf(s)))
        pcId?.let { store.connectedVia(it, addressOf(s), s.link) }
        emit(LinkEvent.Switched(s.link))
        emit(LinkEvent.KnownChanged)
    }

    private fun addressOf(s: LinkSession): String? = addressOf(s.candidate)

    private fun endedByPeer(s: LinkSession, reason: String?) {
        if (s === standby) {
            standby = null
            s.close()
            if (s.link == LinkKind.USB_DEBUGGING) releaseTunnelIfIdle()
            return
        }
        if (s !== active) {
            s.close()
            if (s.link == LinkKind.USB_DEBUGGING) releaseTunnelIfIdle()
            return
        }
        val goneId = pcId
        active = null
        standby?.close()
        standby = null
        s.close()
        if (s.link == LinkKind.USB_DEBUGGING) releaseTunnelIfIdle()
        goneId?.let(store::connectedNow)
        router.use(null)
        heldSessionId = null
        pcId = null
        if (reason == "disconnect") heldOffPc = goneId
        emit(LinkEvent.Ended)
        searching()
    }

    private fun hold(s: LinkSession) {
        // Connected until now: the list's "last connected" reads from the moment the link went.
        pcId?.let(store::connectedNow)
        heldSessionId = s.sessionId.toHex()
        phase = Phase.HELD
        heldAt = now()
        pace()
        seeker.burst()
        emit(LinkEvent.Held)
        setConnection(Connection.Reconnecting(pcName, s.link))
    }

    private fun control(s: LinkSession, m: Message) {
        s.monitor.heard()
        if (s !== active && s !== standby) return
        when (m) {
            is Ping -> s.send(Pong(m.t))
            is Pong -> s.monitor.pong(m.t)
            // The PC sends SWITCH on the link it moves to: the standby. Media moves there; the old link stays as the
            // standby, so packets already on their way over it still arrive (the drain of section 14.7).
            is Switch -> if (s === standby && !s.closed && LinkKind.of(m.link) == s.link) {
                standby = active
                promote(s)
            }
            is Bye -> endedByPeer(s, m.reason)
            is RestartStream -> if (s === active) ladder(m.stream)
            is Report -> {
                s.monitor.peerReport(m.lossPct)
                if (s === active) emit(LinkEvent.Control(m))
            }
            is State, is Settings, is StreamStart, is StreamStop, KeyframeRequest -> if (s === active) emit(LinkEvent.Control(m))
            else -> Unit
        }
    }

    /** One step of the recovery ladder for a stalled stream (section 14.8). */
    private fun ladder(stream: Int) {
        val s = active ?: return
        when (reconnector.stall()) {
            Reconnector.Step.RESTART_SOURCE -> {
                emit(LinkEvent.RestartSource(stream))
                if (stream == Stream.SPEAKER) s.send(RestartStream(stream))
            }
            Reconnector.Step.RECREATE_MEDIA -> recreateMedia(s)
            Reconnector.Step.REHANDSHAKE, Reconnector.Step.SWITCH_LINK, Reconnector.Step.HOLD -> s.close()
        }
    }

    private fun recreateMedia(s: LinkSession) {
        thread(name = "owlmic-recreate", isDaemon = true) {
            val path = runCatching { newPath(s.candidate, s.channel, s.welcome, s.path.out, s.path.inbound) }.getOrNull() ?: return@thread
            post(LinkMsg.MediaRecreated(s, path))
        }
    }

    private fun tick() {
        val t = now()
        candidates.entries.removeAll { (_, v) -> (t - v.second > CANDIDATE_TTL_MS).also { stale -> if (stale) drop(v.first) } }
        probation.entries.removeAll { it.value <= t }
        if (heldOffPc != null && fresh().none { it.pcId == heldOffPc }) heldOffPc = null
        when (phase) {
            Phase.SEARCHING, Phase.BUSY, Phase.DENIED, Phase.KEY_CHANGED -> {
                if (phase == Phase.SEARCHING && connection !is Connection.Choosing) setConnection(searchState(t))
                maybeOfferBluetooth(t)
                hintTethering(phase == Phase.SEARCHING && tetherHint(usbPlugged(), LinkKind.USB_TETHERING in allowed, probeInterfaces()))
                decide()
            }
            Phase.HELD -> {
                if (t - heldAt >= HOLD_MS) {
                    phase = Phase.WAITING
                    emit(LinkEvent.Release)
                    setConnection(Connection.Waiting(pcName))
                }
                decide()
            }
            Phase.WAITING -> if (t - heldAt >= WAIT_MS) {
                heldSessionId = null
                emit(LinkEvent.Ended)
                searching()
            } else {
                decide()
            }
            Phase.ACTIVE -> heartbeat(t)
            Phase.CONNECTING, Phase.APPROVING, Phase.UPDATE -> Unit
        }
    }

    private fun heartbeat(t: Long) {
        val a = active ?: return
        // A dead link, or one whose seq is about to wrap: close it, and the session moves on with fresh keys.
        if (a.monitor.dead() || a.path.out.wornOut) {
            a.close()
            return
        }
        if (t - lastPing >= Monitor.HEARTBEAT_MS) {
            lastPing = t
            a.send(Ping(Monitor.nowUs()))
            a.monitor.report(router.speakerStats, thermal(), fast = speakerSince > 0)?.let(a::send)
        }
        if (t - lastConnectedSaved >= CONNECTED_SAVE_MS) {
            lastConnectedSaved = t
            pcId?.let(store::connectedNow)
        }
        (connection as? Connection.Connected)?.let { c ->
            val weak = a.monitor.weak()
            if (c.weak != weak) setConnection(c.copy(weak = weak))
        }
        standby?.let { sb ->
            if (sb.monitor.dead(Monitor.STANDBY_HEARTBEAT_MS)) {
                sb.close()
            } else if (t - lastStandbyPing >= Monitor.STANDBY_HEARTBEAT_MS) {
                lastStandbyPing = t
                sb.send(Ping(Monitor.nowUs()))
            }
        }
        // The speaker stream must keep arriving while it is on.
        if (speakerSince > 0 && t - speakerSince > SPEAKER_STALL_MS) {
            val last = router.speakerStats.lastArrivalUs
            if (last == 0L || Monitor.nowUs() - last > SPEAKER_STALL_MS * 1_000) {
                speakerSince = t
                ladder(Stream.SPEAKER)
            }
        }
        upgrade()
    }

    /** Opens a better link, or a warm Wi-Fi standby while on a cable (section 14.7). The PC decides when to switch. */
    private fun upgrade() {
        if (attempt != null) return
        val a = active ?: return
        val t = now()
        val same = fresh().filter { (it.pcId == pcId || (it.pcId == null && it.link == LinkKind.USB_DEBUGGING)) && (probation[key(it)] ?: 0) <= t }
        val better = same.filter { it.link.ordinal < a.link.ordinal && standby?.link != it.link }.minByOrNull { it.link.ordinal }
        val warm = if (a.link.isCable && standby == null) same.firstOrNull { it.link == LinkKind.WIFI } else null
        (better ?: warm)?.let { connect(it, Purpose.STANDBY, a.sessionId.toHex()) }
    }

    private fun maybeOfferBluetooth(t: Long) {
        val b = bluetooth ?: return
        val show = sdk >= 31 && !b.permitted() && LinkKind.BLUETOOTH in allowed && t - searchStartedAt >= OFFER_BLUETOOTH_AFTER_MS &&
            fresh().isEmpty() && store.known().values.any { it.approved }
        offerBluetooth(show)
    }

    private fun offerBluetooth(show: Boolean) {
        if (show != bluetoothOffered) {
            bluetoothOffered = show
            emit(LinkEvent.BluetoothOffer(show))
        }
    }

    private fun hintTethering(show: Boolean) {
        if (show != tetherHinted) {
            tetherHinted = show
            emit(LinkEvent.TetherHint(show))
        }
    }

    private fun forget(id: String) {
        store.forget(id)
        failures.keys.removeAll { it.startsWith("$id:") }
        if (pcId == id) {
            // BYE goes out first; the link closes once it has had time to leave.
            active?.let { s ->
                s.send(Bye(BYE_REASON))
                later(DRAIN_MS) { s.close() }
            }
            standby?.close()
            active = null
            standby = null
            router.use(null)
            pcId = null
            heldSessionId = null
            emit(LinkEvent.Ended)
            searching()
        } else if (id == keyChangedPc || id == deniedPc) {
            keyChangedPc = null
            deniedPc = null
            searching()
        }
        emit(LinkEvent.KnownChanged)
    }

    /**
     * Blocks [id]: its candidates go, a choice listing it is made again without it, and if it is the connected PC the
     * session ends with BYE, as when it is forgotten, and the phone looks for another.
     */
    private fun block(id: String, name: String) {
        if (pcId == id && active != null) store.connectedNow(id)
        val answered = candidates.values.firstOrNull { it.first.pcId == id }?.first
        store.block(id, name, answered?.let(::addressOf), answered?.link)
        candidates.entries.removeAll { (_, v) -> (v.first.pcId == id).also { gone -> if (gone) drop(v.first) } }
        // A handshake with it in flight is forgotten; its session is closed when it lands.
        if (attempt?.candidate?.pcId == id) {
            attempt = null
            if (phase == Phase.CONNECTING || phase == Phase.APPROVING) searching(fresh = false)
        }
        if (pcId == id) {
            active?.let { s ->
                s.send(Bye(BYE_REASON))
                later(DRAIN_MS) { s.close() }
            }
            standby?.close()
            active = null
            standby = null
            router.use(null)
            pcId = null
            heldSessionId = null
            emit(LinkEvent.Ended)
            searching()
        } else if (id == keyChangedPc || id == deniedPc) {
            keyChangedPc = null
            deniedPc = null
            searching()
        } else if (connection is Connection.Choosing) {
            emit(LinkEvent.Choices(emptyList()))
            searching(fresh = false)
        }
        emit(LinkEvent.KnownChanged)
        if (phase == Phase.SEARCHING) decide()
    }

    /**
     * The user's hard reconnect (the Reconnect button). Connected: the links close, as a dead link would, so the session
     * is held and dialled again within a tick and resumes with fresh keys. Looking, held or turned away: every wait is
     * cleared and the search starts over now. A handshake already running is left to finish.
     */
    private fun reconnect() {
        heldOffPc = null
        failures.clear()
        probation.clear()
        unreachable = null
        lastFailureAt = 0
        when (phase) {
            Phase.ACTIVE -> {
                standby?.close()
                active?.close()
                return
            }
            Phase.CONNECTING, Phase.APPROVING -> {
                seeker.burst()
                return
            }
            Phase.DENIED -> deniedPc?.let(store::clearRefusal)
            Phase.HELD, Phase.WAITING, Phase.SEARCHING, Phase.BUSY, Phase.UPDATE, Phase.KEY_CHANGED -> Unit
        }
        deniedPc = null
        if (phase != Phase.HELD && phase != Phase.WAITING) searching()
        seeker.burst()
        decide()
    }

    /** Back to looking. [fresh] restarts the clock for the "not found" messages and the Bluetooth offer. */
    private fun searching(fresh: Boolean = true) {
        phase = Phase.SEARCHING
        if (fresh) searchStartedAt = now()
        pace()
        setConnection(searchState(now()))
    }

    /** What the status says while looking: how long it has been, and a PC whose handshakes have failed for 10 s. */
    private fun searchState(t: Long): Connection.Searching {
        val elapsed = t - searchStartedAt
        val stage = when {
            elapsed >= HELP_AFTER_MS -> SearchStage.HELP
            elapsed >= NOT_FOUND_AFTER_MS -> SearchStage.NOT_FOUND
            else -> SearchStage.SEARCHING
        }
        val cantReach = unreachable?.takeIf { t - unreachableSince >= CANT_REACH_AFTER_MS && t - lastFailureAt <= CANDIDATE_TTL_MS }
        return Connection.Searching(stage, cantReach)
    }

    private fun setConnection(c: Connection) {
        if (c == connection) return
        connection = c
        emit(LinkEvent.ConnectionChanged(c))
    }

    /** After a failure: the attempt in flight is forgotten (its result is closed when it lands) and a lost active link is let go. */
    override fun restart() {
        attempt = null
        val a = active
        if (a != null && a.closed) closed(a)
        if (phase == Phase.CONNECTING || phase == Phase.APPROVING) searching(fresh = false)
    }

    override fun close() {
        if (active != null) pcId?.let(store::connectedNow)
        seeker.close()
        wifi?.close()
        active?.close()
        standby?.close()
        router.use(null)
        super.close()
    }

    private companion object {
        const val TICK_MS = 1_000L
        const val CANDIDATE_TTL_MS = 5_000L
        const val DIAL_MS = 2_000
        const val BLUETOOTH_DIAL_MS = 4_000L
        const val DRAIN_MS = 200L
        const val CABLE_PROBATION_MS = 1_000L
        const val PROBATION_MS = 10_000L
        const val CANT_REACH_AFTER_MS = 10_000L
        const val KEY_CHANGED_RETRY_MS = 30_000L
        const val BLOCKED_RETRY_MS = 60_000L
        const val CONNECTED_SAVE_MS = 60_000L

        /** BYE's reason is free text; the PC treats a block like being forgotten. */
        const val BYE_REASON = "forgotten"
        val RETRY_MS = longArrayOf(500, 1_000, 2_000)
        // The PC takes at most 4 handshakes a minute from one address (section 15.5).
        const val BUSY_RETRY_MS = 15_000L
        const val NOT_FOUND_AFTER_MS = 8_000L
        const val HELP_AFTER_MS = 15_000L
        const val BLUETOOTH_AFTER_MS = 5_000L
        const val OFFER_BLUETOOTH_AFTER_MS = 10_000L
        const val HOLD_MS = 30_000L
        const val WAIT_MS = 5 * 60_000L
        const val SPEAKER_STALL_MS = 1_000L
    }
}
