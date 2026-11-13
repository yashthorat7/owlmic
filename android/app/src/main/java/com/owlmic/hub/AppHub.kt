package com.owlmic.hub

import android.app.Application
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.media.audiofx.NoiseSuppressor
import android.os.BatteryManager
import android.os.Build
import android.provider.Settings.Global
import com.owlmic.Owlmic
import com.owlmic.core.design.SettingsModel
import com.owlmic.core.hub.AppState
import com.owlmic.core.hub.Connection
import com.owlmic.core.hub.Feature
import com.owlmic.core.hub.FeaturePhase
import com.owlmic.core.hub.FeatureProblem
import com.owlmic.core.hub.FeatureView
import com.owlmic.core.hub.Health
import com.owlmic.core.hub.Hub
import com.owlmic.core.hub.KnownPcView
import com.owlmic.core.hub.LinkKind
import com.owlmic.core.hub.SpeakerOutput
import com.owlmic.core.hub.worst
import com.owlmic.core.link.Bluetooth
import com.owlmic.core.link.LinkEvent
import com.owlmic.core.link.LinkHub
import com.owlmic.core.link.LinkMsg
import com.owlmic.core.link.MediaRouter
import com.owlmic.core.link.PcMemory
import com.owlmic.core.link.WifiNetwork
import com.owlmic.core.proto.FeatureState
import com.owlmic.core.proto.KeyframeRequest
import com.owlmic.core.proto.Report
import com.owlmic.core.proto.State
import com.owlmic.core.proto.Stream
import com.owlmic.core.proto.StreamStart
import com.owlmic.core.proto.StreamStop
import com.owlmic.core.settings.KeystoreWrapper
import com.owlmic.core.settings.SettingValues
import com.owlmic.core.settings.SettingsEvent
import com.owlmic.core.settings.SettingsHub
import com.owlmic.core.settings.SettingsMsg
import com.owlmic.core.settings.Store
import com.owlmic.media.MediaEvent
import com.owlmic.media.MediaHub
import com.owlmic.media.MediaMsg
import com.owlmic.media.audio.MicCodec
import com.owlmic.media.audio.MicConfig
import com.owlmic.media.audio.SpeakerFormat
import com.owlmic.media.camera.CameraConfig
import com.owlmic.media.camera.Lens
import com.owlmic.media.camera.Orientation
import com.owlmic.media.camera.Ratio
import com.owlmic.media.camera.VideoPlan
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import org.json.JSONObject
import java.io.File
import com.owlmic.core.proto.Settings as SettingsMessage

sealed interface AppMsg {
    /** First message: opens the store and starts the Link and Settings hubs, off the main thread. */
    data object Boot : AppMsg

    /** Once a second: the hubs' health into AppState (section 11.2, rule 4). */
    data object Tick : AppMsg

    /** Turn a feature on or off. The service has already made itself foreground with the right types. */
    class SetFeature(val feature: Feature, val on: Boolean) : AppMsg

    /** The permission a feature needs was refused: its tile says so (section 8.1). */
    class PermissionMissing(val feature: Feature) : AppMsg

    /** The notification's Pause mic or Resume mic. */
    data object ToggleMicPause : AppMsg

    /** A tap on a paused tile: the user takes back what the PC paused. */
    class Resume(val feature: Feature) : AppMsg

    data object StopAll : AppMsg

    data object FlipLens : AppMsg

    class Setting(val id: String, val value: String) : AppMsg

    class ChoosePc(val id: String) : AppMsg

    data object AskAgain : AppMsg

    class AddAddress(val address: String) : AppMsg

    class RemoveAddress(val address: String) : AppMsg

    class Forget(val id: String) : AppMsg

    /** The connection list's Block: never connect to this PC again until it is unblocked. */
    class Block(val id: String, val name: String) : AppMsg

    class Unblock(val id: String) : AppMsg

    /** The connection list's Reconnect: drop the link and dial again, or search again without waiting. */
    data object Reconnect : AppMsg

    data object TryBluetooth : AppMsg

    data object Opened : AppMsg

    class Output(val output: SpeakerOutput) : AppMsg

    class Thermal(val status: Int) : AppMsg

    /** The activity asked Android for [permission]. */
    class Asked(val permission: String) : AppMsg

    class FromLink(val event: LinkEvent) : AppMsg

    class FromSettings(val event: SettingsEvent) : AppMsg

    class FromMedia(val event: MediaEvent) : AppMsg
}

/**
 * The App Hub (section 11.1): owns the Link, Settings and Media hubs, turns their events into one [AppState], and
 * carries out the feature rules: what turns on when, what pauses during a drop and what comes back (section 7.6).
 * The store (file and Keystore) opens on the hub thread at [AppMsg.Boot], never on the main thread.
 */
class AppHub(private val app: Application, private val onState: (AppState) -> Unit) : Hub<AppMsg>("app") {
    private val router = MediaRouter()
    private var thermal: Int? = null

    private lateinit var store: Store
    private lateinit var link: LinkHub
    private lateinit var settings: SettingsHub
    private var booted = false
    val media = MediaHub(app, router, Owlmic.mutableLevel) { post(AppMsg.FromMedia(it)) }

    private var state = AppState(settings = SettingValues.plain(SettingValues.defaults()))
    private var lastPublish = 0L
    private var publishPending = false
    private var speakerExpected = false
    private var linkKind: LinkKind? = null
    private var pcId: String? = null
    private var waiting = false
    private var micPausedByPc = false
    private var micPausedByUser = false
    private var cameraPausedByPc = false
    private var speakerPausedByPc = false
    private var speakerFormat: SpeakerFormat? = null
    private var cameraFormat: Triple<Int, Int, Int>? = null

    init {
        post(AppMsg.Boot)
        scope.launch {
            while (isActive) {
                delay(HEALTH_EVERY_MS)
                post(AppMsg.Tick)
            }
        }
    }

    private fun boot() {
        store = Store(File(app.filesDir, "owlmic.json"), KeystoreWrapper())
        link = LinkHub(
            store, router, Bluetooth(app), WifiNetwork(app),
            phoneName = { phoneName(app) },
            model = Build.MODEL,
            sdk = Build.VERSION.SDK_INT,
            thermal = { thermal },
            usbPlugged = { usbPlugged(app) },
            emit = { post(AppMsg.FromLink(it)) },
        )
        settings = SettingsHub(store) { post(AppMsg.FromSettings(it)) }
        booted = true
        state = state.copy(manualAddresses = store.addresses(), askedPermissions = store.asked())
        knownChanged()
    }

    /** A Keystore that failed at boot is tried again with the usual backoff. */
    override fun restart() {
        if (!booted) boot()
    }

    override fun droppable(message: AppMsg) = message == AppMsg.Tick

    override fun handle(message: AppMsg) {
        if (message == AppMsg.Boot) {
            if (!booted) boot()
            publish()
            return
        }
        // Until the store opens there is nothing to connect to; the service sees the unchanged state and lets go.
        if (!booted) return
        when (message) {
            AppMsg.Boot -> Unit
            AppMsg.Tick -> state = state.copy(health = worst(listOf(health(), link.health(), settings.health(), media.health())))
            is AppMsg.SetFeature -> setFeature(message.feature, message.on)
            is AppMsg.PermissionMissing -> set(message.feature, FeatureView(FeaturePhase.OFF, FeatureProblem.PERMISSION))
            AppMsg.ToggleMicPause -> when {
                state.mic.phase == FeaturePhase.PAUSED && state.connected -> resume(Feature.MIC)
                state.mic.isOn -> {
                    micPausedByUser = true
                    applyMicPause()
                    sendState()
                }
            }
            is AppMsg.Resume -> if (state.connected) resume(message.feature)
            AppMsg.StopAll -> Feature.entries.forEach { if (state.feature(it).phase != FeaturePhase.OFF) setFeature(it, false) }
            AppMsg.FlipLens -> settings.post(SettingsMsg.Change("camera.lens", if (state.settings["camera.lens"] == "front") "back" else "front"))
            is AppMsg.Setting -> settings.post(SettingsMsg.Change(message.id, message.value))
            is AppMsg.ChoosePc -> link.post(LinkMsg.Choose(message.id))
            AppMsg.AskAgain -> link.post(LinkMsg.AskAgain)
            is AppMsg.AddAddress -> {
                store.addAddress(message.address)
                state = state.copy(manualAddresses = store.addresses())
                link.post(LinkMsg.Opened)
            }
            is AppMsg.RemoveAddress -> {
                store.removeAddress(message.address)
                state = state.copy(manualAddresses = store.addresses())
            }
            is AppMsg.Forget -> link.post(LinkMsg.Forget(message.id))
            is AppMsg.Block -> link.post(LinkMsg.Block(message.id, message.name))
            is AppMsg.Unblock -> link.post(LinkMsg.Unblock(message.id))
            AppMsg.Reconnect -> link.post(LinkMsg.Reconnect)
            AppMsg.TryBluetooth -> link.post(LinkMsg.TryBluetooth)
            AppMsg.Opened -> link.post(LinkMsg.Opened)
            is AppMsg.Output -> state = state.copy(speakerOutput = message.output)
            is AppMsg.Thermal -> {
                thermal = message.status
                media.post(MediaMsg.Thermal(message.status))
            }
            is AppMsg.Asked -> {
                store.markAsked(message.permission)
                state = state.copy(askedPermissions = store.asked())
            }
            is AppMsg.FromLink -> fromLink(message.event)
            is AppMsg.FromSettings -> fromSettings(message.event)
            is AppMsg.FromMedia -> fromMedia(message.event)
        }
        expectSpeaker()
        publish()
    }

    // Features.

    private fun setFeature(f: Feature, on: Boolean) {
        if (on) {
            if (!state.connected || state.feature(f).isOn) return
            when (f) {
                Feature.MIC -> {
                    set(f, FeatureView(FeaturePhase.STARTING))
                    startMic()
                    if (state.speaker.isOn) startSpeaker()
                }
                Feature.CAMERA -> {
                    if (linkKind == LinkKind.BLUETOOTH) {
                        set(f, FeatureView(FeaturePhase.OFF, FeatureProblem.NO_BLUETOOTH))
                        return
                    }
                    set(f, FeatureView(FeaturePhase.STARTING))
                    media.post(MediaMsg.Camera(cameraConfig()))
                }
                Feature.SPEAKER -> {
                    set(f, FeatureView(FeaturePhase.STARTING))
                    startSpeaker()
                    if (state.mic.isOn) startMic()
                }
            }
        } else {
            set(f, FeatureView())
            when (f) {
                Feature.MIC -> {
                    micPausedByPc = false
                    micPausedByUser = false
                    media.post(MediaMsg.Mic(null))
                    link.post(LinkMsg.Send(StreamStop(Stream.MIC)))
                    if (state.speaker.isOn) startSpeaker()
                }
                Feature.CAMERA -> {
                    cameraPausedByPc = false
                    media.post(MediaMsg.Camera(null))
                    cameraFormat = null
                    link.post(LinkMsg.Send(StreamStop(Stream.CAMERA)))
                }
                Feature.SPEAKER -> {
                    speakerPausedByPc = false
                    media.post(MediaMsg.Speaker(null, false, true))
                    // Nothing plays now: the mic leaves the voice path its echo canceller needed.
                    if (running(state.mic)) media.post(MediaMsg.Mic(micConfig()))
                }
            }
        }
        sendState()
    }

    /**
     * The Link Hub watches the speaker stream for stalls only while it should be flowing: the Speaker is on, not paused,
     * and the PC has started its stream (section 17.4).
     */
    private fun expectSpeaker() {
        val expected = booted && state.speaker.isOn && speakerFormat != null
        if (expected != speakerExpected) {
            speakerExpected = expected
            link.post(LinkMsg.SpeakerExpected(expected))
        }
    }

    /** Whoever paused it, the user's resume wins, and the PC hears "on" again. */
    private fun resume(f: Feature) {
        when (f) {
            Feature.MIC -> {
                micPausedByPc = false
                micPausedByUser = false
                applyMicPause()
            }
            Feature.CAMERA -> {
                // Paused because the link is Bluetooth: only an IP link brings it back.
                if (state.camera.problem == FeatureProblem.NO_BLUETOOTH) return
                cameraPausedByPc = false
                media.post(MediaMsg.CameraPaused(false))
            }
            Feature.SPEAKER -> speakerPausedByPc = false
        }
        if (state.feature(f).phase == FeaturePhase.PAUSED) set(f, FeatureView(FeaturePhase.ON))
        sendState()
    }

    private fun micConfig() = MicConfig.of(MicCodec.forLink(linkKind ?: LinkKind.WIFI), state.speaker.isOn, state.settings)

    private fun startMic() {
        val c = micConfig()
        media.post(MediaMsg.Mic(c))
        // Without a platform noise suppressor the PC runs its own at the default noise reduction level.
        val params = JSONObject().put("sampleRate", 48_000).put("channels", 1).put("frameMs", c.codec.frameMs)
            .put("noiseSuppressor", NoiseSuppressor.isAvailable())
        link.post(LinkMsg.Send(StreamStart(Stream.MIC, c.codec.wire, params)))
    }

    private fun startSpeaker() {
        val f = speakerFormat ?: return
        // With voice isolation the speaker plays as a call, so the mic's echo canceller hears it.
        media.post(MediaMsg.Speaker(f, state.mic.isOn && MicConfig.isolates(state.settings), linkKind?.isWireless != false))
    }

    private fun cameraConfig(): CameraConfig {
        val s = state.settings
        val fps = s["camera.frameRate"]?.toIntOrNull() ?: 24
        return CameraConfig(
            lens = if ((s["camera.lens"] ?: SettingsModel.find("camera.lens")?.default) == "back") Lens.BACK else Lens.FRONT,
            plan = VideoPlan.of(s["camera.quality"] ?: "auto", fps, linkKind ?: LinkKind.WIFI),
            orientation = when (s["camera.orientation"]) {
                "landscape" -> Orientation.LANDSCAPE
                "portrait" -> Orientation.PORTRAIT
                else -> Orientation.AUTO
            },
            ratio = Ratio.of(s["camera.ratio"]),
            mirrorPreview = (s["camera.mirrorPreview"] ?: SettingsModel.find("camera.mirrorPreview")?.default) == "on",
        )
    }

    /** The caller tells the PC. */
    private fun applyMicPause() {
        val paused = micPausedByPc || micPausedByUser
        media.post(MediaMsg.MicPaused(paused))
        if (state.mic.phase != FeaturePhase.OFF) set(Feature.MIC, FeatureView(if (paused) FeaturePhase.PAUSED else FeaturePhase.ON))
    }

    /** The feature's hardware is in use: not off, not failed, not let go during a long drop or on Bluetooth. */
    private fun running(v: FeatureView) =
        v.phase != FeaturePhase.OFF && v.phase != FeaturePhase.FAILED && v.problem != FeatureProblem.NO_BLUETOOTH && !waiting

    private fun set(f: Feature, v: FeatureView) {
        state = when (f) {
            Feature.MIC -> state.copy(mic = v)
            Feature.CAMERA -> state.copy(camera = v)
            Feature.SPEAKER -> state.copy(speaker = v)
        }
    }

    private fun wire(v: FeatureView) = when (v.phase) {
        FeaturePhase.OFF, FeaturePhase.FAILED -> FeatureState.OFF
        FeaturePhase.PAUSED -> FeatureState.PAUSED
        else -> FeatureState.ON
    }

    private fun sendState() {
        if (state.connected) link.post(LinkMsg.Send(State(wire(state.mic), wire(state.camera), wire(state.speaker))))
    }

    // Link.

    private fun fromLink(e: LinkEvent) {
        when (e) {
            is LinkEvent.ConnectionChanged -> {
                val wasConnected = state.connected
                state = state.copy(connection = e.connection)
                (e.connection as? Connection.Connected)?.let { linkKind = it.link }
                if (wasConnected != state.connected) knownChanged()
            }
            is LinkEvent.Choices -> state = state.copy(choices = e.pcs)
            is LinkEvent.BluetoothOffer -> state = state.copy(bluetoothOffer = e.show)
            is LinkEvent.TetherHint -> state = state.copy(tetherHint = e.show)
            is LinkEvent.Welcomed -> welcomed(e)
            is LinkEvent.Switched -> switched(e.link)
            is LinkEvent.Control -> control(e.message)
            LinkEvent.Held -> Unit
            LinkEvent.Release -> {
                // 30 s without the PC: let go of the hardware, keep the user's choices to bring back (section 7.6).
                waiting = true
                media.post(MediaMsg.Mic(null))
                media.post(MediaMsg.Camera(null))
                media.post(MediaMsg.Speaker(null, false, true))
                Feature.entries.forEach { f -> if (state.feature(f).isOn) set(f, FeatureView(FeaturePhase.PAUSED)) }
            }
            LinkEvent.Ended -> {
                waiting = false
                Feature.entries.forEach { f -> if (state.feature(f).phase != FeaturePhase.OFF) set(f, FeatureView()) }
                media.post(MediaMsg.Mic(null))
                media.post(MediaMsg.Camera(null))
                media.post(MediaMsg.Speaker(null, false, true))
                speakerFormat = null
                linkKind = null
                pcId = null
                settings.post(SettingsMsg.Use(null, false))
            }
            is LinkEvent.RestartSource -> media.post(
                MediaMsg.Restart(
                    when (e.stream) {
                        Stream.MIC -> Feature.MIC
                        Stream.CAMERA -> Feature.CAMERA
                        else -> Feature.SPEAKER
                    },
                ),
            )
            LinkEvent.KnownChanged -> knownChanged()
        }
    }

    private fun welcomed(e: LinkEvent.Welcomed) {
        linkKind = e.link
        pcId = e.pcId
        settings.post(SettingsMsg.Use(e.pcId, true, e.settings))
        val bringBack = waiting
        waiting = false
        micPausedByPc = false
        cameraPausedByPc = false
        speakerPausedByPc = false
        // Whatever was on carries on with the PC: after a short drop the pipelines kept running; after a long one they
        // restart. A pause the PC made ends with the session it was made in; the user's own pause stays.
        if (state.mic.phase != FeaturePhase.OFF && state.mic.phase != FeaturePhase.FAILED) {
            set(Feature.MIC, FeatureView(if (micPausedByUser) FeaturePhase.PAUSED else FeaturePhase.STARTING))
            startMic()
            media.post(MediaMsg.MicPaused(micPausedByUser))
        }
        if (state.camera.phase != FeaturePhase.OFF && state.camera.phase != FeaturePhase.FAILED) cameraOnLink(e.link)
        if (state.speaker.phase != FeaturePhase.OFF && state.speaker.phase != FeaturePhase.FAILED) {
            set(Feature.SPEAKER, FeatureView(FeaturePhase.STARTING))
            if (bringBack) startSpeaker()
        }
        sendState()
    }

    /**
     * The camera on a new link: Bluetooth can't carry video, so it pauses there (hardware off, the user's choice kept)
     * and comes back by itself on the next IP link (section 7.6).
     */
    private fun cameraOnLink(to: LinkKind) {
        if (to == LinkKind.BLUETOOTH) {
            media.post(MediaMsg.Camera(null))
            set(Feature.CAMERA, FeatureView(FeaturePhase.PAUSED, FeatureProblem.NO_BLUETOOTH))
            return
        }
        cameraPausedByPc = false
        set(Feature.CAMERA, FeatureView(FeaturePhase.STARTING))
        media.post(MediaMsg.CameraPaused(false))
        media.post(MediaMsg.Camera(cameraConfig()))
        cameraFormat?.let { (w, h, fps) -> sendCameraFormat(w, h, fps) }
        media.post(MediaMsg.Keyframe)
    }

    private fun switched(to: LinkKind) {
        linkKind = to
        if (state.mic.isOn || state.mic.phase == FeaturePhase.PAUSED) startMic()
        val camera = state.camera
        when {
            camera.phase == FeaturePhase.OFF || camera.phase == FeaturePhase.FAILED -> Unit
            to == LinkKind.BLUETOOTH -> cameraOnLink(to)
            camera.problem == FeatureProblem.NO_BLUETOOTH -> cameraOnLink(to)
            else -> {
                // Same picture on a new link: a new encoder only if the plan changed, a keyframe either way.
                media.post(MediaMsg.Camera(cameraConfig()))
                cameraFormat?.let { (w, h, fps) -> sendCameraFormat(w, h, fps) }
                media.post(MediaMsg.Keyframe)
            }
        }
        if (state.speaker.isOn) startSpeaker()
        sendState()
    }

    private fun control(m: com.owlmic.core.proto.Message) {
        when (m) {
            is State -> {
                // The PC pauses and resumes what the phone turned on; it never turns anything on (section 17.4).
                if (m.mic == FeatureState.PAUSED && state.mic.isOn) {
                    micPausedByPc = true
                    applyMicPause()
                } else if (m.mic == FeatureState.ON && micPausedByPc) {
                    micPausedByPc = false
                    applyMicPause()
                }
                if (m.camera == FeatureState.PAUSED && state.camera.isOn) {
                    cameraPausedByPc = true
                    media.post(MediaMsg.CameraPaused(true))
                    set(Feature.CAMERA, FeatureView(FeaturePhase.PAUSED))
                } else if (m.camera == FeatureState.ON && cameraPausedByPc) {
                    cameraPausedByPc = false
                    media.post(MediaMsg.CameraPaused(false))
                    set(Feature.CAMERA, FeatureView(FeaturePhase.ON))
                }
                if (m.speaker == FeatureState.PAUSED && state.speaker.isOn) {
                    speakerPausedByPc = true
                    set(Feature.SPEAKER, FeatureView(FeaturePhase.PAUSED))
                } else if (m.speaker == FeatureState.ON && speakerPausedByPc) {
                    speakerPausedByPc = false
                    set(Feature.SPEAKER, FeatureView(FeaturePhase.ON))
                }
                // The PC hears back what the phone now does, "paused" included.
                sendState()
            }
            is SettingsMessage -> settings.post(SettingsMsg.Remote(m.changes))
            is StreamStart -> if (m.stream == Stream.SPEAKER) {
                speakerFormat = SpeakerFormat(
                    codec = m.codec,
                    channels = m.params.optInt("channels", 2),
                    frameMs = m.params.optInt("frameMs", 10),
                )
                if (state.speaker.phase != FeaturePhase.OFF) startSpeaker()
            }
            is StreamStop -> if (m.stream == Stream.SPEAKER) {
                speakerFormat = null
                media.post(MediaMsg.Speaker(null, false, true))
            }
            KeyframeRequest -> media.post(MediaMsg.Keyframe)
            is Report -> {
                media.post(MediaMsg.MicLoss(m.lossPct.toInt()))
                media.post(MediaMsg.CameraReport(m.lossPct, m.rttMs))
            }
            else -> Unit
        }
    }

    /** Paired and blocked PCs: the connected one first, then the most recently connected. */
    private fun knownChanged() {
        val connectedId = pcId.takeIf { state.connected }
        state = state.copy(
            knownPcs = store.known().values.filter { it.approved || it.blocked }
                .sortedWith(compareByDescending<PcMemory> { it.id == connectedId }.thenByDescending { it.lastUsedAt })
                .map { KnownPcView(it.id, it.name, it.id == connectedId, it.blocked, it.address, it.lastUsedAt, it.lastLink) },
        )
    }

    // Settings.

    private fun fromSettings(e: SettingsEvent) {
        when (e) {
            is SettingsEvent.Push -> link.post(LinkMsg.Send(SettingsMessage(e.changes)))
            is SettingsEvent.Values -> {
                state = state.copy(settings = e.values)
                val changed = e.changed
                if (changed.any { it.startsWith("mic.") } && running(state.mic)) media.post(MediaMsg.Mic(micConfig()))
                if ("mic.isolation" in changed && state.speaker.isOn) startSpeaker()
                if (changed.any { it.startsWith("camera.") } && running(state.camera)) media.post(MediaMsg.Camera(cameraConfig()))
                if (changed.any { it.startsWith("link.") }) {
                    val allowed = LinkKind.entries.filter { k ->
                        state.settings[
                            when (k) {
                                LinkKind.USB_DEBUGGING -> "link.usbDebugging"
                                LinkKind.USB_TETHERING -> "link.usbTethering"
                                LinkKind.WIFI -> "link.wifi"
                                LinkKind.BLUETOOTH -> "link.bluetooth"
                            },
                        ] != "off"
                    }.toSet()
                    link.post(LinkMsg.Allowed(allowed))
                }
            }
        }
    }

    // Media.

    private fun fromMedia(e: MediaEvent) {
        when (e) {
            is MediaEvent.CameraFormat -> {
                cameraFormat = Triple(e.width, e.height, e.fps)
                sendCameraFormat(e.width, e.height, e.fps)
            }
            is MediaEvent.Health -> {
                val v = state.feature(e.feature)
                if (v.phase == FeaturePhase.OFF || v.phase == FeaturePhase.PAUSED) return
                set(
                    e.feature,
                    when (e.health) {
                        Health.Ok -> FeatureView(FeaturePhase.ON)
                        is Health.Degraded -> FeatureView(FeaturePhase.RECOVERING)
                        is Health.Failed -> FeatureView(FeaturePhase.FAILED)
                    },
                )
                if (e.health is Health.Failed) {
                    // Given up: let go of the hardware. The tile says to tap to try again.
                    when (e.feature) {
                        Feature.MIC -> media.post(MediaMsg.Mic(null))
                        Feature.CAMERA -> media.post(MediaMsg.Camera(null))
                        Feature.SPEAKER -> media.post(MediaMsg.Speaker(null, false, true))
                    }
                    if (e.feature != Feature.SPEAKER) link.post(LinkMsg.Send(StreamStop(if (e.feature == Feature.MIC) Stream.MIC else Stream.CAMERA)))
                    sendState()
                }
            }
        }
    }

    private fun sendCameraFormat(w: Int, h: Int, fps: Int) {
        link.post(LinkMsg.Send(StreamStart(Stream.CAMERA, "h264", JSONObject().put("width", w).put("height", h).put("fps", fps))))
    }

    /** At most 60 snapshots a second (section 11.2, rule 7): a burst of changes goes out as its last state. */
    private fun publish() {
        if (publishPending) return
        val wait = lastPublish + PUBLISH_EVERY_MS - System.currentTimeMillis()
        if (wait > 0) {
            publishPending = true
            later(wait) {
                publishPending = false
                push()
            }
        } else {
            push()
        }
    }

    private fun push() {
        lastPublish = System.currentTimeMillis()
        if (Owlmic.mutableState.value != state) {
            Owlmic.mutableState.value = state
            onState(state)
        }
    }

    override fun close() {
        if (booted) {
            link.close()
            settings.close()
        }
        media.close()
        super.close()
    }

    private companion object {
        const val HEALTH_EVERY_MS = 1_000L
        const val PUBLISH_EVERY_MS = 16L

        fun phoneName(context: Context): String =
            runCatching { Global.getString(context.contentResolver, Global.DEVICE_NAME) }.getOrNull()?.takeIf { it.isNotBlank() } ?: Build.MODEL

        /** A cable to a computer, not a wall charger: the battery's sticky broadcast says how it charges. */
        fun usbPlugged(context: Context): Boolean =
            context.registerReceiver(null, IntentFilter(Intent.ACTION_BATTERY_CHANGED))?.getIntExtra(BatteryManager.EXTRA_PLUGGED, 0) == BatteryManager.BATTERY_PLUGGED_USB
    }
}
