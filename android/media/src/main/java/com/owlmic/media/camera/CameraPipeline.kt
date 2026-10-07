package com.owlmic.media.camera

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.util.Range
import android.util.Size
import android.view.OrientationEventListener
import android.view.Surface
import androidx.camera.core.CameraSelector
import androidx.camera.core.Preview
import androidx.camera.core.resolutionselector.AspectRatioStrategy
import androidx.camera.core.resolutionselector.ResolutionSelector
import androidx.camera.core.resolutionselector.ResolutionStrategy
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.LifecycleRegistry
import com.owlmic.core.hub.Health
import com.owlmic.core.hub.RestartPolicy
import com.owlmic.core.link.MediaOut
import java.util.concurrent.Executor

enum class Lens { BACK, FRONT }

enum class Orientation { AUTO, LANDSCAPE, PORTRAIT }

data class CameraConfig(val lens: Lens, val plan: VideoPlan, val orientation: Orientation)

/**
 * The phone's camera (section 17.2): CameraX into the [GlRenderer], into the [VideoEncoder], into [out]. The phone
 * controls what is sent (lens, size, frame rate, orientation); the PC controls what is shown. [onFormat] reports each
 * new encoded size so a STREAM_START goes out. The camera is bound again only for another lens, a bigger picture or
 * 60 fps; a smaller plan (a link switch to Wi-Fi, heat) only needs a new encoder, which takes well under 300 ms.
 */
class CameraPipeline(
    private val context: Context,
    private val out: MediaOut,
    private val onFormat: (width: Int, height: Int, fps: Int) -> Unit,
    private val onHealth: (Health) -> Unit,
) {
    private val main = Handler(Looper.getMainLooper())

    // Main thread, except [config] and [thermal], which the GL thread reads when it builds an encoder.
    @Volatile private var config: CameraConfig? = null
    private var lifecycle: CameraLifecycle? = null
    private var preview: Preview? = null

    /** The plan the camera was bound for: what it captures. */
    private var captured: VideoPlan? = null
    @Volatile private var rotation = Surface.ROTATION_0
    @Volatile private var thermal = 0

    /** Created and dropped on the main thread; read from others only to post onto its GL thread. */
    @Volatile private var renderer: GlRenderer? = null

    // GL thread.
    private var encoder: VideoEncoder? = null
    private var bitrate: BitrateController? = null
    private val encoderRestarts = RestartPolicy()

    /** Remembered while the camera is off, so the preview appears as soon as it starts. */
    @Volatile private var previewSurface: Triple<Surface, Int, Int>? = null

    @Volatile var paused = false

    private val orientation = object : OrientationEventListener(context) {
        override fun onOrientationChanged(degrees: Int) {
            if (degrees == ORIENTATION_UNKNOWN) return
            val next = surfaceRotationFor(degrees, rotation)
            if (next != rotation) {
                val flipped = (next % 2) != (rotation % 2)
                rotation = next
                val targetRot = targetRotationFor(config?.orientation ?: Orientation.AUTO, next)
                if (preview?.targetRotation != targetRot) {
                    preview?.targetRotation = targetRot
                }
                if (flipped && config?.orientation == Orientation.AUTO) post { newEncoder() }
            }
        }
    }

    /** Starts, or moves to [config] (another lens, size or orientation). Any thread. */
    fun start(config: CameraConfig) {
        main.post { bind(config) }
    }

    fun stop() {
        main.post { unbind() }
    }

    fun requestKeyframe() = post { encoder?.requestKeyframe() }

    /** Step 1 of the recovery ladder (section 14.8): a fresh encoder, which starts with a keyframe. */
    fun restartEncoder() = post { newEncoder() }

    /** Android's thermal status, from the App Hub: lower the frame rate, then the quality (section 19). Any thread. */
    fun thermal(status: Int) {
        val before = thermal
        thermal = status
        post {
            val c = config ?: return@post
            renderer?.fps = thermalFps(c.plan.fps, status)
            if (thermalPlan(c.plan, status) != thermalPlan(c.plan, before)) newEncoder()
        }
    }

    /** The PC's REPORT for the camera stream: adapt the bitrate. */
    fun report(lossPct: Double, rttMs: Int) = post {
        bitrate?.report(lossPct, rttMs)?.let { encoder?.setBitrate(it) }
    }

    /** The preview tile's surface while it is on screen, null when it goes. */
    fun setPreview(surface: Surface?, width: Int, height: Int) {
        previewSurface = surface?.let { Triple(it, width, height) }
        post { renderer?.setPreview(surface, width, height) }
    }

    private fun post(block: () -> Unit) {
        renderer?.handler?.post(block)
    }

    private fun bind(next: CameraConfig) {
        val previous = config
        config = next
        val owner = lifecycle ?: CameraLifecycle().also {
            lifecycle = it
            it.start()
            orientation.enable()
        }
        val r = renderer ?: GlRenderer().also { renderer = it }
        val capture = captured
        val needsCamera = previous == null || capture == null || previous.lens != next.lens ||
            next.plan.shortSide > capture.shortSide || (next.plan.fps > 30) != (capture.fps > 30)
        // A new shape needs a new encoder; another lens only needs a fresh keyframe.
        val needsEncoder = previous == null || previous.plan != next.plan || previous.orientation != next.orientation
        r.handler.post {
            r.fps = thermalFps(next.plan.fps, thermal)
            if (needsEncoder) newEncoder() else encoder?.requestKeyframe()
            previewSurface?.let { (s, w, h) -> r.setPreview(s, w, h) }
        }
        val targetRot = targetRotationFor(next.orientation, rotation)
        if (!needsCamera) {
            preview?.targetRotation = targetRot
            return
        }
        captured = next.plan
        val ready = ProcessCameraProvider.getInstance(context)
        ready.addListener({
            if (config !== next || lifecycle !== owner) return@addListener
            val provider = ready.get()
            provider.unbindAll()
            val isTargetLandscape = targetRot == Surface.ROTATION_90 || targetRot == Surface.ROTATION_270
            val boundSize = if (isTargetLandscape) Size(next.plan.longSide, next.plan.shortSide) else Size(next.plan.shortSide, next.plan.longSide)
            val selector = ResolutionSelector.Builder()
                .setAspectRatioStrategy(AspectRatioStrategy.RATIO_16_9_FALLBACK_AUTO_STRATEGY)
                .setResolutionStrategy(ResolutionStrategy(boundSize, ResolutionStrategy.FALLBACK_RULE_CLOSEST_HIGHER_THEN_LOWER))
                .build()
            val glExecutor = Executor { r.handler.post(it) }
            fun build(highFps: Boolean): Preview = Preview.Builder()
                .setResolutionSelector(selector)
                .setTargetRotation(targetRot)
                .apply { if (highFps) setTargetFrameRate(Range(next.plan.fps, next.plan.fps)) }
                .build()
                .apply {
                    setSurfaceProvider(glExecutor) { request ->
                        val res = request.resolution
                        request.setTransformationInfoListener(glExecutor) { info -> r.rotation = info.rotationDegrees }
                        request.provideSurface(r.cameraSurface(res.width, res.height), glExecutor) { }
                    }
                }
            val camera = if (next.lens == Lens.FRONT) CameraSelector.DEFAULT_FRONT_CAMERA else CameraSelector.DEFAULT_BACK_CAMERA
            val bound = runCatching { build(next.plan.fps > 30).also { provider.bindToLifecycle(owner, camera, it) } }
                .recoverCatching { build(false).also { provider.bindToLifecycle(owner, camera, it) } }
            preview = bound.getOrNull()
            onHealth(if (bound.isSuccess) Health.Ok else Health.Failed("no usable camera"))
        }, Executor { main.post(it) })
    }

    /** GL thread: a fresh encoder for the current shape. The old one goes, and the PC hears the new size. */
    private fun newEncoder() {
        val r = renderer ?: return
        val c = config ?: return
        r.setEncoder(null, 0, 0)
        encoder?.release()
        encoder = null
        val landscape = when (c.orientation) {
            Orientation.LANDSCAPE -> true
            Orientation.PORTRAIT -> false
            Orientation.AUTO -> rotation == Surface.ROTATION_90 || rotation == Surface.ROTATION_270
        }
        val plan = thermalPlan(c.plan, thermal)
        val (w, h) = if (landscape) plan.longSide to plan.shortSide else plan.shortSide to plan.longSide
        // A rebuilt encoder keeps the adapted bitrate; a new plan starts its own.
        val rate = bitrate?.takeIf { it.start == plan.bitrate } ?: BitrateController(plan.bitrate).also { bitrate = it }
        val e = runCatching {
            VideoEncoder(
                w, h, plan.fps, rate.current,
                onFrame = { pts, key, data, len -> if (!paused && !out.video(pts, key, data, 0, len)) post(::pictureRefused) },
                onError = { post(::encoderFailed) },
            )
        }.getOrElse {
            onHealth(Health.Failed("no video encoder"))
            return
        }
        encoder = e
        r.fps = thermalFps(plan.fps, thermal)
        r.setEncoder(e.inputSurface, w, h)
        onFormat(w, h, plan.fps)
    }

    /** GL thread: a picture was too big to send, so the PC is missing a frame. Less bitrate, and a keyframe to start over. */
    private fun pictureRefused() {
        val e = encoder ?: return
        bitrate?.refused()?.let(e::setBitrate)
        e.requestKeyframe()
    }

    /** GL thread: MediaCodec reported an error; a new encoder after the usual backoff, until it has failed too often. */
    private fun encoderFailed() {
        val r = renderer ?: return
        val delay = encoderRestarts.nextDelayMs()
        if (delay == null) {
            r.setEncoder(null, 0, 0)
            encoder?.release()
            encoder = null
            onHealth(Health.Failed("video encoder keeps failing"))
        } else {
            onHealth(Health.Degraded("video encoder restarting"))
            r.handler.postDelayed({
                newEncoder()
                if (encoder != null) onHealth(Health.Ok)
            }, delay)
        }
    }

    private fun unbind() {
        val owner = lifecycle ?: return
        config = null
        runCatching { ProcessCameraProvider.getInstance(context).get().unbindAll() }
        preview = null
        captured = null
        owner.destroy()
        lifecycle = null
        orientation.disable()
        val r = renderer ?: return
        renderer = null
        r.handler.post {
            r.setEncoder(null, 0, 0)
            encoder?.release()
            encoder = null
            bitrate = null
        }
        r.release()
    }

    /** CameraX binds to a lifecycle; the camera's is its own, from start to stop. Main thread. */
    private class CameraLifecycle : LifecycleOwner {
        private val registry = LifecycleRegistry(this)
        override val lifecycle: Lifecycle get() = registry

        fun start() {
            registry.currentState = Lifecycle.State.RESUMED
        }

        fun destroy() {
            registry.currentState = Lifecycle.State.DESTROYED
        }
    }
}

/**
 * The screen rotation that matches how the phone is held, from the orientation sensor's degrees. A change needs to be
 * 20° past the halfway point, so a phone held near a diagonal doesn't flicker between shapes.
 */
fun surfaceRotationFor(degrees: Int, current: Int): Int {
    val candidate = when (degrees) {
        in 45 until 135 -> Surface.ROTATION_270
        in 135 until 225 -> Surface.ROTATION_180
        in 225 until 315 -> Surface.ROTATION_90
        else -> Surface.ROTATION_0
    }
    if (candidate == current) return current
    val center = when (current) {
        Surface.ROTATION_270 -> 90
        Surface.ROTATION_180 -> 180
        Surface.ROTATION_90 -> 270
        else -> 0
    }
    val distance = minOf((degrees - center + 360) % 360, (center - degrees + 360) % 360)
    return if (distance > 65) candidate else current
}

/** The target rotation that matches the requested [orientation] given the physical [sensorRotation]. */
fun targetRotationFor(orientation: Orientation, sensorRotation: Int): Int = when (orientation) {
    Orientation.LANDSCAPE -> if (sensorRotation == Surface.ROTATION_270) Surface.ROTATION_270 else Surface.ROTATION_90
    Orientation.PORTRAIT -> if (sensorRotation == Surface.ROTATION_180) Surface.ROTATION_180 else Surface.ROTATION_0
    Orientation.AUTO -> sensorRotation
}
