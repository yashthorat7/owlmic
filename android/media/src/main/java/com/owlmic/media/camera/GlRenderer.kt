package com.owlmic.media.camera

import android.graphics.SurfaceTexture
import android.opengl.EGL14
import android.opengl.EGLConfig
import android.opengl.EGLContext
import android.opengl.EGLDisplay
import android.opengl.EGLExt
import android.opengl.EGLSurface
import android.opengl.GLES11Ext
import android.opengl.GLES20
import android.opengl.Matrix
import android.os.Handler
import android.os.HandlerThread
import android.view.Choreographer
import android.view.Surface
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.FloatBuffer
import kotlin.math.abs
import kotlin.math.roundToInt

/**
 * The camera's picture on its way to the encoder (section 17.2, "Shape"): one EGL context on its own thread takes the
 * camera's external texture, rotates it upright and crops it to the output's shape, then draws it into the encoder's
 * input surface and, while visible, into the preview. Nothing is copied on the CPU.
 */
class GlRenderer {
    private val thread = HandlerThread("owlmic-gl").apply { start() }
    val handler = Handler(thread.looper)

    private var display: EGLDisplay = EGL14.EGL_NO_DISPLAY
    private var context: EGLContext = EGL14.EGL_NO_CONTEXT
    private var config: EGLConfig? = null
    private var idle: EGLSurface = EGL14.EGL_NO_SURFACE
    private lateinit var program: Program
    private lateinit var shrink: Program
    private lateinit var blur: Program
    private lateinit var solid: Program
    private var texture = 0
    private var cameraTexture: SurfaceTexture? = null
    private var cameraSurface: Surface? = null

    private var encoder: EGLSurface = EGL14.EGL_NO_SURFACE
    private var encoderWidth = 0
    private var encoderHeight = 0
    private var preview: EGLSurface = EGL14.EGL_NO_SURFACE
    private var previewSurfaceRef: Surface? = null
    private var previewWidth = 0
    private var previewHeight = 0

    // Snapshot of the currently latched SurfaceTexture frame so smooth crop redraws stay consistent during CameraX rebinds.
    private var hasFrame = false
    private var outFrameCount = 0
    private var outFpsWindowStartNs = 0L
    @Volatile var onOutputFps: ((Int) -> Unit)? = null

    private var drawnUprightW = 0
    private var drawnUprightH = 0
    private var drawnRotation = 0
    private var drawnTurn = 0
    private var drawnCamera: CameraTransform? = null

    // GL-thread optical crop animation state.
    private var hasTargetAspect = false
    private var targetAspect: Float? = null
    private var borderWidthPx = 1
    private var hasCropRect = false
    private var lastLayoutW = 0
    private var lastLayoutH = 0
    private var startRect = FloatArray(4)
    private var endRect = FloatArray(4)
    private var animStartNs = 0L
    private var animating = false
    private var lastPreviewDrawNs = 0L
    private var choreographer: Choreographer? = null
    private var cropTickScheduled = false
    private val cropFrameCallback = Choreographer.FrameCallback { frameTimeNanos ->
        cropTickScheduled = false
        if (!animating || preview == EGL14.EGL_NO_SURFACE) return@FrameCallback
        if (frameTimeNanos - lastPreviewDrawNs >= MIN_CROP_TICK_INTERVAL_NS) {
            drawPreview(frameTimeNanos)
        }
        if (animating) scheduleCropTick()
    }

    /** The preview's backdrop: two small textures the picture is shrunk and blurred in, back and forth. */
    private val backdrop = IntArray(2)
    private val backdropBuffers = IntArray(2)
    private var backdropWidth = 0
    private var backdropHeight = 0

    /** Degrees the camera's buffer turns clockwise to be upright, from CameraX. */
    @Volatile var rotation = 0

    /** The transform already in the camera texture's own matrix, or null when the camera doesn't write one. From CameraX. */
    @Volatile var cameraTransform: CameraTransform? = null

    /**
     * How the phone is held for the picture being made: CameraX's target rotation, in degrees. The app's screen stays
     * upright, so the preview turns the picture by this much to look upright to whoever holds the phone. The encoder
     * never does: the PC gets the picture as it is made.
     */
    @Volatile var screenTurn = 0

    /** Flips the preview left to right; the encoder never is. */
    @Volatile var mirrorPreview = false

    /** The encoder gets at most this many frames a second; the preview gets them all. */
    @Volatile var fps = 30

    private var bufferWidth = 0
    private var bufferHeight = 0
    private val gate = FrameGate()
    private val texMatrix = FloatArray(16)
    private val scratch = FloatArray(16)

    private val quad: FloatBuffer = floats(-1f, -1f, 1f, -1f, -1f, 1f, 1f, 1f)
    private val uv: FloatBuffer = floats(0f, 0f, 1f, 0f, 0f, 1f, 1f, 1f)

    init {
        handler.post(::setUp)
    }

    /** The surface the camera draws into, sized [width]×[height]. Call on the GL thread ([handler]). */
    fun cameraSurface(width: Int, height: Int): Surface {
        bufferWidth = width
        bufferHeight = height
        val st = cameraTexture!!
        st.setDefaultBufferSize(width, height)
        return cameraSurface ?: Surface(st).also { cameraSurface = it }
    }

    /** The encoder's input surface, or null to stop encoding. GL thread. */
    fun setEncoder(surface: Surface?, width: Int, height: Int) {
        if (encoder != EGL14.EGL_NO_SURFACE) {
            EGL14.eglMakeCurrent(display, idle, idle, context)
            EGL14.eglDestroySurface(display, encoder)
            encoder = EGL14.EGL_NO_SURFACE
        }
        // Without an encoder the preview shows the camera's whole upright picture until the next one comes.
        encoderWidth = 0
        encoderHeight = 0
        if (surface != null) {
            encoder = EGL14.eglCreateWindowSurface(display, config, surface, intArrayOf(EGL14.EGL_NONE), 0)
            encoderWidth = width
            encoderHeight = height
            if (encoder != EGL14.EGL_NO_SURFACE) {
                EGL14.eglMakeCurrent(display, encoder, encoder, context)
                EGL14.eglSwapInterval(display, 0)
                EGL14.eglMakeCurrent(display, idle, idle, context)
            }
        }
    }

    /** The preview's surface, or null while it isn't on screen (or the user turned it off). GL thread. */
    fun setPreview(surface: Surface?, width: Int, height: Int) {
        if (surface != null && surface === previewSurfaceRef && width == previewWidth && height == previewHeight && preview != EGL14.EGL_NO_SURFACE) {
            return
        }
        if (preview != EGL14.EGL_NO_SURFACE) {
            EGL14.eglMakeCurrent(display, idle, idle, context)
            EGL14.eglDestroySurface(display, preview)
            preview = EGL14.EGL_NO_SURFACE
        }
        previewSurfaceRef = null
        if (surface == null) {
            hasCropRect = false
            animating = false
            lastLayoutW = 0
            lastLayoutH = 0
            cancelCropTick()
            // Without a preview its backdrop isn't needed; it is made again, small, when one comes back.
            if (backdropWidth > 0) {
                EGL14.eglMakeCurrent(display, idle, idle, context)
                dropBackdrop()
            }
        }
        if (surface != null && surface.isValid) {
            previewSurfaceRef = surface
            preview = EGL14.eglCreateWindowSurface(display, config, surface, intArrayOf(EGL14.EGL_NONE), 0)
            previewWidth = width
            previewHeight = height
            if (preview != EGL14.EGL_NO_SURFACE) {
                EGL14.eglMakeCurrent(display, preview, preview, context)
                EGL14.eglSwapInterval(display, 0)
                EGL14.eglMakeCurrent(display, idle, idle, context)
                if (hasFrame) drawPreview(System.nanoTime())
            }
        }
    }

    /** Target optical crop aspect ratio (null = full playground) and hairline border width in px. GL thread. */
    fun setPreviewAspect(aspect: Float?, borderPx: Int) {
        borderWidthPx = borderPx.coerceAtLeast(1)
        if (hasTargetAspect && sameAspect(targetAspect, aspect)) return
        hasTargetAspect = true
        targetAspect = aspect
        if (hasCropRect && preview != EGL14.EGL_NO_SURFACE && previewWidth > 0 && previewHeight > 0 && hasFrame) {
            val nowNs = System.nanoTime()
            startRect = currentCropRect(nowNs)
            endRect = cropRectFor(aspect, previewWidth, previewHeight)
            animStartNs = 0L
            animating = !sameRect(startRect, endRect)
            if (animating) {
                scheduleCropTick()
            } else {
                cancelCropTick()
                drawPreview(nowNs)
            }
        }
    }

    private fun scheduleCropTick() {
        if (cropTickScheduled) return
        val c = choreographer ?: return
        cropTickScheduled = true
        c.postFrameCallback(cropFrameCallback)
    }

    private fun cancelCropTick() {
        if (!cropTickScheduled) return
        cropTickScheduled = false
        choreographer?.removeFrameCallback(cropFrameCallback)
    }

    fun release() {
        handler.post {
            cancelCropTick()
            hasFrame = false
            setEncoder(null, 0, 0)
            setPreview(null, 0, 0)
            EGL14.eglMakeCurrent(display, idle, idle, context)
            dropBackdrop()
            cameraSurface?.release()
            cameraTexture?.release()
            EGL14.eglMakeCurrent(display, EGL14.EGL_NO_SURFACE, EGL14.EGL_NO_SURFACE, EGL14.EGL_NO_CONTEXT)
            EGL14.eglDestroySurface(display, idle)
            EGL14.eglDestroyContext(display, context)
            EGL14.eglTerminate(display)
            thread.quitSafely()
        }
    }

    private fun setUp() {
        display = EGL14.eglGetDisplay(EGL14.EGL_DEFAULT_DISPLAY)
        val version = IntArray(2)
        EGL14.eglInitialize(display, version, 0, version, 1)
        val attributes = intArrayOf(
            EGL14.EGL_RED_SIZE, 8, EGL14.EGL_GREEN_SIZE, 8, EGL14.EGL_BLUE_SIZE, 8, EGL14.EGL_ALPHA_SIZE, 8,
            EGL14.EGL_RENDERABLE_TYPE, EGL14.EGL_OPENGL_ES2_BIT, EGL_RECORDABLE_ANDROID, 1, EGL14.EGL_NONE,
        )
        val configs = arrayOfNulls<EGLConfig>(1)
        val count = IntArray(1)
        EGL14.eglChooseConfig(display, attributes, 0, configs, 0, 1, count, 0)
        config = configs[0]
        context = EGL14.eglCreateContext(display, config, EGL14.EGL_NO_CONTEXT, intArrayOf(EGL14.EGL_CONTEXT_CLIENT_VERSION, 2, EGL14.EGL_NONE), 0)
        idle = EGL14.eglCreatePbufferSurface(display, config, intArrayOf(EGL14.EGL_WIDTH, 1, EGL14.EGL_HEIGHT, 1, EGL14.EGL_NONE), 0)
        EGL14.eglMakeCurrent(display, idle, idle, context)

        program = Program(link(VERTEX, FRAGMENT))
        shrink = Program(link(VERTEX_UV, FRAGMENT_SHRINK))
        blur = Program(link(VERTEX_UV, FRAGMENT_BLUR))
        solid = Program(link(VERTEX_SOLID, FRAGMENT_SOLID))
        choreographer = Choreographer.getInstance()
        val t = IntArray(1)
        GLES20.glGenTextures(1, t, 0)
        texture = t[0]
        GLES20.glBindTexture(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, texture)
        GLES20.glTexParameteri(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, GLES20.GL_TEXTURE_MIN_FILTER, GLES20.GL_LINEAR)
        GLES20.glTexParameteri(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, GLES20.GL_TEXTURE_MAG_FILTER, GLES20.GL_LINEAR)
        GLES20.glTexParameteri(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, GLES20.GL_TEXTURE_WRAP_S, GLES20.GL_CLAMP_TO_EDGE)
        GLES20.glTexParameteri(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, GLES20.GL_TEXTURE_WRAP_T, GLES20.GL_CLAMP_TO_EDGE)
        cameraTexture = SurfaceTexture(texture).apply { setOnFrameAvailableListener({ frame() }, handler) }
    }

    private fun frame() {
        val st = cameraTexture ?: return
        EGL14.eglMakeCurrent(display, idle, idle, context)
        try {
            st.updateTexImage()
        } catch (_: RuntimeException) {
            return
        }
        st.getTransformMatrix(texMatrix)
        val ts = st.timestamp
        val nowNs = System.nanoTime()
        val upright = if (rotation % 180 == 0) bufferWidth to bufferHeight else bufferHeight to bufferWidth
        val camera = cameraTransform
        hasFrame = bufferWidth > 0 && bufferHeight > 0
        drawnUprightW = upright.first
        drawnUprightH = upright.second
        drawnRotation = rotation
        drawnTurn = screenTurn
        drawnCamera = camera
        if (encoder != EGL14.EGL_NO_SURFACE && gate.pass(ts, fps)) {
            EGL14.eglMakeCurrent(display, encoder, encoder, context)
            GLES20.glViewport(0, 0, encoderWidth, encoderHeight)
            drawCamera(program, sampled(upright, encoderWidth, encoderHeight, rotation, 0, false, camera))
            EGLExt.eglPresentationTimeANDROID(display, encoder, ts)
            EGL14.eglSwapBuffers(display, encoder)
            outFrameCount++
            if (outFpsWindowStartNs == 0L) {
                outFpsWindowStartNs = nowNs
            } else if (nowNs - outFpsWindowStartNs >= 1_000_000_000L) {
                val measured = (outFrameCount * 1_000_000_000L / (nowNs - outFpsWindowStartNs)).toInt()
                outFrameCount = 0
                outFpsWindowStartNs = nowNs
                onOutputFps?.invoke(measured)
            }
        }
        if (preview != EGL14.EGL_NO_SURFACE && hasFrame) {
            drawPreview(nowNs)
        }
    }

    private fun drawPreview(nowNs: Long) {
        if (preview == EGL14.EGL_NO_SURFACE || !hasFrame || drawnUprightW <= 0 || drawnUprightH <= 0 || previewWidth <= 0 || previewHeight <= 0) {
            return
        }
        lastPreviewDrawNs = nowNs
        if (!hasCropRect || lastLayoutW != previewWidth || lastLayoutH != previewHeight) {
            val target = cropRectFor(targetAspect, previewWidth, previewHeight)
            startRect = target
            endRect = target.copyOf()
            hasCropRect = true
            animating = false
            cancelCropTick()
            lastLayoutW = previewWidth
            lastLayoutH = previewHeight
        }
        if (animating && animStartNs == 0L) {
            animStartNs = nowNs
        }
        val rect = currentCropRect(nowNs)
        if (animating && animStartNs > 0L && nowNs - animStartNs >= CROP_ANIM_NS) {
            animating = false
            startRect = endRect.copyOf()
        }
        val vx = rect[0].roundToInt().coerceIn(0, previewWidth)
        val vy = rect[1].roundToInt().coerceIn(0, previewHeight)
        val vw = rect[2].roundToInt().coerceIn(1, (previewWidth - vx).coerceAtLeast(1))
        val vh = rect[3].roundToInt().coerceIn(1, (previewHeight - vy).coerceAtLeast(1))
        val glY = previewHeight - vy - vh
        val upright = drawnUprightW to drawnUprightH
        val turn = drawnTurn
        val mirror = mirrorPreview
        val camera = drawnCamera
        val aw = if (turn % 180 != 0) vh else vw
        val ah = if (turn % 180 != 0) vw else vh

        EGL14.eglMakeCurrent(display, preview, preview, context)
        GLES20.glViewport(0, 0, previewWidth, previewHeight)
        GLES20.glClearColor(0f, 0f, 0f, 1f)
        GLES20.glClear(GLES20.GL_COLOR_BUFFER_BIT)
        val insetX = previewWidth - vw
        val insetY = previewHeight - vh
        if (insetX > 1 || insetY > 1) {
            drawBackdrop(upright, aw, ah, drawnRotation, turn, mirror, camera)
        }
        GLES20.glViewport(vx, glY, vw, vh)
        drawCamera(program, sampled(upright, aw, ah, drawnRotation, turn, mirror, camera))
        drawCropBorders(vx, glY, vw, vh, insetX, insetY)
        if (!EGL14.eglSwapBuffers(display, preview)) setPreview(null, 0, 0)
    }

    private fun currentCropRect(nowNs: Long): FloatArray {
        if (!animating) return endRect
        if (animStartNs == 0L) return startRect
        val u = ((nowNs - animStartNs).coerceAtLeast(0L).toFloat() / CROP_ANIM_NS).coerceIn(0f, 1f)
        return interpolateCropRect(startRect, endRect, easeOutExpo(u), previewWidth, previewHeight)
    }

    private fun drawCropBorders(vx: Int, glY: Int, vw: Int, vh: Int, insetX: Int, insetY: Int) {
        val linePx = borderWidthPx.coerceAtLeast(1)
        val fadeSpan = (linePx * 3f).coerceAtLeast(1f)
        val alphaX = ((insetX * 0.5f) / fadeSpan).coerceIn(0f, 1f) * CROP_BORDER_ALPHA
        val alphaY = ((insetY * 0.5f) / fadeSpan).coerceIn(0f, 1f) * CROP_BORDER_ALPHA
        if (alphaX <= 0.005f && alphaY <= 0.005f) return
        GLES20.glViewport(0, 0, previewWidth, previewHeight)
        GLES20.glEnable(GLES20.GL_BLEND)
        GLES20.glBlendFunc(GLES20.GL_SRC_ALPHA, GLES20.GL_ONE_MINUS_SRC_ALPHA)
        GLES20.glEnable(GLES20.GL_SCISSOR_TEST)
        GLES20.glUseProgram(solid.id)
        val colorUniform = solid.uniform("uColor")
        if (alphaX > 0.005f) {
            GLES20.glUniform4f(colorUniform, 1f, 1f, 1f, alphaX)
            fillScissor(vx, glY, linePx, vh)
            fillScissor(vx + vw - linePx, glY, linePx, vh)
        }
        if (alphaY > 0.005f) {
            GLES20.glUniform4f(colorUniform, 1f, 1f, 1f, alphaY)
            fillScissor(vx, glY + vh - linePx, vw, linePx)
            fillScissor(vx, glY, vw, linePx)
        }
        GLES20.glDisable(GLES20.GL_SCISSOR_TEST)
        GLES20.glDisable(GLES20.GL_BLEND)
    }

    private fun fillScissor(x: Int, y: Int, w: Int, h: Int) {
        if (w <= 0 || h <= 0) return
        GLES20.glScissor(x, y, w, h)
        drawQuad(solid)
    }

    /** The texture matrix that draws the upright picture (uw×uh) cropped to ow:oh, turned [turn] degrees clockwise. */
    private fun sampled(upright: Pair<Int, Int>, ow: Int, oh: Int, rot: Int, turn: Int, mirror: Boolean, camera: CameraTransform?): FloatArray {
        val (cx, cy) = centerCrop(upright.first, upright.second, ow, oh)
        return sampled(cx, cy, rot, turn, mirror, camera)
    }

    private fun sampled(cx: Float, cy: Float, rot: Int, turn: Int, mirror: Boolean, camera: CameraTransform?): FloatArray {
        Matrix.multiplyMM(scratch, 0, texMatrix, 0, sampling(rot, cx, cy, turn, mirror, camera), 0)
        return scratch
    }

    /** The camera's whole picture, upright for the hold it is made for (width to height), or null before the camera has a size. GL thread. */
    fun uprightFrame(): Pair<Int, Int>? =
        if (bufferWidth <= 0 || bufferHeight <= 0) null else if (rotation % 180 == 0) bufferWidth to bufferHeight else bufferHeight to bufferWidth

    /**
     * The shown picture (the ow:oh part that is sent) shrunk to a few dozen pixels across, cropped to cover the whole
     * preview, blurred both ways and drawn dimmed over all of it. Small enough that its passes cost next to nothing.
     */
    private fun drawBackdrop(upright: Pair<Int, Int>, ow: Int, oh: Int, rot: Int, turn: Int, mirror: Boolean, camera: CameraTransform?) {
        val short = minOf(previewWidth, previewHeight).coerceAtLeast(1)
        val bw = (BACKDROP_SHORT_SIDE * previewWidth / short.toFloat()).roundToInt().coerceAtLeast(1)
        val bh = (BACKDROP_SHORT_SIDE * previewHeight / short.toFloat()).roundToInt().coerceAtLeast(1)
        if (bw != backdropWidth || bh != backdropHeight) makeBackdrop(bw, bh)
        // The preview's shape as the picture sees it: on its side when the phone is.
        val (cw, ch) = if (turn % 180 != 0) previewHeight to previewWidth else previewWidth to previewHeight
        GLES20.glViewport(0, 0, bw, bh)
        GLES20.glBindFramebuffer(GLES20.GL_FRAMEBUFFER, backdropBuffers[0])
        GLES20.glUseProgram(shrink.id)
        GLES20.glUniform2f(shrink.uniform("uStep"), 1f / (3 * bw), 1f / (3 * bh))
        val (sx, sy) = coverCrop(upright.first, upright.second, ow, oh, cw, ch)
        drawCamera(shrink, sampled(sx, sy, rot, turn, mirror, camera))
        blurPass(backdrop[0], backdropBuffers[1], 1f / bw, 0f, 1f)
        blurPass(backdrop[1], backdropBuffers[0], 0f, 1f / bh, 1f)
        GLES20.glBindFramebuffer(GLES20.GL_FRAMEBUFFER, 0)
        GLES20.glViewport(0, 0, previewWidth, previewHeight)
        blurPass(backdrop[0], 0, 0f, 0f, BACKDROP_BRIGHTNESS)
    }

    private fun makeBackdrop(w: Int, h: Int) {
        dropBackdrop()
        GLES20.glGenTextures(2, backdrop, 0)
        GLES20.glGenFramebuffers(2, backdropBuffers, 0)
        for (i in 0..1) {
            GLES20.glBindTexture(GLES20.GL_TEXTURE_2D, backdrop[i])
            GLES20.glTexImage2D(GLES20.GL_TEXTURE_2D, 0, GLES20.GL_RGBA, w, h, 0, GLES20.GL_RGBA, GLES20.GL_UNSIGNED_BYTE, null)
            GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_MIN_FILTER, GLES20.GL_LINEAR)
            GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_MAG_FILTER, GLES20.GL_LINEAR)
            GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_WRAP_S, GLES20.GL_CLAMP_TO_EDGE)
            GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_WRAP_T, GLES20.GL_CLAMP_TO_EDGE)
            GLES20.glBindFramebuffer(GLES20.GL_FRAMEBUFFER, backdropBuffers[i])
            GLES20.glFramebufferTexture2D(GLES20.GL_FRAMEBUFFER, GLES20.GL_COLOR_ATTACHMENT0, GLES20.GL_TEXTURE_2D, backdrop[i], 0)
        }
        GLES20.glBindFramebuffer(GLES20.GL_FRAMEBUFFER, 0)
        backdropWidth = w
        backdropHeight = h
    }

    private fun dropBackdrop() {
        if (backdropWidth == 0) return
        GLES20.glDeleteFramebuffers(2, backdropBuffers, 0)
        GLES20.glDeleteTextures(2, backdrop, 0)
        backdropWidth = 0
        backdropHeight = 0
    }

    /** One blur pass of [source] along (dx, dy) into [target] (0 is the window), at [brightness]. */
    private fun blurPass(source: Int, target: Int, dx: Float, dy: Float, brightness: Float) {
        if (target != 0) GLES20.glBindFramebuffer(GLES20.GL_FRAMEBUFFER, target)
        GLES20.glUseProgram(blur.id)
        GLES20.glActiveTexture(GLES20.GL_TEXTURE0)
        GLES20.glBindTexture(GLES20.GL_TEXTURE_2D, source)
        GLES20.glUniform2f(blur.uniform("uDir"), dx, dy)
        GLES20.glUniform1f(blur.uniform("uBrightness"), brightness)
        drawQuad(blur)
    }

    /** Draws the camera texture with [matrix] into the current viewport, through [program] or [shrink]. */
    private fun drawCamera(prog: Program, matrix: FloatArray) {
        GLES20.glUseProgram(prog.id)
        GLES20.glActiveTexture(GLES20.GL_TEXTURE0)
        GLES20.glBindTexture(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, texture)
        GLES20.glUniformMatrix4fv(prog.uniform("uTex"), 1, false, matrix, 0)
        drawQuad(prog)
    }

    private fun drawQuad(prog: Program) {
        val position = prog.position
        val coord = prog.coord
        GLES20.glEnableVertexAttribArray(position)
        GLES20.glVertexAttribPointer(position, 2, GLES20.GL_FLOAT, false, 0, quad)
        if (coord >= 0) {
            GLES20.glEnableVertexAttribArray(coord)
            GLES20.glVertexAttribPointer(coord, 2, GLES20.GL_FLOAT, false, 0, uv)
        }
        GLES20.glDrawArrays(GLES20.GL_TRIANGLE_STRIP, 0, 4)
        GLES20.glDisableVertexAttribArray(position)
        if (coord >= 0) GLES20.glDisableVertexAttribArray(coord)
    }

    private fun link(vertex: String, fragment: String): Int {
        fun shader(type: Int, source: String) = GLES20.glCreateShader(type).also {
            GLES20.glShaderSource(it, source)
            GLES20.glCompileShader(it)
        }
        return GLES20.glCreateProgram().also {
            GLES20.glAttachShader(it, shader(GLES20.GL_VERTEX_SHADER, vertex))
            GLES20.glAttachShader(it, shader(GLES20.GL_FRAGMENT_SHADER, fragment))
            GLES20.glLinkProgram(it)
        }
    }

    /** A linked program and its locations, looked up once: by name on every draw, each costs a round trip through JNI. GL thread. */
    private class Program(val id: Int) {
        val position = GLES20.glGetAttribLocation(id, "aPosition")
        val coord = GLES20.glGetAttribLocation(id, "aCoord")
        private val uniforms = HashMap<String, Int>()

        fun uniform(name: String) = uniforms.getOrPut(name) { GLES20.glGetUniformLocation(id, name) }
    }

    private companion object {
        /** EGL_RECORDABLE_ANDROID: the config can feed a MediaCodec input surface. */
        const val EGL_RECORDABLE_ANDROID = 0x3142

        /** The backdrop's short side in pixels: small enough to blur cheaply, big enough not to flicker. */
        const val BACKDROP_SHORT_SIDE = 36f

        /** How bright the backdrop is drawn, so the picture itself stands out. */
        const val BACKDROP_BRIGHTNESS = 0.55f

        /** Duration of the smooth real-time optical crop transition, matching the HUD curve. */
        const val CROP_ANIM_NS = 280_000_000L

        /** Skip duplicate Choreographer draws when a camera frame just rendered within this window. */
        const val MIN_CROP_TICK_INTERVAL_NS = 4_000_000L

        /** Translucent white hairline matching prototype `.cam-frame` border (0.22 alpha). */
        const val CROP_BORDER_ALPHA = 0.22f

        const val VERTEX_SOLID = """
            attribute vec4 aPosition;
            void main() {
                gl_Position = aPosition;
            }
        """

        const val FRAGMENT_SOLID = """
            precision mediump float;
            uniform vec4 uColor;
            void main() {
                gl_FragColor = uColor;
            }
        """

        const val VERTEX = """
            attribute vec4 aPosition;
            attribute vec4 aCoord;
            uniform mat4 uTex;
            varying vec2 vCoord;
            void main() {
                gl_Position = aPosition;
                vCoord = (uTex * aCoord).xy;
            }
        """

        const val FRAGMENT = """
            #extension GL_OES_EGL_image_external : require
            precision mediump float;
            varying vec2 vCoord;
            uniform samplerExternalOES sTex;
            void main() {
                gl_FragColor = texture2D(sTex, vCoord);
            }
        """

        const val VERTEX_UV = """
            attribute vec4 aPosition;
            attribute vec4 aCoord;
            varying vec2 vUv;
            void main() {
                gl_Position = aPosition;
                vUv = aCoord.xy;
            }
        """

        /** Nine samples across each small pixel, so shrinking a big picture doesn't shimmer. */
        const val FRAGMENT_SHRINK = """
            #extension GL_OES_EGL_image_external : require
            precision mediump float;
            varying vec2 vUv;
            uniform mat4 uTex;
            uniform vec2 uStep;
            uniform samplerExternalOES sTex;
            void main() {
                vec4 sum = vec4(0.0);
                for (int i = -1; i <= 1; i++) {
                    for (int j = -1; j <= 1; j++) {
                        sum += texture2D(sTex, (uTex * vec4(vUv + vec2(float(i), float(j)) * uStep, 0.0, 1.0)).xy);
                    }
                }
                gl_FragColor = sum / 9.0;
            }
        """

        /** A nine-tap Gaussian along uDir, in five samples thanks to linear filtering. uDir of zero just copies. */
        const val FRAGMENT_BLUR = """
            precision mediump float;
            varying vec2 vUv;
            uniform sampler2D sTex;
            uniform vec2 uDir;
            uniform float uBrightness;
            void main() {
                vec4 c = texture2D(sTex, vUv) * 0.2270270270
                    + (texture2D(sTex, vUv + uDir * 1.3846153846) + texture2D(sTex, vUv - uDir * 1.3846153846)) * 0.3162162162
                    + (texture2D(sTex, vUv + uDir * 3.2307692308) + texture2D(sTex, vUv - uDir * 3.2307692308)) * 0.0702702703;
                gl_FragColor = vec4(c.rgb * uBrightness, 1.0);
            }
        """

        fun floats(vararg v: Float): FloatBuffer =
            ByteBuffer.allocateDirect(v.size * 4).order(ByteOrder.nativeOrder()).asFloatBuffer().apply {
                put(v)
                position(0)
            }
    }
}

/**
 * Where a picture of [ow]×[oh] goes in a preview of [pw]×[ph]: zoomed to fit (cover) the playground screen,
 * and on its side when [turn] is a quarter. Returns x, y, width and height.
 */
internal fun fitPreview(ow: Int, oh: Int, pw: Int, ph: Int, turn: Int): IntArray {
    val (fw, fh) = if (turn % 180 != 0) oh to ow else ow to oh
    val scale = maxOf(pw.toFloat() / fw, ph.toFloat() / fh)
    val w = (fw * scale).toInt()
    val h = (fh * scale).toInt()
    return intArrayOf((pw - w) / 2, (ph - h) / 2, w, h)
}

/** Target centered optical crop rectangle [x, y, w, h] inside [pw]×[ph] for [targetAr] (null = full container). */
internal fun cropRectFor(targetAr: Float?, pw: Int, ph: Int): FloatArray {
    val w = pw.toFloat()
    val h = ph.toFloat()
    if (targetAr == null || targetAr <= 0f || pw <= 0 || ph <= 0) return floatArrayOf(0f, 0f, w, h)
    val containerAr = w / h
    if (abs(containerAr - targetAr) < 0.01f) return floatArrayOf(0f, 0f, w, h)
    return if (containerAr > targetAr) {
        val cw = h * targetAr
        floatArrayOf((w - cw) * 0.5f, 0f, cw, h)
    } else {
        val ch = w / targetAr
        floatArrayOf(0f, (h - ch) * 0.5f, w, ch)
    }
}

/** Symmetrically interpolates two centered crop rectangles inside [pw]×[ph]. */
internal fun interpolateCropRect(from: FloatArray, to: FloatArray, t: Float, pw: Int, ph: Int): FloatArray {
    val f = t.coerceIn(0f, 1f)
    val w = from[2] + (to[2] - from[2]) * f
    val h = from[3] + (to[3] - from[3]) * f
    return floatArrayOf((pw - w) * 0.5f, (ph - h) * 0.5f, w, h)
}

/** Matches cubic-bezier(0.16, 1.0, 0.3, 1.0) EaseOutExpo used by the camera HUD. */
internal fun easeOutExpo(x: Float): Float {
    val clamped = x.coerceIn(0f, 1f)
    if (clamped <= 0f || clamped >= 1f) return clamped
    val cx = 3f * 0.16f
    val bx = 3f * (0.3f - 0.16f) - cx
    val ax = 1f - cx - bx
    var t = clamped
    repeat(4) {
        val xt = ((ax * t + bx) * t + cx) * t - clamped
        val dx = (3f * ax * t + 2f * bx) * t + cx
        if (abs(dx) < 1e-5f) return@repeat
        t = (t - xt / dx).coerceIn(0f, 1f)
    }
    val cy = 3f
    val by = -cy
    val ay = 1f - cy - by
    return (((ay * t + by) * t + cy) * t).coerceIn(0f, 1f)
}

private fun sameAspect(a: Float?, b: Float?): Boolean = when {
    a == null && b == null -> true
    a == null || b == null -> false
    else -> abs(a - b) < 1e-4f
}

private fun sameRect(a: FloatArray, b: FloatArray): Boolean =
    abs(a[0] - b[0]) < 0.5f && abs(a[1] - b[1]) < 0.5f && abs(a[2] - b[2]) < 0.5f && abs(a[3] - b[3]) < 0.5f
