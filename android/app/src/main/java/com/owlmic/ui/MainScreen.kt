package com.owlmic.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.Crossfade
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.union
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import com.owlmic.Owlmic
import com.owlmic.R
import com.owlmic.core.design.Tokens
import com.owlmic.core.hub.AppState
import com.owlmic.core.hub.Connection
import com.owlmic.core.hub.Feature
import com.owlmic.core.hub.LinkKind
import com.owlmic.hub.AppMsg
import com.owlmic.core.R as Copy

/**
 * The one screen, as drawn in the prototype (website/proto-templates): the wordmark with the help and link tiles on top,
 * the preview area in the middle, and the camera, mic and speaker tiles below, in a bento grid of rounded tiles.
 * [turn] is how the phone is held (see Turn.kt): the grid never moves; a change fades the preview's content out, turns
 * the icons, and fades it back in. [onLayoutTurn] hears when the content changes, for the status bar. [onFeature] and
 * [onTryBluetooth] go through the activity, which asks for permissions first; [onTethering] opens the system's tethering
 * settings. Settings open by long-pressing a feature tile.
 */
@Composable
fun MainScreen(turn: Int, onLayoutTurn: (Int) -> Unit, onFeature: (Feature) -> Unit, onTryBluetooth: () -> Unit, onTethering: () -> Unit) {
    val seq = remember { TurnSequence(turn) }
    val layoutChanged by rememberUpdatedState(onLayoutTurn)
    LaunchedEffect(turn) { seq.follow(turn) { layoutChanged(it) } }
    Screen(seq, onFeature, onTryBluetooth, onTethering)
}

@Composable
private fun Screen(seq: TurnSequence, onFeature: (Feature) -> Unit, onTryBluetooth: () -> Unit, onTethering: () -> Unit) {
    val state by Owlmic.state.collectAsState()
    var sheet by remember { mutableStateOf<String?>(null) }
    var drawer by remember { mutableStateOf<Drawer?>(null) }
    val toggle = { d: Drawer -> drawer = if (drawer == d) null else d }

    // A black band at the bottom either way, and at the top too sideways, where the status bar is hidden.
    val band = Tokens.Phone.SIDE_BAND_DP.dp
    val sideways = seq.layout.sideways
    val insets = WindowInsets.safeDrawing.union(WindowInsets(top = if (sideways) band else 0.dp, bottom = band))
    val fade by seq.fade
    Box(Modifier.fillMaxSize().background(Theme.bg)) {
        Column(
            Modifier.fillMaxSize().windowInsetsPadding(insets).background(Theme.seam).padding(Theme.gap),
            verticalArrangement = Arrangement.spacedBy(Theme.gap),
        ) {
            TopBar(seq, state.connection, drawer, onHelp = { toggle(Drawer.HELP) }, onLink = { toggle(Drawer.CONNECTION) })
            Playground(
                seq,
                state,
                drawer,
                Modifier.weight(1f).fillMaxWidth(),
                onStrip = { action ->
                    when (action) {
                        StripAction.CHOOSE -> drawer = Drawer.CONNECTION
                        StripAction.ASK_AGAIN -> Owlmic.post(AppMsg.AskAgain)
                        StripAction.TETHERING -> onTethering()
                        StripAction.BLUETOOTH -> onTryBluetooth()
                    }
                },
            )
            Row(Modifier.fillMaxWidth().height(Tokens.Phone.BOTTOM_BAR_DP.dp), horizontalArrangement = Arrangement.spacedBy(Theme.gap)) {
                FeatureTile(seq.icons, state, Feature.CAMERA, Modifier.weight(1f), onFeature) { sheet = SECTION_CAMERA }
                FeatureTile(seq.icons, state, Feature.MIC, Modifier.weight(1f), onFeature) { sheet = SECTION_MIC }
                FeatureTile(seq.icons, state, Feature.SPEAKER, Modifier.weight(1f), onFeature) { sheet = SECTION_SPEAKER }
            }
        }
        AnimatedVisibility(sheet != null, enter = fadeIn(tween(Theme.STATE_MS)), exit = fadeOut(tween(Theme.STATE_MS))) {
            // The insets (the keyboard's too) stay outside the turn: they belong to the screen, not to the reader.
            Box(Modifier.fillMaxSize().background(Theme.bg).windowInsetsPadding(insets)) {
                SettingsSheet(
                    state,
                    startAt = sheet ?: SECTION_MIC,
                    onClose = { sheet = null },
                    Modifier.fillMaxSize().upright(seq.layout, swap = true).graphicsLayer { alpha = fade },
                )
            }
        }
    }
    BackHandler(sheet != null || drawer != null) {
        if (sheet != null) sheet = null else drawer = null
    }
}

const val SECTION_MIC = "mic"
const val SECTION_CAMERA = "camera"
const val SECTION_SPEAKER = "speaker"

/** A rounded black tile of the grid. Light with a black icon when [on] (an open drawer's tile); darker while pressed. */
@Composable
private fun Tile(
    modifier: Modifier,
    on: Boolean = false,
    label: String? = null,
    onTap: () -> Unit,
    onLongPress: (() -> Unit)? = null,
    content: @Composable BoxScope.(fg: Color) -> Unit,
) {
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val bg by animateColorAsState(
        when {
            on -> Theme.ink
            pressed -> Theme.pressed
            else -> Theme.bg
        },
        tween(Theme.STATE_MS),
        label = "tile",
    )
    val fg by animateColorAsState(if (on) Theme.onContent else Theme.ink, tween(Theme.STATE_MS), label = "content")
    Box(
        modifier
            .clip(RoundedCornerShape(Theme.radius))
            .background(bg)
            .combinedClickable(
                interactionSource = source,
                indication = null,
                role = Role.Button,
                onClickLabel = label,
                onLongClick = onLongPress,
                onClick = onTap,
            ),
        contentAlignment = Alignment.Center,
    ) { content(fg) }
}

@Composable
private fun TopBar(seq: TurnSequence, connection: Connection, drawer: Drawer?, onHelp: () -> Unit, onLink: () -> Unit) {
    val height = Tokens.Phone.TOP_BAR_DP.dp
    val icons = seq.icons
    Row(Modifier.fillMaxWidth().height(height), horizontalArrangement = Arrangement.spacedBy(Theme.gap)) {
        Wordmark(seq, Modifier.weight(1f))
        val help = stringResource(Copy.string.section_help)
        Tile(Modifier.fillMaxHeight().aspectRatio(1f), on = drawer == Drawer.HELP, label = help, onTap = onHelp) { fg ->
            Icon(Glyph.HELP, fg, help, Modifier.upright(icons), size = Theme.glyph, stroke = Tokens.Phone.TOP_GLYPH_STROKE)
        }
        val description = stringResource(linkLabel(connection))
        Tile(
            Modifier.fillMaxHeight().aspectRatio(1f).semantics { contentDescription = description },
            on = drawer == Drawer.CONNECTION,
            label = stringResource(Copy.string.section_connection),
            onTap = onLink,
        ) { fg ->
            // While on its way the icon dims softly in and out: no blink, no colour, only the icon.
            val breathing = rememberInfiniteTransition(label = "link")
            val pulse by breathing.animateFloat(
                Tokens.Phone.LINK_PULSE_LOW,
                1f,
                infiniteRepeatable(tween(Tokens.Phone.LINK_PULSE_MS.toInt(), easing = FastOutSlowInEasing), RepeatMode.Reverse),
                label = "pulse",
            )
            val alive = pulses(connection)
            Crossfade(glyphOf(connection), Modifier.upright(icons).graphicsLayer { alpha = if (alive) pulse else 1f }, tween(Theme.STATE_MS * 2), label = "glyph") { g ->
                Icon(g, fg, null, size = Theme.glyph, stroke = Tokens.Phone.TOP_GLYPH_STROKE)
            }
        }
    }
}

/**
 * OWLMIC, letter by letter: in a row while upright; sideways each letter turns upright and they stack from the reader's
 * top, a little apart, at the reader's bottom end of the tile, as in the prototype. It fades with the preview's content
 * when the hold changes.
 */
@Composable
private fun Wordmark(seq: TurnSequence, modifier: Modifier) {
    val up = seq.layout
    val fade by seq.fade
    val (align, inset) = when (up.turn) {
        TURN_LEFT -> Alignment.CenterStart to PaddingValues(start = Tokens.Phone.WORDMARK_SIDE_INSET_DP.dp)
        TURN_RIGHT -> Alignment.CenterEnd to PaddingValues(end = Tokens.Phone.WORDMARK_SIDE_INSET_DP.dp)
        else -> Alignment.CenterStart to PaddingValues(start = Tokens.Phone.WORDMARK_INSET_DP.dp)
    }
    val letter = if (up.sideways) {
        DpSize(Tokens.Phone.WORDMARK_LETTER_SIDE_DP.dp, Tokens.Phone.WORDMARK_LETTER_SIDE_DP.dp)
    } else {
        DpSize(Tokens.Phone.WORDMARK_WIDTH_DP.dp / 6, Tokens.Phone.WORDMARK_HEIGHT_DP.dp)
    }
    val gap = if (up.sideways) Tokens.Phone.WORDMARK_LETTER_GAP_DP.dp else 0.dp
    val name = stringResource(Copy.string.app_name)
    Box(
        modifier.fillMaxHeight().clip(RoundedCornerShape(Theme.radius)).background(Theme.bg).semantics { contentDescription = name }.padding(inset),
        contentAlignment = align,
    ) {
        Row(Modifier.graphicsLayer { alpha = fade }, horizontalArrangement = Arrangement.spacedBy(gap)) {
            wordmarkOrder(up.turn).forEach { c ->
                Image(painterResource(LETTERS.getValue(c)), null, Modifier.size(letter).upright(up), colorFilter = ColorFilter.tint(Theme.ink))
            }
        }
    }
}

private val LETTERS = mapOf(
    'o' to R.drawable.wordmark_o,
    'w' to R.drawable.wordmark_w,
    'l' to R.drawable.wordmark_l,
    'm' to R.drawable.wordmark_m,
    'i' to R.drawable.wordmark_i,
    'c' to R.drawable.wordmark_c,
)

/** What the link tile says to a screen reader: the icon in words. */
private fun linkLabel(c: Connection): Int = when (c) {
    is Connection.Connecting -> Copy.string.link_connecting
    is Connection.Approving -> Copy.string.link_approval
    is Connection.Reconnecting, is Connection.Waiting -> Copy.string.status_reconnecting
    is Connection.Connected -> when {
        c.weak -> Copy.string.link_weak
        c.link == LinkKind.USB_DEBUGGING -> Copy.string.link_usb_debugging
        c.link == LinkKind.USB_TETHERING -> Copy.string.link_usb_tethering
        c.link == LinkKind.WIFI -> if (c.hotspot) Copy.string.link_hotspot else Copy.string.link_wifi
        else -> Copy.string.link_bluetooth
    }
    else -> Copy.string.link_not_connected
}

/**
 * A feature tile: the prototype's icon, slashed while off; no text on it. Why it is paused, restarting or stuck is said
 * to screen readers. Until a PC is connected the icons are dimmed: nothing can start yet.
 */
@Composable
private fun FeatureTile(up: Upright, state: AppState, feature: Feature, modifier: Modifier, onFeature: (Feature) -> Unit, onLongPress: () -> Unit) {
    val look = lookOf(state.feature(feature))
    val (name, glyph) = when (feature) {
        Feature.MIC -> Copy.string.feature_mic to if (look.on) Glyph.MIC else Glyph.MIC_OFF
        Feature.CAMERA -> Copy.string.feature_camera to if (look.on) Glyph.CAMERA else Glyph.CAMERA_OFF
        Feature.SPEAKER -> Copy.string.feature_speaker to if (look.on) Glyph.SPEAKER else Glyph.SPEAKER_OFF
    }
    val note = look.note?.let { stringResource(noteText(it, feature)) }
    val label = stringResource(name)
    val description = listOfNotNull(label, note ?: stringResource(if (look.on) Copy.string.feature_on else Copy.string.feature_off)).joinToString(", ")
    val dim by animateFloatAsState(if (state.connected || look.on) 1f else Tokens.Phone.DIM_ICON_ALPHA, tween(Theme.STATE_MS), label = "dim")
    Tile(modifier.fillMaxHeight().semantics { contentDescription = description }, label = label, onTap = { onFeature(feature) }, onLongPress = onLongPress) { fg ->
        Icon(glyph, fg, null, Modifier.upright(up).graphicsLayer { alpha = dim }, size = Theme.glyph, stroke = Tokens.Phone.GLYPH_STROKE)
    }
}

private fun noteText(note: TileNote, feature: Feature): Int = when (note) {
    TileNote.PAUSED -> Copy.string.feature_paused
    TileNote.RESTARTING -> Copy.string.feature_restarting
    TileNote.FAILED -> Copy.string.feature_failed
    TileNote.NO_BLUETOOTH -> Copy.string.camera_no_bluetooth
    TileNote.PERMISSION -> if (feature == Feature.CAMERA) Copy.string.camera_permission else Copy.string.mic_permission
}
