package com.owlmic.ui

import android.view.OrientationEventListener
import android.view.Surface
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.runtime.Stable
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.layout
import androidx.compose.ui.unit.Constraints
import com.owlmic.core.design.Tokens
import com.owlmic.media.camera.surfaceRotationFor

/*
 * The screen stays upright (the activity is locked to portrait). When the phone is held sideways, the grid turns with it
 * and everything drawn on it turns back a quarter, so it reads upright, as in the prototype (website/proto-templates).
 */

/** Upright. */
const val TURN_NONE = 0

/** Sideways with the phone's top to the left: glyphs turn a quarter clockwise. */
const val TURN_LEFT = 90

/** Sideways with the phone's top to the right: glyphs turn a quarter anticlockwise. */
const val TURN_RIGHT = 270

/**
 * The next quarter turn from the orientation sensor's [degrees], with the camera's own thresholds: a change needs to be
 * 20° past the halfway point, so a phone held near a diagonal doesn't flicker. Upside down, and lying flat, keep [current].
 */
fun nextTurn(degrees: Int, current: Int): Int {
    if (degrees == OrientationEventListener.ORIENTATION_UNKNOWN) return current
    val surface = when (current) {
        TURN_LEFT -> Surface.ROTATION_90
        TURN_RIGHT -> Surface.ROTATION_270
        else -> Surface.ROTATION_0
    }
    return when (surfaceRotationFor(degrees, surface)) {
        Surface.ROTATION_0 -> TURN_NONE
        Surface.ROTATION_90 -> TURN_LEFT
        Surface.ROTATION_270 -> TURN_RIGHT
        else -> current
    }
}

/** How far glyphs turn, clockwise, to read upright. */
fun uprightAngle(turn: Int): Float = when (turn) {
    TURN_LEFT -> 90f
    TURN_RIGHT -> -90f
    else -> 0f
}

/** A [turn] and the [angle] things are drawn at for it. */
class Upright(val turn: Int, val angle: State<Float>) {
    val sideways get() = turn != TURN_NONE
}

/**
 * A change of hold, in three steps, so nothing on the preview ever jumps: what is drawn on the preview fades out, the
 * tiles' icons turn, then the preview's content fades back in, laid out for the new hold.
 */
@Stable
class TurnSequence(start: Int) {
    private var iconTurn by mutableIntStateOf(start)
    private val iconAngle = Animatable(uprightAngle(start))

    /** The preview's content (and the drawers and settings over it): laid out for this turn, faded by [fade]. */
    var layoutTurn by mutableIntStateOf(start)
        private set
    private val layoutAngle = mutableFloatStateOf(uprightAngle(start))
    private val fading = Animatable(1f)

    val icons get() = Upright(iconTurn, iconAngle.asState())
    val layout get() = Upright(layoutTurn, layoutAngle)
    val fade: State<Float> get() = fading.asState()

    /** Runs the three steps towards [turn]; [onLayout] hears the moment the content changes, while it is faded out. */
    suspend fun follow(turn: Int, onLayout: (Int) -> Unit) {
        val fadeMs = Tokens.Phone.TURN_FADE_MS.toInt()
        if (turn == layoutTurn && turn == iconTurn) {
            // The first hold (the app may open sideways), or turned back before anything moved: undo any fade that had
            // started, and say how the content is laid out.
            onLayout(turn)
            fading.animateTo(1f, tween(fadeMs))
            return
        }
        fading.animateTo(0f, tween(fadeMs))
        iconTurn = turn
        iconAngle.animateTo(uprightAngle(turn), tween(Tokens.Phone.TURN_MS.toInt()))
        layoutTurn = turn
        layoutAngle.floatValue = uprightAngle(turn)
        onLayout(turn)
        fading.animateTo(1f, tween(fadeMs))
    }
}

/**
 * Draws this element turned with [up]. With [swap], text and other wide content is laid out for the reader's width
 * while sideways (the tile's height on screen), not the screen's.
 */
fun Modifier.upright(up: Upright, swap: Boolean = false): Modifier {
    val laidOut = if (swap && up.sideways) {
        layout { measurable, constraints ->
            val p = measurable.measure(Constraints(constraints.minHeight, constraints.maxHeight, constraints.minWidth, constraints.maxWidth))
            layout(p.height, p.width) { p.place((p.height - p.width) / 2, (p.width - p.height) / 2) }
        }
    } else {
        this
    }
    return laidOut.graphicsLayer { rotationZ = up.angle.value }
}

/** The wordmark's letters in screen order, left to right: reversed when the reader's top is the screen's right. */
fun wordmarkOrder(turn: Int): List<Char> = if (turn == TURN_LEFT) "owlmic".reversed().toList() else "owlmic".toList()
