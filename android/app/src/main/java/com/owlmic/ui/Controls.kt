package com.owlmic.ui

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.setProgress
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.semantics.toggleableState
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.owlmic.Owlmic
import com.owlmic.core.design.SettingsModel
import com.owlmic.core.design.Tokens
import com.owlmic.core.hub.AppState
import com.owlmic.hub.AppMsg
import kotlin.math.roundToInt
import com.owlmic.core.R as Copy

/*
 * The controls over the preview (the mic's settings in the capsule's drawer, the round camera buttons) and in settings.
 * One look everywhere: the grid's ink on black, no colour, no text on buttons; what is on carries a tick or turns light.
 */

/** Hears every touch inside without taking it, so a tap anywhere on the preview wakes the controls and buttons still work. */
fun Modifier.onAnyTouch(onTouch: () -> Unit): Modifier = pointerInput(Unit) {
    awaitPointerEventScope {
        while (true) {
            awaitPointerEvent(PointerEventPass.Initial)
            onTouch()
        }
    }
}

/** The mic's settings: what the capsule's drawer and the mic-only drawer show. */
@Composable
fun ColumnScope.MicControls(state: AppState) {
    SettingToggle(state, "mic.isolation", Copy.string.setting_mic_isolation)
    val isolationOn = (state.settings["mic.isolation"] ?: SettingsModel.find("mic.isolation")?.default) == "on"
    SettingToggle(state, "mic.autoLevel", Copy.string.setting_mic_auto_level, enabled = isolationOn)
    SettingSlider(state, "mic.noiseReduction", Copy.string.setting_mic_noise_reduction)
    SettingSlider(state, "mic.volume", Copy.string.setting_mic_volume, fromMiddle = true)
}

/** How a slider's step reads: noise reduction's 0 is Off; the volume is in dB with its sign. */
@Composable
fun stepText(id: String, value: String): String = when {
    id == "mic.volume" -> stringResource(Copy.string.value_db, if (value.startsWith("-") || value == "0") value else "+$value")
    value == "0" -> stringResource(Copy.string.value_off)
    else -> value
}

/**
 * A hardware fader on a line: tap or drag. The first step of noise reduction is off; [fromMiddle] fills from the
 * centre (the volume's 0 dB) instead of from the start. A drag sends only where it ends, so the PC isn't flooded.
 */
@Composable
fun SettingSlider(state: AppState, id: String, label: Int, fromMiddle: Boolean = false) {
    val def = SettingsModel.find(id) ?: return
    val current = state.settings[id] ?: def.default
    val steps = def.values.size
    var dragged by remember { mutableStateOf<Int?>(null) }
    val isDragging = dragged != null
    val at = dragged ?: def.values.indexOf(current).coerceAtLeast(0)
    val commit = { i: Int -> if (def.values[i] != current) Owlmic.post(AppMsg.Setting(id, def.values[i])) }
    val name = stringResource(label)
    val value = stepText(id, def.values[at])

    val trackHeight by animateDpAsState(if (isDragging) 10.dp else 8.dp, tween(Theme.STATE_MS), label = "trackHeight")
    val capHeight by animateDpAsState(if (isDragging) 16.dp else 14.dp, tween(Theme.STATE_MS), label = "capHeight")

    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            BasicText(name, Modifier.weight(1f), style = Theme.pageText)
            BasicText(value, style = Theme.pageText.copy(color = Theme.ink, fontFeatureSettings = "tnum"))
        }
        Canvas(
            Modifier
                .fillMaxWidth()
                .height(24.dp)
                .semantics {
                    contentDescription = name
                    stateDescription = value
                    progressBarRangeInfo = ProgressBarRangeInfo(at.toFloat(), 0f..(steps - 1).toFloat(), steps - 2)
                    setProgress { target ->
                        commit(target.roundToInt().coerceIn(0, steps - 1))
                        true
                    }
                }
                .pointerInput(steps, current) {
                    val step = { x: Float -> ((x / size.width) * (steps - 1)).roundToInt().coerceIn(0, steps - 1) }
                    detectTapGestures { commit(step(it.x)) }
                }
                .pointerInput(steps, current) {
                    val step = { x: Float -> ((x / size.width) * (steps - 1)).roundToInt().coerceIn(0, steps - 1) }
                    detectHorizontalDragGestures(
                        onDragStart = { dragged = step(it.x) },
                        onDragEnd = {
                            dragged?.let(commit)
                            dragged = null
                        },
                        onDragCancel = { dragged = null },
                    ) { change, _ ->
                        change.consume()
                        dragged = step(change.position.x)
                    }
                },
        ) {
            val w = size.width
            val cy = size.height / 2f
            val th = trackHeight.toPx()
            val tr = th / 2f

            // Track background: rgba(255, 255, 255, 0.14)
            val trackColor = Color(0x24FFFFFF)
            drawRoundRect(
                color = trackColor,
                topLeft = Offset(0f, cy - tr),
                size = Size(w, th),
                cornerRadius = CornerRadius(tr, tr),
            )

            // Fill
            val x = { i: Int -> (w * i / (steps - 1).coerceAtLeast(1)) }
            val from = if (fromMiddle) x((steps - 1) / 2) else 0f
            val to = x(at)
            val fillLeft = minOf(from, to)
            val fillWidth = kotlin.math.abs(to - from)
            if (fillWidth > 0f) {
                drawRoundRect(
                    color = Color.White,
                    topLeft = Offset(fillLeft, cy - tr),
                    size = Size(fillWidth, th),
                    cornerRadius = CornerRadius(tr, tr),
                )
            }

            // Cap (hardware fader): width 2.5dp, height capHeight, rounded 2dp, white with black shadow/outline
            val cw = 2.5.dp.toPx()
            val ch = capHeight.toPx()
            val capX = to.coerceIn(cw / 2f, w - cw / 2f)
            val capLeft = capX - cw / 2f
            val capTop = cy - ch / 2f

            // Black outline / shadow around cap (1.5px)
            drawRoundRect(
                color = Color.Black,
                topLeft = Offset(capLeft - 1.5.dp.toPx(), capTop - 1.5.dp.toPx()),
                size = Size(cw + 3.dp.toPx(), ch + 3.dp.toPx()),
                cornerRadius = CornerRadius(3.5.dp.toPx(), 3.5.dp.toPx()),
            )
            // White cap
            drawRoundRect(
                color = Color.White,
                topLeft = Offset(capLeft, capTop),
                size = Size(cw, ch),
                cornerRadius = CornerRadius(2.dp.toPx(), 2.dp.toPx()),
            )
        }
    }
}

/** An on/off setting as a row with a switch. */
@Composable
fun SettingToggle(state: AppState, id: String, label: Int, enabled: Boolean = true) {
    val def = SettingsModel.find(id) ?: return
    val on = (state.settings[id] ?: def.default) == "on"
    val clickableModifier = if (enabled) {
        Modifier
            .semantics { toggleableState = ToggleableState(on) }
            .clickable(role = Role.Switch) { Owlmic.post(AppMsg.Setting(id, if (on) "off" else "on")) }
    } else {
        Modifier
    }
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = Theme.touch)
            .graphicsLayer { alpha = if (enabled) 1f else 0.36f }
            .then(clickableModifier),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        BasicText(stringResource(label), Modifier.weight(1f), style = Theme.pageText)
        Switch(on)
    }
}

/** On: white pill and black thumb at the end. Off: black pill and dim white thumb at start. */
@Composable
fun Switch(on: Boolean) {
    val at by animateDpAsState(if (on) 18.dp else 0.dp, tween(Theme.STATE_MS), label = "switch")
    val trackBg by animateColorAsState(if (on) Color.White else Theme.bg, tween(Theme.STATE_MS), label = "switchTrackBg")
    val trackBorder by animateColorAsState(if (on) Color.White else Theme.chipOnEdge, tween(Theme.STATE_MS), label = "switchTrackBorder")
    val thumbBg by animateColorAsState(if (on) Theme.onContent else Theme.text2, tween(Theme.STATE_MS), label = "switchThumbBg")
    Box(
        Modifier
            .size(38.dp, 20.dp)
            .clip(CircleShape)
            .background(trackBg)
            .border(1.dp, trackBorder, CircleShape),
    ) {
        Box(
            Modifier
                .offset { IntOffset((2.dp + at).roundToPx(), 2.dp.roundToPx()) }
                .size(14.dp)
                .clip(CircleShape)
                .background(thumbBg),
        )
    }
}

/** A square button with its [label] under the content; [on] adds a tick in the corner. */
@Composable
fun SquareButton(
    label: String,
    modifier: Modifier,
    on: Boolean = false,
    description: String = label,
    onClick: () -> Unit,
    content: @Composable BoxScope.() -> Unit,
) {
    val shape = RoundedCornerShape(12.dp)
    val onText = stringResource(Copy.string.value_on)
    Box(
        modifier
            .clip(shape)
            .background(Theme.chip)
            .border(1.dp, if (on) Theme.chipOnEdge else Theme.chipOffEdge, shape)
            .semantics(mergeDescendants = true) {
                contentDescription = description
                if (on) stateDescription = onText
            }
            .clickable(role = Role.Button, onClick = onClick),
    ) {
        Column(Modifier.align(Alignment.Center).padding(horizontal = 4.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Box(Modifier.height(28.dp), contentAlignment = Alignment.Center, content = content)
            BasicText(label, Modifier.padding(top = 8.dp), style = Theme.pageDetail.copy(textAlign = TextAlign.Center), maxLines = 1)
        }
        if (on) Icon(Glyph.CHECK, Theme.ink, null, Modifier.align(Alignment.TopEnd).padding(6.dp), size = 14.dp, stroke = 2.6f)
    }
}
