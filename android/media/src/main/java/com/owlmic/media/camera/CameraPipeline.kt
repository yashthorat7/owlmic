package com.owlmic.media.camera

import android.content.Context
import android.hardware.camera2.CameraCharacteristics
import android.os.Handler
import android.os.Looper
import android.util.Range
import android.util.Size
import android.view.OrientationEventListener
import android.view.Surface
import androidx.camera.camera2.interop.Camera2CameraInfo
import androidx.camera.camera2.interop.ExperimentalCamera2Interop
import androidx.camera.core.CameraInfo
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
import com.owlmic.core.hub.LinkKind
import com.owlmic.core.hub.RestartPolicy
import com.owlmic.core.link.MediaOut
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executor
import java.util.concurrent.TimeUnit

enum class Lens { BACK, FRONT }

/** Which way is up in the picture: the way the phone is held, or always as if held sideways or upright. */
enum class Orientation { AUTO, LANDSCAPE, PORTRAIT }

/**
 * [ratio] is the picture's shape: the sensor's whole frame by default, or a shape the user picked. [mirrorPreview] flips
 * only the phone's own preview.
 */
data class CameraConfig(
    val lens: Lens,
    val plan: VideoPlan,
    val orientation: Orientation,
    val ratio: Ratio = Ratio.FULL,
    val mirrorPreview: Boolean = false,
    val quality: String = "auto",
    val link: LinkKind = LinkKind.WIFI,
)

/**
 * The phone's camera (section 17.2): CameraX into the [GlRenderer], into the [VideoEncoder], into [out]. The phone
 * controls what is sent (lens, size, shape, frame rate, orientation); the PC controls what is shown. [onFormat] reports
 * each new encoded size so a STREAM_START goes out. The camera is bound again only for another lens, a bigger picture,
 * 60 fps or another framing; a smaller plan (a link switch to Wi-Fi, heat) or another shape from the same frame only
 * needs a new encoder, which takes well under 300 ms.
 *
 * By default ([Ratio.FULL]) the PC gets everything the sensor captures, at its own aspect: the encoder's size comes from
 * the camera's real frame, upright for the hold, so turning the phone flips it (4:3 to 3:4) and cuts nothing. A preset
 * keeps its shape whichever way the phone is held.
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

    /** The config the camera was bound for: what it captures. */
    private var captured: CameraConfig? = null
    @Volatile private var capturedFps = 0

    /** GL thread: the size the current encoder was made for. */
    private var encoded: Pair<Int, Int>? = null
    @Volatile private var rotation = Surface.ROTATION_0
    @Volatile private var pendingRot: Int? = null
    @Volatile private var thermal = 0

    /** Created and dropped on the main thread; read from others only to post onto its GL thread. */
    @Volatile private var renderer: GlRenderer? = null

    // GL thread.
    private var encoder: VideoEncoder? = null
    private var optimizer: StreamOptimizer? = null
    private var activePlan: VideoPlan? = null
    @Volatile private var measuredOutFps: Int? = null
    private val encoderRestarts = RestartPolicy()

    /** Remembered while the camera is off, so the preview appears as soon as it starts. */
    @Volatile private var previewSurface: Triple<Surface, Int, Int>? = null
    @Volatile private var previewAspect: Float? = null
    @Volatile private var previewBorderPx = 1

    @Volatile var paused = false

    private val orientation = object : OrientationEventListener(context) {
        override fun onOrientationChanged(degrees: Int) {
            if (degrees == ORIENTATION_UNKNOWN) return
            val next = surfaceRotationFor(degrees, rotation)
            if (next != rotation) {
                rotation = next
                val targetRot = targetRotationFor(config?.orientation ?: Orientation.AUTO, next)
                // CameraX's new rotation reaches the renderer with the frames it applies to; a full frame that turns
                // from 4:3 to 3:4 gets its new encoder there (see [frameChanged]), so the two never disagree.
                if (preview?.targetRotation != targetRot) {
                    pendingRot = targetRot
                    preview?.targetRotation = targetRot
                }
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

    /** Android's thermal status, from the App Hub: adapt stream without dropping below 24 fps. Any thread. */
    fun thermal(status: Int) {
        thermal = status
        post {
            optimizer?.setThermal(status)?.let(::applyAdjustment)
        }
    }

    /** The PC's REPORT for the camera stream: adapt FPS (down to 24 floor), resolution, and bitrate smoothly. */
    fun report(
        lossPct: Double,
        rttMs: Int,
        camLossPct: Double? = null,
        camFps: Int? = null,
        camDropPct: Double? = null,
    ) = post {
        optimizer?.report(
            lossPct = lossPct,
            rttMs = rttMs,
            camLossPct = camLossPct,
            camFps = camFps,
            camDropPct = camDropPct,
            measuredOutFps = measuredOutFps,
            congested = out.videoCongested(),
        )?.let(::applyAdjustment)
    }

    /**
     * The preview tile's surface while it is on screen, null when it goes (or the user turns the preview off). A surface
     * that goes is let go of on the GL thread before this returns, so no frame is drawn into it after it is destroyed.
     * Main thread.
     */
    fun setPreview(surface: Surface?, width: Int, height: Int) {
        previewSurface = surface?.let { Triple(it, width, height) }
        val r = renderer ?: return
        if (surface != null) {
            r.handler.post {
                r.setPreviewAspect(previewAspect, previewBorderPx)
                r.setPreview(surface, width, height)
            }
            return
        }
        val released = CountDownLatch(1)
        if (r.handler.post { r.setPreview(null, 0, 0); released.countDown() }) released.await(PREVIEW_RELEASE_MS, TimeUnit.MILLISECONDS)
    }

    /** Sets the preview tile's target crop aspect ratio (null = full playground) and hairline border width in px. Any thread. */
    fun setPreviewAspect(aspect: Float?, borderPx: Int) {
        previewAspect = aspect
        previewBorderPx = borderPx
        val r = renderer ?: return
        r.handler.post { r.setPreviewAspect(aspect, borderPx) }
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
        val r = renderer ?: GlRenderer().also { gl ->
            gl.onOutputFps = { fpsSample ->
                measuredOutFps = fpsSample
                if (out.videoCongested()) {
                    optimizer?.onCongested()?.let(::applyAdjustment)
                }
            }
            renderer = gl
        }
        // The mirror is the preview's alone: the stream doesn't change, so neither the encoder nor the camera does. A
        // bind still on its way was for the previous config, though, and drops itself, so this one must bind again.
        r.mirrorPreview = next.mirrorPreview
        val capture = captured
        if (capture != null && previous?.copy(mirrorPreview = next.mirrorPreview) == next) return
        val needsCamera = previous == null || capture == null || previous.lens != next.lens ||
            next.plan.shortSide > capture.plan.shortSide || next.plan.fps > capture.plan.fps ||
            framing(next.ratio) != framing(capture.ratio)
        // A new size or shape needs a new encoder; link/bitrate/fps changes tune the running encoder in place.
        val needsEncoder = previous == null ||
            previous.plan.longSide != next.plan.longSide ||
            previous.plan.shortSide != next.plan.shortSide ||
            previous.ratio != next.ratio
        val resetOpt = previous == null || previous.quality != next.quality || previous.plan.fps != next.plan.fps || previous.link != next.link
        val targetRot = targetRotationFor(next.orientation, rotation)
        if (!needsCamera && preview != null && preview?.targetRotation != targetRot) {
            pendingRot = targetRot
            preview?.targetRotation = targetRot
        }
        val rotInFlight = !needsCamera && pendingRot != null
        r.handler.post {
            if (!needsCamera) {
                if (resetOpt || optimizer == null) {
                    val opt = StreamOptimizer(next.quality, next.plan.fps, next.link)
                    opt.setThermal(thermal)
                    optimizer = opt
                    activePlan = opt.current
                }
                val effPlan = activePlan ?: next.plan
                val effFps = if (capturedFps in 1 until effPlan.fps) capturedFps else effPlan.fps
                r.fps = effFps.coerceAtLeast(24)
                if (needsEncoder && !rotInFlight) {
                    newEncoder()
                } else if (!needsEncoder) {
                    encoded?.let { (w, h) -> encoder?.setBitrate(bitrateFor(effPlan, w, h)) }
                    encoder?.requestKeyframe()
                }
            } else {
                // Keep the running encoder alive through CameraX rebind so KEY_REPEAT_PREVIOUS_FRAME_AFTER sustains output.
                encoder?.requestKeyframe()
            }
            r.setPreviewAspect(previewAspect, previewBorderPx)
            previewSurface?.let { (s, w, h) -> r.setPreview(s, w, h) }
        }
        if (!needsCamera) {
            if (rotInFlight) {
                r.handler.postDelayed({
                    if (pendingRot != null) {
                        pendingRot = null
                        frameChanged()
                    }
                }, 150L)
            } else {
                r.handler.post { frameChanged() }
            }
            return
        }
        // Recorded only once bound: a config arriving before then must bind again, as this one will be stale.
        captured = null
        capturedFps = 0
        pendingRot = null
        val ready = ProcessCameraProvider.getInstance(context)
        ready.addListener({
            if (config !== next || lifecycle !== owner) return@addListener
            val provider = ready.get()
            provider.unbindAll()
            if (resetOpt) {
                r.handler.post {
                    val opt = StreamOptimizer(next.quality, next.plan.fps, next.link)
                    opt.setThermal(thermal)
                    optimizer = opt
                    activePlan = opt.current
                }
            }
            val glExecutor = Executor { r.handler.post(it) }
            // The front lens is the default; a phone or tablet without the lens asked for gets the other one.
            val front = if (next.lens == Lens.FRONT) {
                runCatching { provider.hasCamera(CameraSelector.DEFAULT_FRONT_CAMERA) }.getOrDefault(true)
            } else {
                !runCatching { provider.hasCamera(CameraSelector.DEFAULT_BACK_CAMERA) }.getOrDefault(true)
            }
            val camera = if (front) CameraSelector.DEFAULT_FRONT_CAMERA else CameraSelector.DEFAULT_BACK_CAMERA
            val cameraInfo = runCatching { provider.getCameraInfo(camera) }.getOrNull()
            val sensor = cameraInfo?.sensorRotationDegrees ?: 0
            val fourByThree = boundFourByThree(next.ratio, sensorWide(cameraInfo))
            val (bw, bh) = boundSize(next.plan, fourByThree)
            val aspect = if (fourByThree) AspectRatioStrategy.RATIO_4_3_FALLBACK_AUTO_STRATEGY else AspectRatioStrategy.RATIO_16_9_FALLBACK_AUTO_STRATEGY
            val selector = ResolutionSelector.Builder()
                .setAspectRatioStrategy(aspect)
                .setResolutionStrategy(ResolutionStrategy(Size(bw, bh), ResolutionStrategy.FALLBACK_RULE_CLOSEST_HIGHER_THEN_LOWER))
                .build()
            val fpsRange = bestFpsRange(cameraInfo, next.plan.fps)
            fun build(range: Range<Int>?): Preview = Preview.Builder()
                .setResolutionSelector(selector)
                .setTargetRotation(targetRot)
                .apply { if (range != null) setTargetFrameRate(range) }
                .build()
                .apply {
                    setSurfaceProvider(glExecutor) { request ->
                        val res = request.resolution
                        // The frame's size first: the rotation that comes with this request applies to it.
                        val surface = r.cameraSurface(res.width, res.height)
                        request.setTransformationInfoListener(glExecutor) { info ->
                            pendingRot = null
                            r.rotation = info.rotationDegrees
                            r.cameraTransform = if (info.hasCameraTransform()) CameraTransform(sensor, front) else null
                            cameraInfo?.let { c -> pictureTurn(info.rotationDegrees) { c.getSensorRotationDegrees(it) } }?.let { r.screenTurn = it }
                            frameChanged()
                        }
                        request.provideSurface(surface, glExecutor) { }
                    }
                }
            var appliedFps = next.plan.fps
            val bound = runCatching {
                build(fpsRange).also {
                    provider.bindToLifecycle(owner, camera, it)
                    if (fpsRange != null) appliedFps = minOf(next.plan.fps, fpsRange.upper)
                }
            }.recoverCatching {
                provider.unbindAll()
                build(null).also { provider.bindToLifecycle(owner, camera, it) }
            }
            preview = bound.getOrNull()
            if (bound.isSuccess) {
                captured = next
                capturedFps = appliedFps
                r.handler.post {
                    val effPlan = activePlan ?: next.plan
                    val effFps = effPlan.fps.coerceAtMost(appliedFps).coerceAtLeast(24)
                    r.fps = effFps
                    encoded?.let { (w, h) -> encoder?.setBitrate(bitrateFor(effPlan, w, h)) }
                    encoder?.requestKeyframe()
                }
            }
            onHealth(if (bound.isSuccess) Health.Ok else Health.Failed("no usable camera"))
        }, Executor { main.post(it) })
    }

    /**
     * GL thread: the camera's frame or its rotation changed. Any picture whose output shape changed with it (another
     * lens, or the phone turned between portrait and landscape) gets a new encoder before the next frame is drawn.
     */
    private fun frameChanged() {
        val c = config ?: return
        val plan = activePlan ?: c.plan
        val size = outputSize(plan, c.ratio, renderer?.uprightFrame()) ?: return
        if (encoder == null || size != encoded) newEncoder()
    }

    /**
     * GL thread: a fresh encoder for the current shape. The old one goes, and the PC hears the new size. A full-frame
     * picture waits for the camera's first frame size: until then there is nothing to size it by.
     */
    private fun newEncoder() {
        val r = renderer ?: return
        val c = config ?: return
        val base = activePlan ?: c.plan
        val plan = if (capturedFps in 24 until base.fps) base.copy(fps = capturedFps) else base
        val (w, h) = outputSize(plan, c.ratio, r.uprightFrame()) ?: return
        val initialBitrate = bitrateFor(plan, w, h)
        fun create() = VideoEncoder(
            w, h, plan.fps, initialBitrate,
            onFrame = { pts, key, data, len -> if (!paused && !out.video(pts, key, data, 0, len)) post(::pictureRefused) },
            onError = { post(::encoderFailed) },
        )
        // Build the replacement before detaching the live encoder so frames never stall; fall back to release-first on single-instance codecs.
        var prev = encoder
        val e = runCatching { create() }.recoverCatching {
            if (prev == null) throw it
            r.setEncoder(null, 0, 0)
            prev.release()
            prev = null
            encoder = null
            encoded = null
            create()
        }.getOrElse {
            onHealth(Health.Failed("no video encoder"))
            return
        }
        encoder = e
        encoded = w to h
        r.fps = plan.fps.coerceAtLeast(24)
        r.setEncoder(e.inputSurface, w, h)
        prev?.release()
        onFormat(w, h, plan.fps)
    }

    /** GL thread: a picture was too big to send, so the PC is missing a frame. Less bitrate, and a keyframe to start over. */
    private fun pictureRefused() {
        optimizer?.refused()?.let(::applyAdjustment)
        encoder?.requestKeyframe()
    }

    private fun applyAdjustment(adj: StreamAdjustment) {
        activePlan = adj.plan
        val c = config ?: return
        val r = renderer ?: return
        if (adj.resolutionChanged) {
            if (adj.plan.shortSide > (captured?.plan?.shortSide ?: 0)) {
                main.post {
                    val cur = config ?: return@post
                    if (cur.plan.shortSide < adj.plan.shortSide) {
                        bind(cur.copy(plan = adj.plan))
                    }
                }
            } else {
                newEncoder()
            }
            return
        }
        if (adj.fpsChanged) {
            val eff = if (capturedFps in 24 until adj.plan.fps) capturedFps else adj.plan.fps
            r.fps = eff.coerceAtLeast(24)
        }
        if (adj.bitrateChanged) {
            val w = encoded?.first
            val h = encoded?.second
            val targetBps = if (w != null && h != null) bitrateFor(adj.plan, w, h) else adj.plan.bitrate
            encoder?.setBitrate(targetBps)
        }
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
        capturedFps = 0
        pendingRot = null
        owner.destroy()
        lifecycle = null
        orientation.disable()
        val r = renderer ?: return
        renderer = null
        r.handler.post {
            r.setEncoder(null, 0, 0)
            encoder?.release()
            encoder = null
            encoded = null
            optimizer = null
            activePlan = null
            measuredOutFps = null
        }
        r.release()
    }

    /** Whether the [camera]'s whole sensor area is 16:9 rather than 4:3: the full picture then binds the 16:9 frame. */
    @androidx.annotation.OptIn(markerClass = [ExperimentalCamera2Interop::class])
    private fun sensorWide(camera: CameraInfo?): Boolean {
        val area = camera?.let { runCatching { Camera2CameraInfo.from(it).getCameraCharacteristic(CameraCharacteristics.SENSOR_INFO_ACTIVE_ARRAY_SIZE) }.getOrNull() }
        return area != null && sensorIsWide(area.width(), area.height())
    }

    /** Picks AE range whose lower bound never drops below 24 fps when available. */
    @androidx.annotation.OptIn(markerClass = [ExperimentalCamera2Interop::class])
    private fun bestFpsRange(camera: CameraInfo?, requested: Int): Range<Int>? {
        if (requested <= 0) return null
        val ranges = camera?.let {
            runCatching { Camera2CameraInfo.from(it).getCameraCharacteristic(CameraCharacteristics.CONTROL_AE_AVAILABLE_TARGET_FPS_RANGES) }.getOrNull()
        } ?: return null
        val picked = selectFpsRange(ranges.map { it.lower to it.upper }, requested) ?: return null
        return Range(picked.first, picked.second)
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

/**
 * How the phone was held for a frame, as a quarter turn in degrees: the Surface rotation whose buffer rotation
 * ([rotationFor], CameraX's for that rotation) is the frame's [rotationDegrees]. Null if none is.
 */
fun pictureTurn(rotationDegrees: Int, rotationFor: (Int) -> Int): Int? =
    listOf(Surface.ROTATION_0, Surface.ROTATION_90, Surface.ROTATION_180, Surface.ROTATION_270)
        .firstOrNull { rotationFor(it) == rotationDegrees }?.let { it * 90 }

/** The target rotation that matches the requested [orientation] given the physical [sensorRotation]. */
fun targetRotationFor(orientation: Orientation, sensorRotation: Int): Int = when (orientation) {
    Orientation.LANDSCAPE -> if (sensorRotation == Surface.ROTATION_270) Surface.ROTATION_270 else Surface.ROTATION_90
    Orientation.PORTRAIT -> if (sensorRotation == Surface.ROTATION_180) Surface.ROTATION_180 else Surface.ROTATION_0
    Orientation.AUTO -> sensorRotation
}

/** Which camera binding a ratio needs: the full frame, the 4:3 one or the 16:9 one. Another one means binding again. */
private fun framing(ratio: Ratio): Int = when (ratio) {
    Ratio.FULL -> 0
    Ratio.CLASSIC, Ratio.PORTRAIT_CLASSIC, Ratio.SQUARE -> 1
    Ratio.WIDE, Ratio.TALL, Ratio.CINEMA, Ratio.PORTRAIT_CINEMA -> 2
}

/** How long the main thread waits for the GL thread to let go of a preview surface that is going. */
private const val PREVIEW_RELEASE_MS = 500L
