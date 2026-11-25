package com.owlmic.core.settings

import com.owlmic.core.design.Scope
import com.owlmic.core.design.SettingsModel
import com.owlmic.core.proto.SettingChange

/** A setting's value and its version: the newest change wins, and on a tie the PC does (section 18.1). */
data class Versioned(val value: String, val version: Long)

object SettingValues {
    /** Every setting the phone keeps (shared and phone-only), at its default. */
    fun defaults(): Map<String, Versioned> =
        SettingsModel.ALL.filter { it.scope != Scope.PC }.associate { it.id to Versioned(it.default, 0) }

    fun isValid(id: String, value: String) = SettingsModel.find(id)?.values?.contains(value) == true

    fun isShared(id: String) = SettingsModel.find(id)?.scope == Scope.SHARED

    /** The phone changes [id]: its version moves past everything seen so far. */
    fun change(values: Map<String, Versioned>, id: String, value: String): Map<String, Versioned> {
        if (!isValid(id, value)) return values
        val version = (values.values.maxOfOrNull { it.version } ?: 0) + 1
        return values + (id to Versioned(value, version))
    }

    /**
     * The PC's changes: a shared setting moves when the change is at least as new as ours, except those in [keep].
     * When [authoritative] is true (the PC's snapshot right after WELCOME), non-pending shared settings align to the
     * PC's exact value and version so stale local versions never block future PC edits.
     * Returns the new map and what changed.
     */
    fun applyRemote(
        values: Map<String, Versioned>,
        changes: List<SettingChange>,
        keep: Set<String> = emptySet(),
        authoritative: Boolean = false,
    ): Pair<Map<String, Versioned>, Set<String>> {
        val out = values.toMutableMap()
        val changed = mutableSetOf<String>()
        for (c in changes) {
            if (!isShared(c.id) || !isValid(c.id, c.value) || c.id in keep) continue
            val mine = out[c.id]
            if (authoritative || mine == null || c.version >= mine.version) {
                if (mine?.value != c.value) changed += c.id
                out[c.id] = Versioned(c.value, if (authoritative) c.version else maxOf(c.version, mine?.version ?: 0))
            }
        }
        return out to changed
    }

    /**
     * WELCOME's settings carry no versions, so the PC's value stands for every shared setting except those the phone
     * changed while away ([pending]); the versions follow in the PC's SETTINGS snapshot.
     */
    fun applyWelcome(values: Map<String, Versioned>, pcValues: Map<String, String>, pending: Set<String>): Map<String, Versioned> {
        val out = values.toMutableMap()
        for ((id, value) in pcValues) {
            if (!isShared(id) || !isValid(id, value) || id in pending) continue
            out[id] = Versioned(value, out[id]?.version ?: 0)
        }
        return out
    }

    /**
     * The phone's changes made while away, versioned past everything either side has seen, so the PC takes them: the
     * user made them last, on the phone.
     */
    fun bumpPending(values: Map<String, Versioned>, pending: Set<String>): Map<String, Versioned> {
        var version = values.values.maxOfOrNull { it.version } ?: 0
        return values + pending.mapNotNull { id -> values[id]?.let { id to it.copy(version = ++version) } }
    }

    fun plain(values: Map<String, Versioned>) = values.mapValues { it.value.value }
}
