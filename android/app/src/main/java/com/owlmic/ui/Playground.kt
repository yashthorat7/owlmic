package com.owlmic.ui

import android.view.SurfaceHolder
import android.view.SurfaceView
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.lerp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import com.owlmic.Owlmic
import com.owlmic.R
import com.owlmic.core.design.SettingsModel
import com.owlmic.core.design.Tokens
import com.owlmic.core.hub.AppState
import com.owlmic.core.hub.Connection
import com.owlmic.core.hub.FeaturePhase
import com.owlmic.hub.AppMsg
import kotlinx.coroutines.delay
import com.owlmic.core.R as Copy

/**
 * The middle tile: the owl on a dot grid while nothing is on, the mic's sound wave, what the camera sends, and the
 * status strip when there is something to read or do. Everything drawn on it is laid out for the reader
 * ([TurnSequence]'s layout) and fades with a change of hold; the camera's picture underneath is turned by its own GL
 * pass. The drawers open over it.
 *
 * The round buttons (the camera's eye, mirror and flip; the mic's settings) rest at 20%: a touch anywhere on the
 * preview brings them up, and 3 s without one lets them down again. They work at either.
 */
@Composable
fun Playground(seq: TurnSequence, state: AppState, drawer: Drawer?, modifier: Modifier, onStrip: (StripAction) -> Unit) {
    val camera = state.camera
    // Paused for Bluetooth, the camera is off: its tile says why instead of the preview showing nothing.
    val live = (camera.isOn || camera.phase == FeaturePhase.PAUSED) && camera.problem == null
    val mic = state.mic.isOn

    // The mic's settings drawer: closed by default, and whenever the layout under it changes.
    var open by remember { mutableStateOf(false) }
    LaunchedEffect(live, mic) { open = false }
    BackHandler(open) { open = false }
    // The eye: the preview off (black), for someone who doesn't want to watch themselves. The PC still gets the picture.
    var watching by rememberSaveable { mutableStateOf(true) }

    var awake by remember { mutableStateOf(false) }
    var touches by remember { mutableIntStateOf(0) }
    LaunchedEffect(touches) {
        if (touches == 0) return@LaunchedEffect
        awake = true
        delay(Tokens.Phone.CONTROLS_IDLE_MS.toLong())
        awake = false
    }
    val controls by animateFloatAsState(
        if (awake) 1f else Tokens.Phone.CONTROLS_DIM_ALPHA,
        tween(Tokens.Phone.CONTROLS_FADE_MS.toInt()),
        label = "controls",
    )

    Box(modifier.clip(RoundedCornerShape(Theme.radius)).background(Theme.bg)) {
        if (!live) DotGrid(Modifier.fillMaxSize())
        if (live && watching) Preview()
        if (live) Frame(Modifier.fillMaxSize())
        Content(
            seq.layout,
            seq.fade,
            state,
            Mode(live, mic, open, watching),
            controls,
            onOpen = { open = it },
            onWatch = { watching = it },
            onTouch = { touches++ },
            onStrip = onStrip,
        )
        AnimatedDrawer(seq, drawer, state)
    }
}

/** What the preview shows: the camera's picture ([live]), the mic, the mic's drawer [open], the picture [watching]. */
private data class Mode(val live: Boolean, val mic: Boolean, val open: Boolean, val watching: Boolean)

/** The wave's move between the whole preview and the capsule, and the drawer's opening. */
private const val MOVE_MS = 420

@Composable
private fun Content(
    up: Upright,
    fade: State<Float>,
    state: AppState,
    mode: Mode,
    controls: Float,
    onOpen: (Boolean) -> Unit,
    onWatch: (Boolean) -> Unit,
    onTouch: () -> Unit,
    onStrip: (StripAction) -> Unit,
) {
    BoxWithConstraints(Modifier.fillMaxSize().upright(up, swap = true).graphicsLayer { alpha = fade.value }.onAnyTouch(onTouch)) {
        val inset = if (up.sideways) Tokens.Phone.STRIP_SIDE_INSET_DP.dp else Tokens.Phone.STRIP_INSET_DP.dp
        // 80% of the phone's width upright, and the same width sideways: the window's short side either way.
        val window = LocalWindowInfo.current.containerSize
        val short = with(LocalDensity.current) { minOf(window.width, window.height).toDp() }
        val capsule = minOf(short * Tokens.Phone.CAPSULE_WIDTH_FRACTION, maxWidth - 32.dp)

        AnimatedVisibility(!mode.live && !mode.mic, Modifier.align(Alignment.Center), enter = fadeIn(tween(Theme.STATE_MS)), exit = fadeOut(tween(Theme.STATE_MS))) {
            Image(
                painterResource(R.drawable.owl),
                null,
                Modifier.size(Tokens.Phone.OWL_DP.dp).graphicsLayer { alpha = Tokens.Phone.OWL_ALPHA },
            )
        }

        // The strip sits at the reader's bottom; the capsule stays above it.
        val strip = stripOf(state)
        var stripHeight by remember { mutableIntStateOf(0) }
        val above = with(LocalDensity.current) { if (strip != null) stripHeight.toDp() + 12.dp else 0.dp }
        AnimatedVisibility(
            strip != null,
            Modifier.align(Alignment.BottomCenter).padding(bottom = inset).onSizeChanged { stripHeight = it.height },
            enter = fadeIn(tween(Theme.STATE_MS)),
            exit = fadeOut(tween(Theme.STATE_MS)),
        ) {
            // The last strip stays while it fades out.
            var last by remember { mutableStateOf(Strip.CODE) }
            if (strip != null) last = strip
            StatusStrip(state, last, Modifier.width(capsule), onStrip)
        }

        if (mode.mic) {
            // 0: the wave fills the preview (only the mic on); 1: it sits in the capsule at the bottom (with the camera).
            val dock by animateFloatAsState(if (mode.live) 1f else 0f, tween(MOVE_MS), label = "dock")
            WaveHost(state, dock, mode.open, Bounds(maxWidth, maxHeight, capsule, inset + above), onOpen)
        }

        if (mode.live) {
            val mirrored = (state.settings["camera.mirrorPreview"] ?: SettingsModel.find("camera.mirrorPreview")?.default) == "on"
            Row(
                Modifier.align(Alignment.TopEnd).padding(12.dp).graphicsLayer { alpha = controls },
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                RoundButton(if (mode.watching) Glyph.EYE else Glyph.EYE_OFF, stringResource(Copy.string.phone_preview), on = !mode.watching) {
                    onWatch(!mode.watching)
                }
                RoundButton(Glyph.MIRROR, stringResource(Copy.string.setting_camera_mirror_preview), on = mirrored) {
                    Owlmic.post(AppMsg.Setting("camera.mirrorPreview", if (mirrored) "off" else "on"))
                }
                RoundButton(Glyph.FLIP, stringResource(Copy.string.phone_flip_camera)) { Owlmic.post(AppMsg.FlipLens) }
            }
        } else if (mode.mic) {
            RoundButton(
                Glyph.SLIDERS,
                stringResource(Copy.string.ui_settings),
                Modifier.align(Alignment.BottomEnd).padding(12.dp).graphicsLayer { alpha = if (mode.open) 1f else controls },
                on = mode.open,
            ) { onOpen(!mode.open) }
        }
    }
}

/** The preview's size for the reader, and the capsule's width and distance from the bottom. */
private class Bounds(val width: Dp, val height: Dp, val capsule: Dp, val bottom: Dp)

/**
 * The sound wave and the mic's settings. With only the mic on ([dock] 0) it fills the preview, and the settings button
 * opens the drawer from the top. With the camera ([dock] 1) it is the capsule at the bottom: tapping it grows it upward
 * into the drawer. Either way the drawer ends in a line, with the wave under it.
 */
@Composable
private fun WaveHost(state: AppState, dock: Float, open: Boolean, b: Bounds, onOpen: (Boolean) -> Unit) {
    val docked = dock >= 1f
    val capsuleHeight = Tokens.Phone.CAPSULE_HEIGHT_DP.dp
    val shape = RoundedCornerShape(Theme.radius)
    Box(Modifier.fillMaxSize().padding(bottom = lerp(0.dp, b.bottom, dock)), contentAlignment = Alignment.BottomCenter) {
        Column(
            Modifier
                .width(lerp(b.width, b.capsule, dock))
                .then(if (docked) Modifier.wrapContentHeight() else Modifier.height(lerp(b.height, capsuleHeight, dock)))
                .clip(shape)
                .background(Theme.bg.copy(alpha = dock))
                .border(1.dp, Theme.stripEdge.copy(alpha = Theme.stripEdge.alpha * dock), shape),
        ) {
            AnimatedVisibility(
                open,
                enter = expandVertically(tween(MOVE_MS), expandFrom = Alignment.Top) + fadeIn(tween(MOVE_MS)),
                exit = shrinkVertically(tween(MOVE_MS), shrinkTowards = Alignment.Top) + fadeOut(tween(Theme.STATE_MS)),
            ) {
                // In the capsule, the drawer stops short of the round camera buttons at the top.
                val room = if (docked) b.height - b.bottom - capsuleHeight - Tokens.Phone.ROUND_BUTTON_DP.dp - 40.dp else b.height * 0.6f
                Column {
                    Panel(Modifier.heightIn(max = room)) { MicControls(state) }
                    Box(Modifier.fillMaxWidth().padding(horizontal = 16.dp).height(1.dp).background(Theme.stripEdge))
                }
            }
            // The wave; in the capsule, tapping it opens or closes the drawer.
            val tap = if (docked) {
                Modifier.clickable(remember { MutableInteractionSource() }, null, role = Role.Button, onClickLabel = stringResource(Copy.string.ui_settings)) {
                    onOpen(!open)
                }
            } else {
                Modifier
            }
            Box(
                (if (docked) Modifier.fillMaxWidth().height(capsuleHeight) else Modifier.fillMaxWidth().weight(1f)).then(tap),
                contentAlignment = Alignment.Center,
            ) {
                // Only the mic: a wave no wider than the capsule would be, not edge to edge.
                val alone = minOf(b.width * 0.6f, b.capsule * 0.86f)
                Waveform(Modifier.width(lerp(alone, b.capsule * 0.86f, dock)).fillMaxHeight(0.45f + 0.05f * dock))
            }
        }
    }
}

@Composable
private fun Panel(modifier: Modifier, content: @Composable ColumnScope.() -> Unit) {
    Column(
        modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(start = 20.dp, end = 20.dp, top = 18.dp, bottom = 12.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
        content = content,
    )
}

@Composable
private fun StatusStrip(state: AppState, strip: Strip, modifier: Modifier, onStrip: (StripAction) -> Unit) {
    val action = stripAction(strip)
    val tap = if (action != null) Modifier.clickable(role = Role.Button) { onStrip(action) } else Modifier
    val shape = RoundedCornerShape(Theme.radius)
    Column(
        modifier.clip(shape).background(Theme.bg).border(1.dp, Theme.stripEdge, shape).then(tap).padding(horizontal = 18.dp, vertical = 12.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        val c = state.connection
        if (strip == Strip.CODE && c is Connection.Approving) {
            // The PC asks the user to check this code against its own.
            if (c.pc.isNotEmpty()) BasicText(c.pc, style = stripStyle().copy(color = Theme.text, fontWeight = Theme.title.fontWeight), maxLines = 1)
            BasicText(c.code, style = stripStyle().copy(fontFamily = Theme.mono, fontSize = Theme.display.fontSize, lineHeight = Theme.display.lineHeight, color = Theme.text))
        } else {
            BasicText(stripText(state, strip), style = stripStyle(), maxLines = 3)
        }
    }
}

@Composable
private fun stripStyle() = Theme.body.copy(color = Theme.ink, fontSize = 16.sp, lineHeight = 21.sp, textAlign = TextAlign.Center)

@Composable
private fun stripText(state: AppState, strip: Strip): String = when (val c = state.connection) {
    is Connection.Denied -> stringResource(Copy.string.status_denied, c.pc)
    is Connection.Busy -> stringResource(Copy.string.status_busy, c.pc, c.owner)
    is Connection.UpdateNeeded -> stringResource(Copy.string.status_update, stringResource(if (c.phoneOutdated) Copy.string.device_phone else Copy.string.device_pc))
    is Connection.KeyChanged -> stringResource(Copy.string.status_key_changed, c.pc)
    else -> when (strip) {
        Strip.CHOOSE -> stringResource(Copy.string.phone_choose_pc)
        Strip.TETHERING -> stringResource(Copy.string.ui_open_tethering)
        Strip.BLUETOOTH -> stringResource(Copy.string.bt_try)
        else -> ""
    }
}

/** Exactly what the PC receives, drawn by the camera's own GL pass into this surface. */
@Composable
private fun Preview() {
    AndroidView(
        factory = { context ->
            SurfaceView(context).apply {
                keepScreenOn = true
                holder.addCallback(
                    object : SurfaceHolder.Callback {
                        override fun surfaceCreated(holder: SurfaceHolder) = Unit

                        override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) =
                            Owlmic.setPreview(holder.surface, width, height)

                        override fun surfaceDestroyed(holder: SurfaceHolder) = Owlmic.setPreview(null, 0, 0)
                    },
                )
            }
        },
        modifier = Modifier.fillMaxSize(),
    )
}

/**
 * Over the picture: the grid's seam painted into the corners (a SurfaceView ignores Compose's clip), and a black line
 * as wide as the seam all round, so the preview stays a tile of its own even under a bright picture.
 */
@Composable
private fun Frame(modifier: Modifier) {
    Canvas(modifier) {
        val r = Theme.radius.toPx()
        val line = Theme.gap.toPx()
        val all = Rect(0f, 0f, size.width, size.height)
        val path = Path().apply {
            fillType = PathFillType.EvenOdd
            addRect(all)
            addRoundRect(RoundRect(all, CornerRadius(r, r)))
        }
        drawPath(path, Theme.seam)
        drawRoundRect(
            Theme.bg,
            topLeft = Offset(line / 2, line / 2),
            size = Size(size.width - line, size.height - line),
            cornerRadius = CornerRadius(r - line / 2, r - line / 2),
            style = Stroke(line),
        )
    }
}
