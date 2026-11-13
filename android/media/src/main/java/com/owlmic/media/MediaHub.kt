package com.owlmic.media

import android.content.Context
import android.view.Surface
import com.owlmic.core.hub.Feature
import com.owlmic.core.hub.Health
import com.owlmic.core.hub.Hub
import com.owlmic.core.link.MediaRouter
import com.owlmic.media.audio.CallMode
import com.owlmic.media.audio.FarEnd
import com.owlmic.media.audio.MicConfig
import com.owlmic.media.audio.MicPipeline
import com.owlmic.media.audio.SpeakerFormat
import com.owlmic.media.audio.SpeakerPipeline
import com.owlmic.media.camera.CameraConfig
import com.owlmic.media.camera.CameraPipeline
import kotlinx.coroutines.flow.MutableStateFlow

sealed interface MediaMsg {
    class Mic(val config: MicConfig?) : MediaMsg

    class MicPaused(val paused: Boolean) : MediaMsg

    class MicLoss(val percent: Int) : MediaMsg

    class Camera(val config: CameraConfig?) : MediaMsg

    class CameraPaused(val paused: Boolean) : MediaMsg

    class CameraReport(val lossPct: Double, val rttMs: Int) : MediaMsg

    data object Keyframe : MediaMsg

    class Speaker(val format: SpeakerFormat?, val voiceCommunication: Boolean, val wireless: Boolean) : MediaMsg

    /** Step 1 of the recovery ladder: restart [feature] at its source. */
    class Restart(val feature: Feature) : MediaMsg

    /** Android's thermal status changed. */
    class Thermal(val status: Int) : MediaMsg
}

sealed interface MediaEvent {
    class Health(val feature: Feature, val health: com.owlmic.core.hub.Health) : MediaEvent

    /** The camera's encoder now makes [width]×[height] at [fps]: time for a STREAM_START. */
    class CameraFormat(val width: Int, val height: Int, val fps: Int) : MediaEvent
}

/**
 * The Media Hub (section 11.1): starts, stops and reconfigures the mic, camera and speaker pipelines. Media itself
 * flows from the pipelines straight into the [MediaRouter], never through here. [levelFlow] receives the mic's level,
 * 0 to 1, every 50 ms while the mic is on, for the level bar. With voice isolation on both the mic and the Speaker, it
 * puts the phone in communication mode ([CallMode]) before they open.
 */
class MediaHub(
    context: Context,
    router: MediaRouter,
    private val levelFlow: MutableStateFlow<Float>,
    private val emit: (MediaEvent) -> Unit,
) : Hub<MediaMsg>("media") {
    /** What the Speaker plays, heard by the mic's echo suppressor. */
    private val farEnd = FarEnd()
    private val callMode = CallMode(context)
    private var micIsolated = false
    private var speakerIsolated = false
    private val mic = MicPipeline(router, onLevel = { levelFlow.value = it }, onHealth = { emit(MediaEvent.Health(Feature.MIC, it)) }, farEnd)
    private val camera = CameraPipeline(
        context, router,
        onFormat = { w, h, fps -> emit(MediaEvent.CameraFormat(w, h, fps)) },
        onHealth = { emit(MediaEvent.Health(Feature.CAMERA, it)) },
    )
    private val speaker = SpeakerPipeline(onHealth = { emit(MediaEvent.Health(Feature.SPEAKER, it)) }, farEnd)
    private var micOn = false
    private var cameraOn = false
    private var speakerFormat: SpeakerFormat? = null

    init {
        router.speaker = speaker
    }

    /** The camera preview tile's surface. Any thread; goes straight to the renderer. */
    fun setPreview(surface: Surface?, width: Int, height: Int) = camera.setPreview(surface, width, height)

    /** The PC's loss reports come every 10 s; a missed one is replaced by the next. */
    override fun droppable(message: MediaMsg) = message is MediaMsg.MicLoss || message is MediaMsg.CameraReport

    override fun handle(message: MediaMsg) {
        when (message) {
            is MediaMsg.Mic -> {
                val c = message.config
                micIsolated = c?.voiceCommunication == true
                callMode.update(micIsolated, speakerIsolated)
                when {
                    c == null -> if (micOn) {
                        mic.stop()
                        micOn = false
                    }
                    micOn -> mic.update(c)
                    else -> {
                        mic.start(c)
                        micOn = true
                    }
                }
            }
            is MediaMsg.MicPaused -> mic.paused = message.paused
            is MediaMsg.MicLoss -> mic.packetLoss = message.percent
            is MediaMsg.Camera -> {
                val c = message.config
                if (c == null) {
                    if (cameraOn) camera.stop()
                    cameraOn = false
                } else {
                    camera.start(c)
                    cameraOn = true
                }
            }
            is MediaMsg.CameraPaused -> {
                camera.paused = message.paused
                if (!message.paused) camera.requestKeyframe()
            }
            is MediaMsg.CameraReport -> camera.report(message.lossPct, message.rttMs)
            MediaMsg.Keyframe -> camera.requestKeyframe()
            is MediaMsg.Speaker -> {
                val f = message.format
                speakerIsolated = f != null && message.voiceCommunication
                callMode.update(micIsolated, speakerIsolated)
                if (f == null) {
                    speaker.stop()
                    speakerFormat = null
                } else {
                    if (speakerFormat == null) speaker.start(f, message.voiceCommunication, message.wireless) else {
                        speaker.setWireless(message.wireless)
                        if (f != speakerFormat) speaker.configure(f)
                        speaker.setVoiceCommunication(message.voiceCommunication)
                    }
                    speakerFormat = f
                }
            }
            is MediaMsg.Restart -> when (message.feature) {
                Feature.MIC -> mic.restart()
                Feature.CAMERA -> camera.restartEncoder()
                Feature.SPEAKER -> Unit
            }
            is MediaMsg.Thermal -> camera.thermal(message.status)
        }
    }

    override fun onFailure(message: MediaMsg, error: Throwable) {
        val feature = when (message) {
            is MediaMsg.Mic, is MediaMsg.MicPaused, is MediaMsg.MicLoss -> Feature.MIC
            is MediaMsg.Speaker -> Feature.SPEAKER
            else -> Feature.CAMERA
        }
        emit(MediaEvent.Health(feature, Health.Failed(error.message ?: error.javaClass.simpleName)))
    }

    override fun close() {
        mic.stop()
        camera.stop()
        speaker.stop()
        callMode.release()
        super.close()
    }
}
