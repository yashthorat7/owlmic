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
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
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
    LaunchedEffect(live, mic) {
        micOpen = false
        camMenuOpen = false
    }
    BackHandler(micOpen || camMenuOpen) {
        if (camMenuOpen) camMenuOpen = false else micOpen = false
    }

    var awake by remember { mutableStateOf(false) }
    var touches by remember { mutableIntStateOf(0) }
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
        if (!live) DotGrid(Modifier.fillMaxSize())
        if (live) PreviewFrame(state)
        if (live) Frame(Modifier.fillMaxSize())
        Content(
            seq.layout,
            seq.fade,
            state,
            Mode(live, mic, micOpen, camMenuOpen),
            hudAlpha,
            onMicOpen = { micOpen = it },
            onCamMenuToggle = { camMenuOpen = !camMenuOpen },
            onTouch = { touches++ },
            onStrip = onStrip,
        )
        AnimatedDrawer(seq, drawer, state)
    }
}

private data class Mode(val live: Boolean, val mic: Boolean, val micOpen: Boolean, val camMenuOpen: Boolean)

@Composable
private fun Content(
    up: Upright,
    fade: State<Float>,
    state: AppState,
    mode: Mode,
    hudAlpha: Float,
    onMicOpen: (Boolean) -> Unit,
    onCamMenuToggle: () -> Unit,
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

        // Watermark: owl on dot grid when idle
        AnimatedVisibility(
            !mode.live && !mode.mic,
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

        // Waveform Dock
        if (mode.mic) {
            if (mode.live) {
                // Docked capsule at bottom when camera is on
                DockedWave(state, mode.micOpen, maxWidth, onMicOpen)
            } else {
                // Centered wave when only mic is on
                CenteredWave(state, mode.micOpen, onMicOpen)
            }
        }

        // Camera HUD (Format Menu on left, Mirror & Flip on right)
        if (mode.live) {
            val mirrored = (state.settings["camera.mirrorPreview"] ?: SettingsModel.find("camera.mirrorPreview")?.default) == "on"
            Row(
                Modifier
                    .fillMaxWidth()
                    .align(Alignment.TopCenter)
                    .padding(12.dp)
                    .graphicsLayer { alpha = hudAlpha },
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.Top,
            ) {
                CameraFormatMenu(state, mode.camMenuOpen, onCamMenuToggle)
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                    CamHudButton(Glyph.MIRROR, on = mirrored, label = stringResource(Copy.string.setting_camera_mirror_preview)) {
                        Owlmic.post(AppMsg.Setting("camera.mirrorPreview", if (mirrored) "off" else "on"))
                    }
                    CamHudButton(Glyph.FLIP, on = false, label = stringResource(Copy.string.phone_flip_camera)) {
                        Owlmic.post(AppMsg.FlipLens)
                    }
                }
            }
        }
    }
}

/** The camera preview surface with optical crop border lines matching target aspect ratio. */
@Composable
private fun PreviewFrame(state: AppState) {
    val ratio = state.settings["camera.ratio"] ?: "full"
    val targetAr = when (ratio) {
        "16:9" -> 16f / 9f
        "4:3" -> 4f / 3f
        "1:1" -> 1f
        "3:4" -> 3f / 4f
        "9:16" -> 9f / 16f
        else -> null
    }

    BoxWithConstraints(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        val cw = maxWidth.value
        val ch = maxHeight.value
        val containerAr = if (ch > 0f) cw / ch else 1f

        val (cropMode, frameW, frameH) = if (targetAr != null && kotlin.math.abs(containerAr - targetAr) >= 0.01f) {
            if (containerAr > targetAr) {
                Triple("pillar", (ch * targetAr).dp, maxHeight)
            } else {
                Triple("letter", maxWidth, (cw / targetAr).dp)
            }
        } else {
            Triple("none", maxWidth, maxHeight)
        }

        Box(
            Modifier
                .size(frameW, frameH)
                .drawBehind {
                    val lineColor = Color(0x38FFFFFF)
                    if (cropMode == "pillar") {
                        drawLine(lineColor, Offset(0f, 0f), Offset(0f, size.height), 1.dp.toPx())
                        drawLine(lineColor, Offset(size.width, 0f), Offset(size.width, size.height), 1.dp.toPx())
                    } else if (cropMode == "letter") {
                        drawLine(lineColor, Offset(0f, 0f), Offset(size.width, 0f), 1.dp.toPx())
                        drawLine(lineColor, Offset(0f, size.height), Offset(size.width, size.height), 1.dp.toPx())
                    }
                },
        ) {
            Preview()
        }
    }
}

/** Compact expandable format HUD: Resolution, Frame Rate, Aspect Ratio. */
@Composable
private fun CameraFormatMenu(state: AppState, isOpen: Boolean, onToggle: () -> Unit) {
    val quality = state.settings["camera.quality"] ?: "1080p"
    val fps = state.settings["camera.frameRate"] ?: "30"
    val ratio = state.settings["camera.ratio"] ?: "full"

    val resLabel = when (quality) {
        "720p", "HD" -> "HD"
        "480p" -> "480p"
        else -> "FHD"
    }

    val ease = CubicBezierEasing(0.16f, 1f, 0.3f, 1f)
    val width by animateDpAsState(if (isOpen) 180.dp else 50.dp, tween(240, easing = ease), label = "camMenuW")
    val height by animateDpAsState(if (isOpen) 119.dp else 29.dp, tween(240, easing = ease), label = "camMenuH")
    val shape = RoundedCornerShape(8.dp)
    val border = if (isOpen) Color(0x5CFFFFFF) else Color(0x38FFFFFF)
    val bg = if (isOpen) Color(0xFA000000) else Color(0xE0000000)

    Box(
        Modifier
            .size(width, height)
            .clip(shape)
            .background(bg)
            .border(1.dp, border, shape),
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
                        resLabel,
                        style = Theme.pageDetail.copy(color = Color.White, fontSize = 11.sp, fontWeight = FontWeight.Medium),
                    )
                }
            } else {
                Column(Modifier.fillMaxSize()) {
                // Row 1: Resolution (FHD, HD, 480p)
                Row(Modifier.fillMaxWidth().height(29.dp)) {
                    val resOptions = listOf("1080p" to "FHD", "720p" to "HD", "480p" to "480p")
                    resOptions.forEachIndexed { idx, (valKey, label) ->
                        if (idx > 0) Box(Modifier.width(1.dp).fillMaxHeight().background(Color(0x1AFFFFFF)))
                        val active = quality == valKey || (valKey == "1080p" && quality == "auto")
                        Box(
                            Modifier
                                .weight(1f)
                                .fillMaxHeight()
                                .background(if (active) Color.White else Color.Transparent)
                                .clickable(role = Role.Button) {
                                    if (active) onToggle() else Owlmic.post(AppMsg.Setting("camera.quality", valKey))
                                },
                            contentAlignment = Alignment.Center,
                        ) {
                            BasicText(
                                label,
                                style = Theme.pageDetail.copy(
                                    color = if (active) Color.Black else Color(0xA6FFFFFF),
                                    fontSize = 11.sp,
                                    fontWeight = FontWeight.Medium,
                                ),
                            )
                        }
                    }
                }
                Box(Modifier.fillMaxWidth().height(1.dp).background(Color(0x1AFFFFFF)))
                // Row 2: FPS (60fps, 30fps, 24fps)
                Row(Modifier.fillMaxWidth().height(29.dp)) {
                    val fpsOptions = listOf("60" to "60fps", "30" to "30fps", "24" to "24fps")
                    fpsOptions.forEachIndexed { idx, (valKey, label) ->
                        if (idx > 0) Box(Modifier.width(1.dp).fillMaxHeight().background(Color(0x1AFFFFFF)))
                        val active = fps == valKey
                        Box(
                            Modifier
                                .weight(1f)
                                .fillMaxHeight()
                                .background(if (active) Color.White else Color.Transparent)
                                .clickable(role = Role.Button) {
                                    Owlmic.post(AppMsg.Setting("camera.frameRate", valKey))
                                },
                            contentAlignment = Alignment.Center,
                        ) {
                            BasicText(
                                label,
                                style = Theme.pageDetail.copy(
                                    color = if (active) Color.Black else Color(0xA6FFFFFF),
                                    fontSize = 11.sp,
                                    fontWeight = FontWeight.Medium,
                                ),
                            )
                        }
                    }
                }
                Box(Modifier.fillMaxWidth().height(1.dp).background(Color(0x1AFFFFFF)))
                // Row 3: Ratio (Auto, 16:9, 4:3)
                Row(Modifier.fillMaxWidth().height(29.dp)) {
                    val ratioOptions1 = listOf("full" to "Auto", "16:9" to "16:9", "4:3" to "4:3")
                    ratioOptions1.forEachIndexed { idx, (valKey, label) ->
                        if (idx > 0) Box(Modifier.width(1.dp).fillMaxHeight().background(Color(0x1AFFFFFF)))
                        val active = ratio == valKey || (valKey == "full" && ratio == "auto")
                        Box(
                            Modifier
                                .weight(1f)
                                .fillMaxHeight()
                                .background(if (active) Color.White else Color.Transparent)
                                .clickable(role = Role.Button) {
                                    Owlmic.post(AppMsg.Setting("camera.ratio", valKey))
                                },
                            contentAlignment = Alignment.Center,
                        ) {
                            BasicText(
                                label,
                                style = Theme.pageDetail.copy(
                                    color = if (active) Color.Black else Color(0xA6FFFFFF),
                                    fontSize = 11.sp,
                                    fontWeight = FontWeight.Medium,
                                ),
                            )
                        }
                    }
                }
                Box(Modifier.fillMaxWidth().height(1.dp).background(Color(0x1AFFFFFF)))
                // Row 4: Ratio (1:1, 3:4, 9:16)
                Row(Modifier.fillMaxWidth().height(29.dp)) {
                    val ratioOptions2 = listOf("1:1" to "1:1", "3:4" to "3:4", "9:16" to "9:16")
                    ratioOptions2.forEachIndexed { idx, (valKey, label) ->
                        if (idx > 0) Box(Modifier.width(1.dp).fillMaxHeight().background(Color(0x1AFFFFFF)))
                        val active = ratio == valKey
                        Box(
                            Modifier
                                .weight(1f)
                                .fillMaxHeight()
                                .background(if (active) Color.White else Color.Transparent)
                                .clickable(role = Role.Button) {
                                    Owlmic.post(AppMsg.Setting("camera.ratio", valKey))
                                },
                            contentAlignment = Alignment.Center,
                        ) {
                            BasicText(
                                label,
                                style = Theme.pageDetail.copy(
                                    color = if (active) Color.Black else Color(0xA6FFFFFF),
                                    fontSize = 11.sp,
                                    fontWeight = FontWeight.Medium,
                                ),
                            )
                        }
                    }
                }
            }
        }
    }
}
}

/** Mirror and Flip buttons in Camera HUD. */
@Composable
private fun CamHudButton(glyph: Glyph, on: Boolean = false, label: String, onClick: () -> Unit) {
    val shape = RoundedCornerShape(8.dp)
    val bg by animateColorAsState(if (on) Color.White else Color(0xCC000000), tween(Theme.STATE_MS), label = "camBtnBg")
    val border by animateColorAsState(if (on) Color.White else Color(0x33FFFFFF), tween(Theme.STATE_MS), label = "camBtnBorder")
    val fg by animateColorAsState(if (on) Color.Black else Color.White, tween(Theme.STATE_MS), label = "camBtnFg")
    Box(
        Modifier
            .size(29.dp)
            .clip(shape)
            .background(bg)
            .border(1.dp, border, shape)
            .clickable(role = Role.Button, onClickLabel = label, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(glyph, fg, label, size = 15.dp, stroke = 2f)
    }
}

/** Docked waveform capsule at the bottom when camera is on. */
@Composable
private fun DockedWave(state: AppState, open: Boolean, maxWidth: androidx.compose.ui.unit.Dp, onOpen: (Boolean) -> Unit) {
    val width by animateDpAsState(if (open) maxWidth - 24.dp else 236.dp, tween(Theme.STATE_MS), label = "dockWidth")
    val shape = RoundedCornerShape(12.dp)
    val borderColor = if (open) Color(0x52FFFFFF) else Color(0x38FFFFFF)

    Box(
        Modifier
            .fillMaxSize()
            .padding(bottom = 12.dp),
        contentAlignment = Alignment.BottomCenter,
    ) {
        Column(
            Modifier
                .width(width)
                .clip(shape)
                .background(Color.Black)
                .border(1.dp, borderColor, shape),
        ) {
            AnimatedVisibility(
                open,
                enter = expandVertically(tween(Theme.STATE_MS)) + fadeIn(),
                exit = shrinkVertically(tween(Theme.STATE_MS)) + fadeOut(),
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
                    .height(44.dp)
                    .clickable(role = Role.Button, onClick = { onOpen(!open) }),
                contentAlignment = Alignment.Center,
            ) {
                Waveform(Modifier.width(208.dp).height(24.dp), isDocked = true)
            }
        }
    }
}

/** Centered waveform when camera is off, with bottom quick panel on tap. */
@Composable
private fun CenteredWave(state: AppState, open: Boolean, onOpen: (Boolean) -> Unit) {
    val offsetY by animateDpAsState(if (open) (-90).dp else 0.dp, tween(Theme.STATE_MS), label = "waveLift")
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Box(
            Modifier
                .offset { IntOffset(0, offsetY.roundToPx()) }
                .width(288.dp)
                .height(84.dp)
                .clickable(
                    interactionSource = remember { MutableInteractionSource() },
                    indication = null,
                    role = Role.Button,
                    onClick = { onOpen(!open) },
                ),
            contentAlignment = Alignment.Center,
        ) {
            Waveform(Modifier.fillMaxSize(), isDocked = false)
        }

        // Mic-only bottom quick panel
        AnimatedVisibility(
            open,
            Modifier.align(Alignment.BottomCenter),
            enter = slideInVertically(tween(Theme.STATE_MS)) { it } + fadeIn(),
            exit = slideOutVertically(tween(Theme.STATE_MS)) { it } + fadeOut(),
        ) {
            Column(
                Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(topStart = 12.dp, topEnd = 12.dp))
                    .background(Color.Black)
                    .border(1.dp, Color(0x26FFFFFF), RoundedCornerShape(topStart = 12.dp, topEnd = 12.dp))
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 16.dp, vertical = 16.dp),
                verticalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                MicControls(state)
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
