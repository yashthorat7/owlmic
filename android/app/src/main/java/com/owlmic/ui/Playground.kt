package com.owlmic.ui

import android.view.SurfaceHolder
import android.view.SurfaceView
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.Crossfade
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
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
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
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
import androidx.compose.runtime.setValue
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.Alignment
import androidx.compose.ui.BiasAlignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
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
 * ([TurnSequence]'s layout) and fades with a change of hold.
 */
@Composable
fun Playground(seq: TurnSequence, state: AppState, drawer: Drawer?, modifier: Modifier, onStrip: (StripAction) -> Unit) {
    val camera = state.camera
    val live = (camera.isOn || camera.phase == FeaturePhase.PAUSED) && camera.problem == null
    val mic = state.mic.isOn

    var micOpen by remember { mutableStateOf(false) }
    var camMenuOpen by remember { mutableStateOf(false) }
    var camMenuTouches by remember { mutableIntStateOf(0) }
    val previewHidden = (state.settings["camera.hidePreview"] ?: SettingsModel.find("camera.hidePreview")?.default) == "on"
    val physicalSideways = seq.targetTurn != TURN_NONE
    var prevPhysicalSideways by remember { mutableStateOf(physicalSideways) }
    var pendingRatio by remember { mutableStateOf<Pair<String, String>?>(null) }
    LaunchedEffect(physicalSideways) {
        if (physicalSideways != prevPhysicalSideways) {
            prevPhysicalSideways = physicalSideways
            val current = state.settings["camera.ratio"] ?: "full"
            val flipped = flipRatioForOrientation(current, physicalSideways)
            if (flipped != current) {
                pendingRatio = current to flipped
                Owlmic.post(AppMsg.Setting("camera.ratio", flipped))
            }
        }
    }
    val rawRatio = state.settings["camera.ratio"] ?: "full"
    if (pendingRatio != null && rawRatio != pendingRatio!!.first) {
        pendingRatio = null
    }
    val displayRatio = if (pendingRatio != null && rawRatio == pendingRatio!!.first) pendingRatio!!.second else rawRatio

    LaunchedEffect(live, mic, drawer) {
        if (drawer != null || !live) camMenuOpen = false
        if (drawer != null || !mic) micOpen = false
    }
    LaunchedEffect(camMenuOpen, camMenuTouches) {
        if (!camMenuOpen) return@LaunchedEffect
        delay(Tokens.Phone.CONTROLS_IDLE_MS.toLong())
        camMenuOpen = false
    }
    BackHandler(micOpen || camMenuOpen) {
        if (camMenuOpen) camMenuOpen = false else micOpen = false
    }

    var awake by remember { mutableStateOf(false) }
    var touches by remember { mutableIntStateOf(0) }
    LaunchedEffect(live) {
        if (live) touches++
    }
    LaunchedEffect(touches) {
        if (touches == 0) return@LaunchedEffect
        awake = true
        delay(Tokens.Phone.CONTROLS_IDLE_MS.toLong())
        awake = false
    }

    val hudQuiet = !awake && !camMenuOpen && !(live && mic && micOpen)
    val hudAlpha by animateFloatAsState(
        if (hudQuiet) 0.15f else 1f,
        tween(Tokens.Phone.CONTROLS_FADE_MS.toInt()),
        label = "hudAlpha",
    )

    Box(modifier.clip(RoundedCornerShape(Theme.radius)).background(Theme.bg)) {
        val showLive = live && !previewHidden
        if (!showLive) {
            DotGrid(Modifier.fillMaxSize())
        } else {
            PreviewFrame(displayRatio, physicalSideways)
            Frame(Modifier.fillMaxSize())
        }
        Content(
            seq.layout,
            seq.fade,
            state,
            displayRatio,
            Mode(live, mic, micOpen, camMenuOpen, previewHidden),
            hudAlpha,
            onMicOpen = { micOpen = it },
            onCamMenuToggle = {
                camMenuOpen = !camMenuOpen
                if (camMenuOpen) camMenuTouches++
            },
            onCamMenuClose = { camMenuOpen = false },
            onCamMenuInteract = { camMenuTouches++ },
            onPreviewToggle = {
                Owlmic.post(AppMsg.Setting("camera.hidePreview", if (previewHidden) "off" else "on"))
            },
            onTouch = { touches++ },
            onStrip = onStrip,
        )
        AnimatedDrawer(seq, drawer, state)
    }
}

private data class Mode(
    val live: Boolean,
    val mic: Boolean,
    val micOpen: Boolean,
    val camMenuOpen: Boolean,
    val previewHidden: Boolean,
)

@Composable
private fun Content(
    up: Upright,
    fade: State<Float>,
    state: AppState,
    displayRatio: String,
    mode: Mode,
    hudAlpha: Float,
    onMicOpen: (Boolean) -> Unit,
    onCamMenuToggle: () -> Unit,
    onCamMenuClose: () -> Unit,
    onCamMenuInteract: () -> Unit,
    onPreviewToggle: () -> Unit,
    onTouch: () -> Unit,
    onStrip: (StripAction) -> Unit,
) {
    BoxWithConstraints(
        Modifier
            .fillMaxSize()
            .upright(up, swap = true)
            .graphicsLayer { alpha = fade.value }
            .onAnyTouch(onTouch),
    ) {
        val inset = if (up.sideways) Tokens.Phone.STRIP_SIDE_INSET_DP.dp else Tokens.Phone.STRIP_INSET_DP.dp
        val window = LocalWindowInfo.current.containerSize
        val short = with(LocalDensity.current) { minOf(window.width, window.height).toDp() }
        val capsule = minOf(short * Tokens.Phone.CAPSULE_WIDTH_FRACTION, maxWidth - 32.dp)

        // Watermark: owl on dot grid when idle or camera preview is hidden
        AnimatedVisibility(
            (!mode.live || mode.previewHidden) && !mode.mic,
            Modifier.align(Alignment.Center),
            enter = fadeIn(tween(Theme.STATE_MS)),
            exit = fadeOut(tween(Theme.STATE_MS)),
        ) {
            Image(
                painterResource(R.drawable.owl),
                null,
                Modifier.size(205.dp).graphicsLayer { alpha = 0.07f },
            )
        }

        // Status strip
        val strip = stripOf(state)
        AnimatedVisibility(
            strip != null,
            Modifier.align(Alignment.BottomCenter).padding(bottom = inset),
            enter = fadeIn(tween(Theme.STATE_MS)),
            exit = fadeOut(tween(Theme.STATE_MS)),
        ) {
            var last by remember { mutableStateOf(Strip.CHOOSE) }
            if (strip != null) last = strip
            StatusStrip(state, last, Modifier.width(capsule), onStrip)
        }

        // Waveform Dock (smooth animated flight between centered and docked)
        AnimatedVisibility(
            mode.mic,
            Modifier.fillMaxSize(),
            enter = fadeIn(tween(Theme.STATE_MS)),
            exit = fadeOut(tween(Theme.STATE_MS)),
        ) {
            WaveformDock(state, mode.live, mode.micOpen, maxWidth, onMicOpen)
        }

        // Camera HUD (Ratio menu + Preview toggle on left, Mirror & Flip on right)
        if (mode.live) {
            if (mode.camMenuOpen) {
                Box(
                    Modifier
                        .fillMaxSize()
                        .clickable(
                            interactionSource = remember { MutableInteractionSource() },
                            indication = null,
                        ) { onCamMenuClose() },
                )
            }
            val mirrored = (state.settings["camera.mirror"] ?: SettingsModel.find("camera.mirror")?.default) == "on"
            val maxMenuWidth = (maxWidth - 24.dp - Tokens.Phone.CAM_EXPAND_HEIGHT_DP.dp * 3 - 27.dp).coerceAtLeast(100.dp)
            Box(
                Modifier
                    .fillMaxWidth()
                    .align(Alignment.TopCenter)
                    .padding(12.dp)
                    .graphicsLayer { alpha = hudAlpha },
            ) {
                Row(
                    Modifier.align(Alignment.TopEnd),
                    horizontalArrangement = Arrangement.spacedBy(9.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    CamHudButton(Glyph.MIRROR, on = mirrored, label = stringResource(Copy.string.setting_camera_mirror)) {
                        onCamMenuClose()
                        Owlmic.post(AppMsg.Setting("camera.mirror", if (mirrored) "off" else "on"))
                    }
                    CamHudButton(Glyph.FLIP, on = false, label = stringResource(Copy.string.phone_flip_camera)) {
                        onCamMenuClose()
                        Owlmic.post(AppMsg.FlipLens)
                    }
                }
                Row(
                    Modifier.align(Alignment.TopStart),
                    horizontalArrangement = Arrangement.spacedBy(9.dp),
                    verticalAlignment = Alignment.Top,
                ) {
                    CameraFormatMenu(
                        ratio = displayRatio,
                        isOpen = mode.camMenuOpen,
                        sideways = up.sideways,
                        maxMenuWidth = maxMenuWidth,
                        onToggle = onCamMenuToggle,
                        onInteract = onCamMenuInteract,
                    )
                    CamHudButton(
                        if (mode.previewHidden) Glyph.EYE_OFF else Glyph.EYE,
                        on = mode.previewHidden,
                        label = stringResource(if (mode.previewHidden) Copy.string.camera_show_preview else Copy.string.camera_hide_preview),
                    ) {
                        onCamMenuClose()
                        onPreviewToggle()
                    }
                }
            }
        }
    }
}

/** The full-playground camera surface; optical crop and ambient blur animate inside GL without resizing the SurfaceView. */
@Composable
private fun PreviewFrame(ratio: String, sideways: Boolean) {
    val targetAr = previewTargetAspect(ratio, sideways)
    val borderPx = with(LocalDensity.current) { 1.dp.roundToPx().coerceAtLeast(1) }
    LaunchedEffect(targetAr, borderPx) {
        Owlmic.setPreviewAspect(targetAr, borderPx)
    }
    Preview()
}

/** Compact 2x3 expandable Ratio & Orientation HUD (1.5x mobile scale, 3x height when expanded). */
@Composable
private fun CameraFormatMenu(
    ratio: String,
    isOpen: Boolean,
    sideways: Boolean,
    maxMenuWidth: androidx.compose.ui.unit.Dp,
    onToggle: () -> Unit,
    onInteract: () -> Unit,
    modifier: Modifier = Modifier,
) {
    var menuWide by remember { mutableStateOf(sideways) }
    LaunchedEffect(ratio, sideways) {
        when (ratio) {
            "16:9", "4:3", "2:1" -> menuWide = true
            "9:16", "3:4", "1:2" -> menuWide = false
            "full" -> menuWide = sideways
        }
    }
    val isWide = when (ratio) {
        "16:9", "4:3", "2:1" -> true
        "9:16", "3:4", "1:2" -> false
        else -> menuWide
    }

    val rowH = Tokens.Phone.CAM_EXPAND_HEIGHT_DP.dp
    val openW = minOf(Tokens.Phone.CAM_EXPAND_OPEN_WIDTH_DP.dp, maxMenuWidth)
    val ease = CubicBezierEasing(0.16f, 1f, 0.3f, 1f)
    val width by animateDpAsState(if (isOpen) openW else Tokens.Phone.CAM_EXPAND_WIDTH_DP.dp, tween(240, easing = ease), label = "camMenuW")
    val height by animateDpAsState(if (isOpen) rowH * 3 else rowH, tween(240, easing = ease), label = "camMenuH")
    val iconRot by animateFloatAsState(if (isWide) 90f else 0f, tween(220, easing = ease), label = "camOrientRot")
    val shape = RoundedCornerShape(12.dp)
    val border = if (isOpen) Color(0x5CFFFFFF) else Color(0x38FFFFFF)
    val bg = if (isOpen) Color(0xF0000000) else Color(0xBF000000)

    val autoLabel = stringResource(Copy.string.value_ratio_auto)
    val closedLabel = if (ratio == "full" || ratio == "auto") autoLabel else ratio

    val orientClick = {
        onInteract()
        val flipped = toggleRatioOrientation(ratio)
        if (flipped != null) {
            Owlmic.post(AppMsg.Setting("camera.ratio", flipped))
        } else {
            menuWide = !menuWide
        }
    }

    Box(
        modifier
            .size(width, height)
            .clip(shape)
            .background(bg),
    ) {
        Crossfade(targetState = isOpen, animationSpec = tween(180), label = "camMenuContent") { open ->
            if (!open) {
                Box(
                    Modifier
                        .fillMaxSize()
                        .clickable(role = Role.Button, onClick = onToggle),
                    contentAlignment = Alignment.Center,
                ) {
                    BasicText(
                        closedLabel,
                        style = Theme.pageDetail.copy(
                            color = Color.White,
                            fontSize = 15.sp,
                            fontWeight = FontWeight.Medium,
                            fontFeatureSettings = "tnum",
                        ),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            } else {
                val row2 = if (isWide) listOf("16:9" to "16:9", "4:3" to "4:3") else listOf("9:16" to "9:16", "3:4" to "3:4")
                val row3 = if (isWide) listOf("2:1" to "2:1", "1:1" to "1:1") else listOf("1:2" to "1:2", "1:1" to "1:1")
                Column(Modifier.fillMaxSize()) {
                    Row(Modifier.fillMaxWidth().weight(1f)) {
                        val autoActive = ratio == "full" || ratio == "auto"
                        RatioOptionCell(
                            label = autoLabel,
                            active = autoActive,
                            modifier = Modifier.weight(1f).fillMaxHeight(),
                            onClick = {
                                onInteract()
                                if (autoActive) onToggle() else Owlmic.post(AppMsg.Setting("camera.ratio", "full"))
                            },
                        )
                        Box(Modifier.width(1.dp).fillMaxHeight().background(Color(0x1AFFFFFF)))
                        Box(
                            Modifier
                                .weight(1f)
                                .fillMaxHeight()
                                .clickable(
                                    role = Role.Button,
                                    onClickLabel = stringResource(if (isWide) Copy.string.value_landscape else Copy.string.value_portrait),
                                    onClick = orientClick,
                                ),
                            contentAlignment = Alignment.Center,
                        ) {
                            Icon(
                                Glyph.PHONE,
                                Color(0xD9FFFFFF),
                                null,
                                Modifier.graphicsLayer { rotationZ = iconRot },
                                size = 18.dp,
                                stroke = 2.15f,
                            )
                        }
                    }
                    listOf(row2, row3).forEach { rowItems ->
                        Box(Modifier.fillMaxWidth().height(1.dp).background(Color(0x1AFFFFFF)))
                        Row(Modifier.fillMaxWidth().weight(1f)) {
                            rowItems.forEachIndexed { idx, (valKey, label) ->
                                if (idx > 0) Box(Modifier.width(1.dp).fillMaxHeight().background(Color(0x1AFFFFFF)))
                                val active = ratio == valKey
                                RatioOptionCell(
                                    label = label,
                                    active = active,
                                    modifier = Modifier.weight(1f).fillMaxHeight(),
                                    onClick = {
                                        onInteract()
                                        if (active) onToggle() else Owlmic.post(AppMsg.Setting("camera.ratio", valKey))
                                    },
                                )
                            }
                        }
                    }
                }
            }
        }
        Box(Modifier.matchParentSize().border(1.dp, border, shape))
    }
}

@Composable
private fun RatioOptionCell(label: String, active: Boolean, modifier: Modifier, onClick: () -> Unit) {
    Box(
        modifier
            .background(if (active) Color.White else Color.Transparent)
            .clickable(role = Role.Button, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        BasicText(
            label,
            style = Theme.pageDetail.copy(
                color = if (active) Color.Black else Color(0xA6FFFFFF),
                fontSize = 15.sp,
                fontWeight = FontWeight.Medium,
                fontFeatureSettings = "tnum",
            ),
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

/** Mirror and Flip buttons in Camera HUD (1.5x mobile scale). */
@Composable
private fun CamHudButton(glyph: Glyph, on: Boolean = false, label: String, onClick: () -> Unit) {
    val shape = RoundedCornerShape(12.dp)
    val bg by animateColorAsState(if (on) Color.White else Color(0xBF000000), tween(Theme.STATE_MS), label = "camBtnBg")
    val border by animateColorAsState(if (on) Color.White else Color(0x38FFFFFF), tween(Theme.STATE_MS), label = "camBtnBorder")
    val fg by animateColorAsState(if (on) Color.Black else Color.White, tween(Theme.STATE_MS), label = "camBtnFg")
    Box(
        Modifier
            .size(Tokens.Phone.CAM_EXPAND_HEIGHT_DP.dp)
            .clip(shape)
            .background(bg)
            .border(1.dp, border, shape)
            .clickable(role = Role.Button, onClickLabel = label, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(glyph, fg, label, size = 22.dp, stroke = 2.15f)
    }
}

/** Unified waveform dock with smooth animated morphing between centered and docked states. */
@Composable
private fun WaveformDock(
    state: AppState,
    live: Boolean,
    micOpen: Boolean,
    maxWidth: androidx.compose.ui.unit.Dp,
    onMicOpen: (Boolean) -> Unit,
) {
    val ease = CubicBezierEasing(0.16f, 1f, 0.3f, 1f)
    val dockProgress by animateFloatAsState(
        targetValue = if (live) 1f else 0f,
        animationSpec = tween(360, easing = ease),
        label = "dockProgress",
    )

    val targetBias = when {
        live -> 1f
        micOpen -> -0.38f
        else -> 0f
    }
    val animatedBias by animateFloatAsState(
        targetValue = targetBias,
        animationSpec = tween(if (live) 360 else 280, easing = ease),
        label = "verticalBias",
    )

    val targetWidth = if (live) {
        if (micOpen) maxWidth - 24.dp else 236.dp
    } else {
        minOf(288.dp, maxWidth - 24.dp)
    }
    val animatedWidth by animateDpAsState(
        targetValue = targetWidth,
        animationSpec = tween(340, easing = ease),
        label = "dockWidth",
    )

    val animatedBg by animateColorAsState(
        targetValue = if (live) Color.Black else Color.Transparent,
        animationSpec = tween(240, easing = ease),
        label = "dockBg",
    )

    val animatedBorder by animateColorAsState(
        targetValue = if (live) {
            if (micOpen) Color(0x52FFFFFF) else Color(0x38FFFFFF)
        } else Color.Transparent,
        animationSpec = tween(240, easing = ease),
        label = "dockBorder",
    )

    val waveHeight by animateDpAsState(
        targetValue = if (live) 24.dp else 84.dp,
        animationSpec = tween(340, easing = ease),
        label = "waveHeight",
    )

    val headerHeight by animateDpAsState(
        targetValue = if (live) 44.dp else 84.dp,
        animationSpec = tween(340, easing = ease),
        label = "headerHeight",
    )

    val waveWidth by animateDpAsState(
        targetValue = if (live) 208.dp else minOf(288.dp, maxWidth - 32.dp),
        animationSpec = tween(340, easing = ease),
        label = "waveWidth",
    )

    val bottomPadding = (12 * dockProgress).dp
    val shape = RoundedCornerShape(12.dp)

    Box(Modifier.fillMaxSize()) {
        if (micOpen) {
            Box(
                Modifier
                    .fillMaxSize()
                    .clickable(
                        interactionSource = remember { MutableInteractionSource() },
                        indication = null,
                    ) { onMicOpen(false) },
            )
        }

        // Unified animated capsule container
        Column(
            Modifier
                .align(BiasAlignment(0f, animatedBias))
                .padding(bottom = bottomPadding)
                .width(animatedWidth)
                .clip(shape)
                .background(animatedBg)
                .border(1.dp, animatedBorder, shape),
        ) {
            AnimatedVisibility(
                visible = live && micOpen,
                enter = expandVertically(tween(340, easing = ease)) + fadeIn(tween(200)),
                exit = shrinkVertically(tween(340, easing = ease)) + fadeOut(tween(200)),
            ) {
                Column(Modifier.fillMaxWidth()) {
                    Column(
                        Modifier
                            .fillMaxWidth()
                            .verticalScroll(rememberScrollState())
                            .padding(horizontal = 16.dp, vertical = 14.dp),
                        verticalArrangement = Arrangement.spacedBy(14.dp),
                    ) {
                        MicControls(state)
                    }
                    Box(Modifier.fillMaxWidth().height(1.dp).background(Color(0x26FFFFFF)))
                }
            }

            Box(
                Modifier
                    .fillMaxWidth()
                    .height(headerHeight)
                    .clickable(
                        interactionSource = remember { MutableInteractionSource() },
                        indication = null,
                        role = Role.Button,
                        onClick = { onMicOpen(!micOpen) },
                    ),
                contentAlignment = Alignment.Center,
            ) {
                Waveform(
                    modifier = Modifier.width(waveWidth).height(waveHeight),
                    dockProgress = dockProgress,
                )
            }
        }

        // Mic-only bottom quick panel
        AnimatedVisibility(
            visible = !live && micOpen,
            modifier = Modifier.align(Alignment.BottomCenter),
            enter = slideInVertically(tween(Theme.STATE_MS)) { it } + fadeIn(),
            exit = slideOutVertically(tween(Theme.STATE_MS)) { it } + fadeOut(),
        ) {
            Column(
                Modifier
                    .fillMaxWidth()
                    .background(Color.Black),
            ) {
                Box(Modifier.fillMaxWidth().height(1.dp).background(Color(0x26FFFFFF)))
                Column(
                    Modifier
                        .fillMaxWidth()
                        .verticalScroll(rememberScrollState())
                        .padding(horizontal = 16.dp, vertical = 16.dp),
                    verticalArrangement = Arrangement.spacedBy(14.dp),
                ) {
                    MicControls(state)
                }
            }
        }
    }
}

@Composable
private fun StatusStrip(state: AppState, strip: Strip, modifier: Modifier, onStrip: (StripAction) -> Unit) {
    val action = stripAction(strip)
    val tap = if (action != null) Modifier.clickable(role = Role.Button) { onStrip(action) } else Modifier
    val shape = RoundedCornerShape(Theme.radius)
    Column(
        modifier
            .clip(shape)
            .background(Theme.bg)
            .border(1.dp, Theme.stripEdge, shape)
            .then(tap)
            .padding(horizontal = 18.dp, vertical = 12.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        BasicText(stripText(state, strip), style = stripStyle(), maxLines = 3)
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
