package com.owlmic.ui

import com.owlmic.core.hub.AppState
import com.owlmic.core.hub.Connection
import com.owlmic.core.hub.FeaturePhase
import com.owlmic.core.hub.FeatureProblem
import com.owlmic.core.hub.FeatureView
import com.owlmic.core.hub.LinkKind
import kotlin.math.pow

/**
 * The link tile's icon, always in the grid's ink: the link once connected; the globe until then; the alert when
 * something needs the user.
 */
fun glyphOf(c: Connection): Glyph = when (c) {
    is Connection.Connected -> glyphOf(c.link)
    is Connection.Searching -> if (c.unreachable != null) Glyph.ALERT else Glyph.GLOBE
    is Connection.Denied, is Connection.Busy, is Connection.UpdateNeeded, is Connection.KeyChanged -> Glyph.ALERT
    else -> Glyph.GLOBE
}

fun glyphOf(link: LinkKind): Glyph = when (link) {
    LinkKind.USB_DEBUGGING -> Glyph.USB
    LinkKind.USB_TETHERING -> Glyph.CABLE
    LinkKind.WIFI -> Glyph.WIFI
    LinkKind.BLUETOOTH -> Glyph.BLUETOOTH
}

/** The link tile's icon dims softly in and out while the phone is on its way to a PC, so "not yet" reads without a colour. */
fun pulses(c: Connection): Boolean = when (c) {
    is Connection.Searching -> c.unreachable == null
    is Connection.Connecting, is Connection.Approving, is Connection.Reconnecting, is Connection.Waiting -> true
    else -> false
}

/**
 * The status strip, only when there is something to read or do: a step only the user can take.
 * Looking, connecting and reconnecting say nothing: the link tile shows them.
 */
enum class Strip { CHOOSE, DENIED, BUSY, UPDATE, KEY_CHANGED, TETHERING }

fun stripOf(state: AppState): Strip? = when (state.connection) {
    is Connection.Choosing -> Strip.CHOOSE
    is Connection.Denied -> Strip.DENIED
    is Connection.Busy -> Strip.BUSY
    is Connection.UpdateNeeded -> Strip.UPDATE
    is Connection.KeyChanged -> Strip.KEY_CHANGED
    is Connection.Searching -> when {
        state.tetherHint -> Strip.TETHERING
        else -> null
    }
    else -> null
}

/** What tapping the status strip does, when it does anything. */
enum class StripAction { TETHERING, ASK_AGAIN, CHOOSE }

fun stripAction(strip: Strip?): StripAction? = when (strip) {
    Strip.CHOOSE -> StripAction.CHOOSE
    Strip.DENIED -> StripAction.ASK_AGAIN
    Strip.TETHERING -> StripAction.TETHERING
    else -> null
}

/** Why a feature tile is the way it is, for screen readers: the tile itself shows only its icon. */
enum class TileNote(val alert: Boolean) {
    PAUSED(false),
    RESTARTING(false),
    FAILED(true),
    PERMISSION(true),
    NO_BLUETOOTH(true),
}

/** A feature tile: the "on" icon or the slashed one, and why, when it isn't simply on or off. */
data class TileLook(val on: Boolean, val note: TileNote?)

fun lookOf(v: FeatureView): TileLook = when {
    v.problem == FeatureProblem.PERMISSION -> TileLook(false, TileNote.PERMISSION)
    v.problem == FeatureProblem.NO_BLUETOOTH -> TileLook(false, TileNote.NO_BLUETOOTH)
    v.phase == FeaturePhase.FAILED -> TileLook(false, TileNote.FAILED)
    v.phase == FeaturePhase.PAUSED -> TileLook(false, TileNote.PAUSED)
    v.phase == FeaturePhase.RECOVERING -> TileLook(true, TileNote.RESTARTING)
    else -> TileLook(v.isOn, null)
}

/** Where a PC sits in the connection list. */
enum class PcKind { CURRENT, PAIRED, NEW, ADDRESS, BLOCKED }

/**
 * One entry of the connection list. [id] is null for an address added by hand. [canConnect]: the PC answered and
 * waits to be picked. [lastConnectedAt] is 0 for never.
 */
data class PcRow(
    val id: String?,
    val name: String?,
    val kind: PcKind,
    val address: String? = null,
    val link: LinkKind? = null,
    val hotspot: Boolean = false,
    val lastConnectedAt: Long = 0,
    val canConnect: Boolean = false,
) {
    /** Stable across updates, so an open entry stays open. */
    val key get() = id ?: "address:$address"
}

/**
 * The connection list: the connected PC first, then the other PCs this phone used (most recent first), new PCs that
 * answered, and addresses added by hand; [blocked] go below a line.
 */
data class PcList(val rows: List<PcRow>, val blocked: List<PcRow>)

fun pcList(state: AppState): PcList {
    val rows = mutableListOf<PcRow>()
    val waiting = state.choices.associateBy { it.id }
    val c = state.connection as? Connection.Connected
    val known = state.knownPcs.filter { !it.blocked }
    if (c != null) {
        val me = known.firstOrNull { it.current }
        rows += PcRow(me?.id, c.pc, PcKind.CURRENT, c.address ?: me?.address, c.link, c.hotspot, me?.lastConnectedAt ?: 0)
    }
    known.filter { !(it.current && c != null) }.sortedByDescending { it.lastConnectedAt }.forEach {
        rows += PcRow(
            it.id, it.name, PcKind.PAIRED, waiting[it.id]?.address ?: it.address, waiting[it.id]?.link ?: it.lastLink,
            lastConnectedAt = it.lastConnectedAt, canConnect = it.id in waiting,
        )
    }
    val listed = state.knownPcs.map { it.id }.toSet()
    state.choices.filter { it.id !in listed }.forEach { rows += PcRow(it.id, it.name, PcKind.NEW, it.address, it.link, canConnect = true) }
    state.manualAddresses.forEach { rows += PcRow(null, null, PcKind.ADDRESS, it) }
    val blocked = state.knownPcs.filter { it.blocked }.map { PcRow(it.id, it.name, PcKind.BLOCKED, it.address, it.lastLink, lastConnectedAt = it.lastConnectedAt) }
    return PcList(rows, blocked)
}

/** The four links the connection list can switch off and on, with their settings: USB first, then Wi-Fi, then the rest. */
val LINK_SETTINGS = listOf(
    LinkKind.USB_DEBUGGING to "link.usbDebugging",
    LinkKind.WIFI to "link.wifi",
    LinkKind.USB_TETHERING to "link.usbTethering",
    LinkKind.BLUETOOTH to "link.bluetooth",
)

/**
 * The mic's level as the sound wave shows it: under the noise floor threshold it is flat, normal speech reaches the
 * top. Only the picture: the mic's sound is untouched.
 */
const val WAVE_THRESHOLD = 0.08f

fun waveLevel(level: Float): Float {
    val v = ((level - WAVE_THRESHOLD) / (1f - WAVE_THRESHOLD)).coerceIn(0f, 1f)
    return (v.pow(WAVE_EXPONENT) * WAVE_GAIN).coerceAtMost(1f)
}

private const val WAVE_EXPONENT = 0.7f
private const val WAVE_GAIN = 1.5f

/** Switches directional ratios between landscape (x:y) and portrait (y:x) to match phone orientation. */
fun flipRatioForOrientation(ratio: String, sideways: Boolean): String = when (ratio) {
    "16:9", "9:16" -> if (sideways) "16:9" else "9:16"
    "4:3", "3:4" -> if (sideways) "4:3" else "3:4"
    "2:1", "1:2" -> if (sideways) "2:1" else "1:2"
    else -> ratio
}

/** Flips an explicit directional ratio to its opposite orientation, or null for Auto / 1:1. */
fun toggleRatioOrientation(ratio: String): String? = when (ratio) {
    "16:9" -> "9:16"
    "9:16" -> "16:9"
    "4:3" -> "3:4"
    "3:4" -> "4:3"
    "2:1" -> "1:2"
    "1:2" -> "2:1"
    else -> null
}
