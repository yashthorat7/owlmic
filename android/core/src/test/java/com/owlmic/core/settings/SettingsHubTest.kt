package com.owlmic.core.settings

import com.owlmic.core.proto.SettingChange
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit

class SettingsHubTest {
    @get:Rule val folder = TemporaryFolder()

    private val events = LinkedBlockingQueue<SettingsEvent>()

    private fun next(): SettingsEvent = events.poll(2, TimeUnit.SECONDS) ?: error("no event")

    private inline fun <reified E : SettingsEvent> nextOf(): E {
        while (true) {
            val e = next()
            if (e is E) return e
        }
    }

    private fun store() = Store(
        folder.root.resolve("s.json"),
        object : KeyWrapper {
            override fun wrap(secret: ByteArray) = secret

            override fun unwrap(blob: ByteArray) = blob
        },
        write = Runnable::run,
    )

    @Test
    fun aChangeMadeWhileAwayGoesOutOnReconnect() {
        val store = store()
        store.remember("aa", "DESKTOP-A", byteArrayOf(4), ByteArray(32), null)
        val hub = SettingsHub(store) { events.put(it) }
        nextOf<SettingsEvent.Values>()

        hub.post(SettingsMsg.Use("aa", connected = false))
        nextOf<SettingsEvent.Values>()
        hub.post(SettingsMsg.Change("camera.lens", "back"))
        assertEquals("back", nextOf<SettingsEvent.Values>().values["camera.lens"])

        hub.post(SettingsMsg.Use("aa", connected = true, pcValues = mapOf("camera.lens" to "front", "camera.frameRate" to "30")))
        val values = nextOf<SettingsEvent.Values>().values
        assertEquals("back", values["camera.lens"])
        assertEquals("30", values["camera.frameRate"])

        // The PC's snapshot after WELCOME carries its versions; the waiting change goes out past them.
        hub.post(SettingsMsg.Remote(listOf(SettingChange("camera.lens", "front", 40), SettingChange("camera.frameRate", "30", 41))))
        val push = nextOf<SettingsEvent.Push>()
        assertEquals(listOf("camera.lens"), push.changes.map { it.id })
        assertEquals("back", push.changes.single().value)
        assertTrue(push.changes.single().version > 41)
        hub.close()
    }

    @Test
    fun afterTheSnapshotAPhoneChangeOutranksThePcsVersions() {
        val store = store()
        store.remember("aa", "DESKTOP-A", byteArrayOf(4), ByteArray(32), null)
        val hub = SettingsHub(store) { events.put(it) }
        hub.post(SettingsMsg.Use("aa", connected = true, pcValues = mapOf("mic.volume" to "0")))
        hub.post(SettingsMsg.Remote(listOf(SettingChange("mic.volume", "0", 100))))
        hub.post(SettingsMsg.Change("mic.volume", "6"))
        val push = nextOf<SettingsEvent.Push>()
        assertEquals(SettingChange("mic.volume", "6", 101), push.changes.single())
        hub.close()
    }

    @Test
    fun whileConnectedAChangeIsPushedAndThePcsChangesApply() {
        val store = store()
        store.remember("aa", "DESKTOP-A", byteArrayOf(4), ByteArray(32), null)
        val hub = SettingsHub(store) { events.put(it) }
        hub.post(SettingsMsg.Use("aa", connected = true, pcValues = emptyMap()))
        hub.post(SettingsMsg.Remote(emptyList()))
        hub.post(SettingsMsg.Change("mic.volume", "6"))
        val push = nextOf<SettingsEvent.Push>()
        assertEquals("6", push.changes.single().value)

        hub.post(SettingsMsg.Remote(listOf(SettingChange("mic.volume", "12", push.changes.single().version + 1))))
        val after = nextOf<SettingsEvent.Values>()
        assertTrue("mic.volume" in after.changed || after.values["mic.volume"] == "6")
        val applied = if (after.values["mic.volume"] == "12") after else nextOf()
        assertEquals("12", applied.values["mic.volume"])
        hub.close()
    }

    @Test
    fun pcSnapshotRealignsNonPendingVersionsSoPcFlipAndMirrorApplyImmediately() {
        val store = store()
        store.remember("aa", "DESKTOP-A", byteArrayOf(4), ByteArray(32), null)
        store.saveSettings("aa", mapOf("camera.lens" to Versioned("back", 5)), emptySet())
        val hub = SettingsHub(store) { events.put(it) }
        nextOf<SettingsEvent.Values>()

        hub.post(SettingsMsg.Use("aa", connected = true, pcValues = mapOf("camera.lens" to "front", "camera.mirror" to "off")))
        nextOf<SettingsEvent.Values>()
        hub.post(SettingsMsg.Remote(listOf(SettingChange("camera.lens", "front", 0), SettingChange("camera.mirror", "off", 0))))

        hub.post(SettingsMsg.Remote(listOf(SettingChange("camera.lens", "back", 1), SettingChange("camera.mirror", "on", 2))))
        var v = nextOf<SettingsEvent.Values>()
        while ("camera.lens" !in v.changed) v = nextOf()
        assertEquals("back", v.values["camera.lens"])
        assertEquals("on", v.values["camera.mirror"])
        hub.close()
    }

    @Test
    fun hidePreviewAndLinkSettingsSyncBidirectionallyAndPersistOffline() {
        val store = store()
        store.remember("aa", "DESKTOP-A", byteArrayOf(4), ByteArray(32), null)
        val hub = SettingsHub(store) { events.put(it) }
        nextOf<SettingsEvent.Values>()

        hub.post(SettingsMsg.Use("aa", connected = true, pcValues = mapOf("camera.hidePreview" to "off", "link.wifi" to "on")))
        nextOf<SettingsEvent.Values>()
        hub.post(SettingsMsg.Remote(listOf(SettingChange("camera.hidePreview", "off", 1), SettingChange("link.wifi", "on", 1))))

        hub.post(SettingsMsg.Change("camera.hidePreview", "on"))
        val push = nextOf<SettingsEvent.Push>()
        assertEquals(SettingChange("camera.hidePreview", "on", 2), push.changes.single())
        assertEquals("on", nextOf<SettingsEvent.Values>().values["camera.hidePreview"])

        hub.post(SettingsMsg.Remote(listOf(SettingChange("camera.hidePreview", "off", 3), SettingChange("link.wifi", "off", 4))))
        var v = nextOf<SettingsEvent.Values>()
        while (v.values["camera.hidePreview"] != "off" || "camera.hidePreview" !in v.changed) v = nextOf()
        assertEquals("off", v.values["camera.hidePreview"])
        assertEquals("off", v.values["link.wifi"])

        hub.post(SettingsMsg.Use(null, connected = false))
        var offline = nextOf<SettingsEvent.Values>()
        while (offline.values["link.wifi"] != "off") offline = nextOf()
        assertEquals("off", offline.values["link.wifi"])
        hub.close()
    }
}
