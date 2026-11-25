package com.owlmic.ui

import com.owlmic.core.hub.AppState
import com.owlmic.core.hub.Connection
import com.owlmic.core.hub.FeaturePhase
import com.owlmic.core.hub.FeatureProblem
import com.owlmic.core.hub.FeatureView
import com.owlmic.core.hub.KnownPcView
import com.owlmic.core.hub.LinkKind
import com.owlmic.core.hub.PcChoice
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ScreenModelTest {
    @Test
    fun theLinkTileShowsTheLinkOnceConnectedAndKeepsPulsingGlobeUntilConnected() {
        assertEquals(Glyph.WIFI, glyphOf(Connection.Connected("PC", LinkKind.WIFI)))
        assertEquals(Glyph.WIFI, glyphOf(Connection.Connected("PC", LinkKind.WIFI, weak = true)))
        assertEquals(Glyph.USB, glyphOf(Connection.Connected("PC", LinkKind.USB_DEBUGGING)))
        listOf(
            Connection.Searching(),
            Connection.Searching(unreachable = "PC"),
            Connection.Choosing(2),
            Connection.Connecting("PC", LinkKind.WIFI),
            Connection.Approving("PC", "1", LinkKind.BLUETOOTH),
            Connection.Reconnecting("PC", LinkKind.USB_TETHERING),
            Connection.Waiting("PC"),
            Connection.Denied("PC"),
            Connection.Busy("PC", "Pixel"),
            Connection.UpdateNeeded("PC", phoneOutdated = true),
            Connection.KeyChanged("PC"),
        ).forEach {
            assertEquals(it.toString(), Glyph.GLOBE, glyphOf(it))
            assertTrue(it.toString(), pulses(it))
        }
        assertFalse(pulses(Connection.Connected("PC", LinkKind.WIFI)))
    }

    @Test
    fun theStripSpeaksOnlyWhenThereIsSomethingToReadOrDo() {
        // Looking, connecting, approving, reconnecting and connected say nothing: the link tile shows them.
        listOf(
            Connection.Searching(),
            Connection.Searching(unreachable = "PC"),
            Connection.Connecting("PC", LinkKind.WIFI),
            Connection.Approving("PC", "4821", LinkKind.WIFI),
            Connection.Reconnecting("PC", null),
            Connection.Waiting("PC"),
            Connection.Connected("PC", LinkKind.WIFI),
        ).forEach { assertNull(it.toString(), stripOf(AppState(connection = it))) }
        assertEquals(Strip.CHOOSE, stripOf(AppState(connection = Connection.Choosing(2))))
        assertEquals(Strip.DENIED, stripOf(AppState(connection = Connection.Denied("PC"))))
        assertEquals(Strip.KEY_CHANGED, stripOf(AppState(connection = Connection.KeyChanged("PC"))))
        assertEquals(Strip.TETHERING, stripOf(AppState(tetherHint = true)))
        assertNull(stripOf(AppState(bluetoothOffer = true)))
        assertNull(stripOf(AppState(connection = Connection.Reconnecting("PC", null), tetherHint = true)))

        assertEquals(StripAction.CHOOSE, stripAction(Strip.CHOOSE))
        assertEquals(StripAction.ASK_AGAIN, stripAction(Strip.DENIED))
        assertEquals(StripAction.TETHERING, stripAction(Strip.TETHERING))
        assertNull(stripAction(Strip.KEY_CHANGED))
    }

    @Test
    fun featureTilesShowOnOffAndWhyNot() {
        assertEquals(TileLook(false, null), lookOf(FeatureView()))
        assertEquals(TileLook(true, null), lookOf(FeatureView(FeaturePhase.ON)))
        assertEquals(TileLook(true, null), lookOf(FeatureView(FeaturePhase.STARTING)))
        assertEquals(TileLook(true, TileNote.RESTARTING), lookOf(FeatureView(FeaturePhase.RECOVERING)))
        assertEquals(TileLook(false, TileNote.PAUSED), lookOf(FeatureView(FeaturePhase.PAUSED)))
        assertEquals(TileLook(false, TileNote.FAILED), lookOf(FeatureView(FeaturePhase.FAILED)))
        assertEquals(TileLook(false, TileNote.PERMISSION), lookOf(FeatureView(FeaturePhase.OFF, FeatureProblem.PERMISSION)))
        assertEquals(TileLook(false, TileNote.NO_BLUETOOTH), lookOf(FeatureView(FeaturePhase.PAUSED, FeatureProblem.NO_BLUETOOTH)))
        assertTrue(TileNote.FAILED.alert)
        assertFalse(TileNote.PAUSED.alert)
    }

    @Test
    fun theConnectedPcComesFirstThenTheOthersByLastUseAndBlockedOnesBelow() {
        assertEquals(PcList(emptyList(), emptyList()), pcList(AppState()))

        val state = AppState(
            connection = Connection.Connected("Desk", LinkKind.WIFI, address = "192.168.1.15:7653"),
            choices = listOf(PcChoice("b", "Laptop", "192.168.1.20", LinkKind.WIFI), PcChoice("n", "New", "192.168.1.30", LinkKind.WIFI)),
            knownPcs = listOf(
                KnownPcView("a", "Desk", current = true, lastConnectedAt = 300),
                KnownPcView("c", "Studio", false, address = "10.0.0.7:7653", lastConnectedAt = 100, lastLink = LinkKind.USB_TETHERING),
                KnownPcView("b", "Laptop", false, lastConnectedAt = 200),
                KnownPcView("x", "Office", false, blocked = true, address = "10.0.0.9:7653"),
            ),
            manualAddresses = listOf("10.0.0.42"),
        )
        val list = pcList(state)
        assertEquals(
            listOf(
                PcRow("a", "Desk", PcKind.CURRENT, "192.168.1.15:7653", LinkKind.WIFI, lastConnectedAt = 300),
                PcRow("b", "Laptop", PcKind.PAIRED, "192.168.1.20", LinkKind.WIFI, lastConnectedAt = 200, canConnect = true),
                PcRow("c", "Studio", PcKind.PAIRED, "10.0.0.7:7653", LinkKind.USB_TETHERING, lastConnectedAt = 100),
                PcRow("n", "New", PcKind.NEW, "192.168.1.30", LinkKind.WIFI, canConnect = true),
                PcRow(null, null, PcKind.ADDRESS, "10.0.0.42"),
            ),
            list.rows,
        )
        assertEquals(listOf(PcRow("x", "Office", PcKind.BLOCKED, "10.0.0.9:7653")), list.blocked)
    }

    @Test
    fun aBlockedPcThatAnswersIsNotOfferedAndTheLinksPutUsbThenWifiFirst() {
        val state = AppState(choices = listOf(PcChoice("x", "Office")), knownPcs = listOf(KnownPcView("x", "Office", false, blocked = true)))
        assertEquals(emptyList<PcRow>(), pcList(state).rows)
        assertEquals(listOf(LinkKind.USB_DEBUGGING, LinkKind.USB_TETHERING, LinkKind.WIFI, LinkKind.BLUETOOTH), LINK_SETTINGS.map { it.first })
    }

    @Test
    fun theSoundWaveReachesWellUpForNormalSpeechAndTheTopWhenLoud() {
        assertEquals(0f, waveLevel(0.08f), 0f)
        assertEquals(0.22f, waveLevel(0.16f), 0.02f)
        assertEquals(0.37f, waveLevel(0.25f), 0.02f)
        assertEquals(0.69f, waveLevel(0.50f), 0.02f)
        assertEquals(0.86f, waveLevel(0.65f), 0.02f)
        assertEquals(1f, waveLevel(0.80f), 0.001f)
        assertEquals(1f, waveLevel(1f), 0.001f)
    }

    @Test
    fun rotatingThePhoneSwitchesDirectionalRatiosBetweenLandscapeAndPortrait() {
        assertEquals("16:9", flipRatioForOrientation("9:16", sideways = true))
        assertEquals("9:16", flipRatioForOrientation("16:9", sideways = false))
        assertEquals("4:3", flipRatioForOrientation("3:4", sideways = true))
        assertEquals("3:4", flipRatioForOrientation("4:3", sideways = false))
        assertEquals("2:1", flipRatioForOrientation("1:2", sideways = true))
        assertEquals("1:2", flipRatioForOrientation("2:1", sideways = false))
        assertEquals("full", flipRatioForOrientation("full", sideways = true))
        assertEquals("1:1", flipRatioForOrientation("1:1", sideways = false))

        assertEquals("1:2", toggleRatioOrientation("2:1"))
        assertEquals("2:1", toggleRatioOrientation("1:2"))
        assertNull(toggleRatioOrientation("full"))
        assertNull(toggleRatioOrientation("1:1"))

        assertNull(previewTargetAspect("full", sideways = false))
        assertNull(previewTargetAspect("auto", sideways = true))
        assertEquals(16f / 9f, previewTargetAspect("16:9", sideways = false)!!, 0.0001f)
        assertEquals(9f / 16f, previewTargetAspect("16:9", sideways = true)!!, 0.0001f)
        assertEquals(1f, previewTargetAspect("1:1", sideways = true)!!, 0.0001f)
    }

    @Test
    fun inFlightConnectionAppearsInPcListWithActiveLink() {
        val list = pcList(AppState(connection = Connection.Approving("Desk", "4821", LinkKind.USB_DEBUGGING)))
        assertEquals(
            listOf(PcRow(null, "Desk", PcKind.NEW, null, LinkKind.USB_DEBUGGING)),
            list.rows,
        )
    }
}
