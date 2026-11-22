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
    fun theLinkTileShowsTheLinkOnceConnectedTheGlobeUntilThenOrTheAlert() {
        assertEquals(Glyph.WIFI, glyphOf(Connection.Connected("PC", LinkKind.WIFI)))
        assertEquals(Glyph.WIFI, glyphOf(Connection.Connected("PC", LinkKind.WIFI, weak = true)))
        assertEquals(Glyph.USB, glyphOf(Connection.Connected("PC", LinkKind.USB_DEBUGGING)))
        listOf(
            Connection.Searching(),
            Connection.Choosing(2),
            Connection.Connecting("PC", LinkKind.WIFI),
            Connection.Approving("PC", "1", LinkKind.BLUETOOTH),
            Connection.Reconnecting("PC", LinkKind.USB_TETHERING),
            Connection.Waiting("PC"),
        ).forEach { assertEquals(it.toString(), Glyph.GLOBE, glyphOf(it)) }
        listOf(
            Connection.Searching(unreachable = "PC"),
            Connection.Denied("PC"),
            Connection.Busy("PC", "Pixel"),
            Connection.UpdateNeeded("PC", phoneOutdated = true),
            Connection.KeyChanged("PC"),
        ).forEach { assertEquals(it.toString(), Glyph.ALERT, glyphOf(it)) }
    }

    @Test
    fun theLinkTileDimsInAndOutOnlyOnTheWay() {
        listOf(
            Connection.Searching(),
            Connection.Connecting("PC", LinkKind.WIFI),
            Connection.Approving("PC", "4821", LinkKind.WIFI),
            Connection.Reconnecting("PC", LinkKind.WIFI),
            Connection.Waiting("PC"),
        ).forEach { assertTrue(it.toString(), pulses(it)) }
        listOf(Connection.Connected("PC", LinkKind.WIFI), Connection.Searching(unreachable = "PC"), Connection.Denied("PC"), Connection.Choosing(2))
            .forEach { assertFalse(it.toString(), pulses(it)) }
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
        assertEquals(listOf(LinkKind.USB_DEBUGGING, LinkKind.WIFI, LinkKind.USB_TETHERING, LinkKind.BLUETOOTH), LINK_SETTINGS.map { it.first })
    }

    @Test
    fun theSoundWaveReachesWellUpForNormalSpeechAndTheTopWhenLoud() {
        assertEquals(0f, waveLevel(0.1f), 0f)
        // Normal speech (about -26 dBFS, 0.71 on the meter) reaches past the middle, scaled with 1.2x threshold.
        assertEquals(0.53f, waveLevel(0.71f), 0.02f)
        assertEquals(1f, waveLevel(0.92f), 0.001f)
        assertEquals(1f, waveLevel(1f), 0.001f)
    }
}
