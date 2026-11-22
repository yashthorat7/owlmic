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
import android.view.Surface
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.FloatBuffer
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
    private var texture = 0
    private var cameraTexture: SurfaceTexture? = null
    private var cameraSurface: Surface? = null

    private var encoder: EGLSurface = EGL14.EGL_NO_SURFACE
    private var encoderWidth = 0
    private var encoderHeight = 0
    private var preview: EGLSurface = EGL14.EGL_NO_SURFACE
    private var previewWidth = 0
    private var previewHeight = 0

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
        }
    }

    /** The preview's surface, or null while it isn't on screen (or the user turned it off). GL thread. */
    fun setPreview(surface: Surface?, width: Int, height: Int) {
        if (preview != EGL14.EGL_NO_SURFACE) {
            EGL14.eglMakeCurrent(display, idle, idle, context)
            EGL14.eglDestroySurface(display, preview)
            preview = EGL14.EGL_NO_SURFACE
        }
        // Without a preview its backdrop isn't needed; it is made again, small, when one comes back.
        if (surface == null && backdropWidth > 0) {
            EGL14.eglMakeCurrent(display, idle, idle, context)
            dropBackdrop()
        }
        if (surface != null && surface.isValid) {
            preview = EGL14.eglCreateWindowSurface(display, config, surface, intArrayOf(EGL14.EGL_NONE), 0)
            previewWidth = width
            previewHeight = height
        }
    }

    fun release() {
        handler.post {
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
        val upright = if (rotation % 180 == 0) bufferWidth to bufferHeight else bufferHeight to bufferWidth
        val camera = cameraTransform
        if (encoder != EGL14.EGL_NO_SURFACE && gate.pass(ts, fps)) {
            EGL14.eglMakeCurrent(display, encoder, encoder, context)
            GLES20.glViewport(0, 0, encoderWidth, encoderHeight)
            drawCamera(program, sampled(upright, encoderWidth, encoderHeight, 0, false, camera))
            EGLExt.eglPresentationTimeANDROID(display, encoder, ts)
            EGL14.eglSwapBuffers(display, encoder)
        }
        if (preview != EGL14.EGL_NO_SURFACE) {
            // Exactly what is sent: the encoder's framing, fitted inside the preview, turned the way the phone is held.
            // Around it, rather than black bars, the same picture blurred and dimmed to fill the preview.
            val ow = if (encoderWidth > 0) encoderWidth else upright.first
            val oh = if (encoderHeight > 0) encoderHeight else upright.second
            val turn = screenTurn
            val mirror = mirrorPreview
            val (x, y, w, h) = fitPreview(ow, oh, previewWidth, previewHeight, turn)
            EGL14.eglMakeCurrent(display, preview, preview, context)
            GLES20.glViewport(0, 0, previewWidth, previewHeight)
            GLES20.glClearColor(0f, 0f, 0f, 1f)
            GLES20.glClear(GLES20.GL_COLOR_BUFFER_BIT)
            // Only where fitting leaves room: a picture that fills the preview gets nothing behind it.
            if (w < previewWidth - 1 || h < previewHeight - 1) drawBackdrop(upright, ow, oh, turn, mirror, camera)
            GLES20.glViewport(x, y, w, h)
            drawCamera(program, sampled(upright, ow, oh, turn, mirror, camera))
            if (!EGL14.eglSwapBuffers(display, preview)) setPreview(null, 0, 0)
        }
    }

    /** The texture matrix that draws the upright picture (uw×uh) cropped to ow:oh, turned [turn] degrees clockwise. */
    private fun sampled(upright: Pair<Int, Int>, ow: Int, oh: Int, turn: Int, mirror: Boolean, camera: CameraTransform?): FloatArray {
        val (cx, cy) = centerCrop(upright.first, upright.second, ow, oh)
        return sampled(cx, cy, turn, mirror, camera)
    }

    private fun sampled(cx: Float, cy: Float, turn: Int, mirror: Boolean, camera: CameraTransform?): FloatArray {
        Matrix.multiplyMM(scratch, 0, texMatrix, 0, sampling(rotation, cx, cy, turn, mirror, camera), 0)
        return scratch
    }

    /** The camera's whole picture, upright for the hold it is made for (width to height), or null before the camera has a size. GL thread. */
    fun uprightFrame(): Pair<Int, Int>? =
        if (bufferWidth <= 0 || bufferHeight <= 0) null else if (rotation % 180 == 0) bufferWidth to bufferHeight else bufferHeight to bufferWidth

    /**
     * The shown picture (the ow:oh part that is sent) shrunk to a few dozen pixels across, cropped to cover the whole
     * preview, blurred both ways and drawn dimmed over all of it. Small enough that its passes cost next to nothing.
     */
    private fun drawBackdrop(upright: Pair<Int, Int>, ow: Int, oh: Int, turn: Int, mirror: Boolean, camera: CameraTransform?) {
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
        drawCamera(shrink, sampled(sx, sy, turn, mirror, camera))
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
        GLES20.glEnableVertexAttribArray(coord)
        GLES20.glVertexAttribPointer(coord, 2, GLES20.GL_FLOAT, false, 0, uv)
        GLES20.glDrawArrays(GLES20.GL_TRIANGLE_STRIP, 0, 4)
        GLES20.glDisableVertexAttribArray(position)
        GLES20.glDisableVertexAttribArray(coord)
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
