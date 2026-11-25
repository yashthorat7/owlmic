package com.owlmic.core.settings

import com.owlmic.core.hub.Hub
import com.owlmic.core.proto.SettingChange

sealed interface SettingsMsg {
    /** The pairing in use: [pcId] null when no PC was ever connected. [pcValues] is WELCOME's settings, on connect only. */
    class Use(val pcId: String?, val connected: Boolean, val pcValues: Map<String, String>? = null) : SettingsMsg

    /** The user changed a setting on the phone. */
    class Change(val id: String, val value: String) : SettingsMsg

    /** The PC changed shared settings (SETTINGS). */
    class Remote(val changes: List<SettingChange>) : SettingsMsg
}

sealed interface SettingsEvent {
    /** All current values; [changed] says which moved, so only those are applied live. */
    class Values(val values: Map<String, String>, val changed: Set<String>) : SettingsEvent

    /** Shared changes for the PC. */
    class Push(val changes: List<SettingChange>) : SettingsEvent
}

/**
 * One settings model per pairing (section 18.1): shared settings sync both ways with versions, changes made while the
 * PC is away wait and go out on reconnect, and everything applies live. After WELCOME the PC sends a SETTINGS snapshot
 * with its versions (protocol section 4); the phone's waiting changes go out once that has arrived, versioned past it.
 */
class SettingsHub(private val store: Store, private val emit: (SettingsEvent) -> Unit) : Hub<SettingsMsg>("settings") {
    private var pcId: String? = null
    private var connected = false
    private var values: Map<String, Versioned> = store.settings(null)
    private var pending: Set<String> = emptySet()

    /** Connected, and the PC's snapshot hasn't arrived yet. */
    private var awaitingSnapshot = false
    private var connections = 0

    init {
        post(SettingsMsg.Use(null, false))
    }

    override fun handle(message: SettingsMsg) {
        when (message) {
            is SettingsMsg.Use -> {
                pcId = message.pcId
                connected = message.connected
                values = SettingValues.defaults() + store.settings(pcId)
                pending = store.pending(pcId)
                val pc = message.pcValues
                awaitingSnapshot = pc != null && connected
                if (pc != null && connected) {
                    values = SettingValues.applyWelcome(values, pc, pending)
                    // A PC that sends no snapshot still gets the waiting changes, a little later.
                    val connection = ++connections
                    later(SNAPSHOT_WAIT_MS) { if (awaitingSnapshot && connection == connections) handle(SettingsMsg.Remote(emptyList())) }
                }
                store.saveSettings(pcId, values, pending)
                emit(SettingsEvent.Values(SettingValues.plain(values), values.keys))
            }
            is SettingsMsg.Change -> {
                val before = values[message.id]?.value
                values = SettingValues.change(values, message.id, message.value)
                val now = values[message.id] ?: return
                if (before == now.value) return
                if (SettingValues.isShared(message.id) && pcId != null) {
                    if (connected && !awaitingSnapshot) {
                        emit(SettingsEvent.Push(listOf(SettingChange(message.id, now.value, now.version))))
                    } else {
                        pending = pending + message.id
                    }
                }
                store.saveSettings(pcId, values, pending)
                emit(SettingsEvent.Values(SettingValues.plain(values), setOf(message.id)))
            }
            is SettingsMsg.Remote -> {
                val snapshot = awaitingSnapshot
                val (next, changed) = SettingValues.applyRemote(
                    values,
                    message.changes,
                    keep = if (snapshot) pending else emptySet(),
                    authoritative = snapshot,
                )
                values = next
                if (snapshot) {
                    awaitingSnapshot = false
                    if (pending.isNotEmpty()) {
                        values = SettingValues.bumpPending(values, pending)
                        emit(SettingsEvent.Push(pending.mapNotNull { id -> values[id]?.let { SettingChange(id, it.value, it.version) } }))
                    }
                    pending = emptySet()
                } else {
                    pending = pending - changed
                }
                store.saveSettings(pcId, values, pending)
                if (changed.isNotEmpty()) emit(SettingsEvent.Values(SettingValues.plain(values), changed))
            }
        }
    }

    /** After a failure: the values in the store are the truth. */
    override fun restart() = handle(SettingsMsg.Use(pcId, connected))

    private companion object {
        const val SNAPSHOT_WAIT_MS = 3_000L
    }
}
