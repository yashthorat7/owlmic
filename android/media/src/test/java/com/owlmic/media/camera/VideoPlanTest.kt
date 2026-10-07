package com.owlmic.media.camera

import android.view.Surface
import com.owlmic.core.hub.LinkKind
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class VideoPlanTest {
    @Test
    fun autoIs1080pOnACableAnd720pOnWifi() {
        assertEquals(VideoPlan(1920, 1080, 24, 13_000_000), VideoPlan.of("auto", 24, LinkKind.USB_DEBUGGING))
        assertEquals(VideoPlan(1280, 720, 24, 3_000_000), VideoPlan.of("auto", 24, LinkKind.WIFI))
    }

    @Test
    fun startingBitratesFollowTheTable() {
        assertEquals(4_000_000, VideoPlan.of("720p", 30, LinkKind.WIFI).bitrate)
        assertEquals(10_000_000, VideoPlan.of("720p", 30, LinkKind.USB_TETHERING).bitrate)
        assertEquals(6_000_000, VideoPlan.of("1080p", 30, LinkKind.WIFI).bitrate)
        assertEquals(16_000_000, VideoPlan.of("1080p", 30, LinkKind.USB_DEBUGGING).bitrate)
        assertEquals(24_000_000, VideoPlan.of("1080p", 60, LinkKind.USB_DEBUGGING).bitrate)
    }

    @Test
    fun lossLowersTheBitrateAtMostEveryTwoSeconds() {
        var t = 10_000L
        val b = BitrateController(10_000_000) { t }
        assertEquals(8_000_000, b.report(5.0, 10))
        t += 1_000
        assertNull(b.report(5.0, 10))
        t += 1_000
        assertEquals(6_400_000, b.report(5.0, 10))
    }

    @Test
    fun aRisingRoundTripCountsAsCongestion() {
        var t = 10_000L
        val b = BitrateController(10_000_000) { t }
        assertNull(b.report(0.0, 20))
        t += 2_000
        assertEquals(8_000_000, b.report(0.0, 80))
    }

    @Test
    fun fiveCleanSecondsRaiseItBackButNeverAboveTheStart() {
        var t = 10_000L
        val b = BitrateController(10_000_000) { t }
        b.report(5.0, 10)
        t += 4_000
        assertNull(b.report(0.0, 10))
        t += 1_000
        assertEquals(8_800_000, b.report(0.0, 10))
        t += 5_000
        assertEquals(9_680_000, b.report(0.0, 10))
        t += 5_000
        assertEquals(10_000_000, b.report(0.0, 10))
        t += 5_000
        assertNull(b.report(0.0, 10))
    }

    @Test
    fun neverBelowAQuarterOfTheStart() {
        var t = 10_000L
        val b = BitrateController(8_000_000) { t }
        repeat(20) {
            b.report(50.0, 10)
            t += 2_000
        }
        assertEquals(2_000_000, b.current)
    }

    @Test
    fun heatLowersTheFrameRate() {
        assertEquals(60, thermalFps(60, 2))
        assertEquals(24, thermalFps(60, 3))
        assertEquals(15, thermalFps(30, 4))
        assertEquals(15, thermalFps(60, 6))
        assertEquals(24, thermalFps(24, 3))
    }

    @Test
    fun cropKeepsTheCentreAtTheOutputsShape() {
        assertEquals(1f to 1f, centerCrop(1920, 1080, 1280, 720))
        val (x, y) = centerCrop(1080, 1920, 1280, 720)
        assertEquals(1f, x, 0f)
        assertEquals(0.316f, y, 0.001f)
        val (x2, y2) = centerCrop(1920, 1080, 720, 1280)
        assertEquals(0.316f, x2, 0.001f)
        assertEquals(1f, y2, 0f)
    }

    @Test
    fun orientationChangesOnlyWellPastTheDiagonal() {
        assertEquals(Surface.ROTATION_0, surfaceRotationFor(10, Surface.ROTATION_0))
        // 50° is past the 45° boundary but not by enough: still portrait.
        assertEquals(Surface.ROTATION_0, surfaceRotationFor(50, Surface.ROTATION_0))
        assertEquals(Surface.ROTATION_270, surfaceRotationFor(70, Surface.ROTATION_0))
        assertEquals(Surface.ROTATION_270, surfaceRotationFor(40, Surface.ROTATION_270))
        assertEquals(Surface.ROTATION_0, surfaceRotationFor(20, Surface.ROTATION_270))
        assertEquals(Surface.ROTATION_90, surfaceRotationFor(280, Surface.ROTATION_0))
    }

    @Test
    fun targetRotationMatchesLandscapePortraitAndAuto() {
        assertEquals(Surface.ROTATION_90, targetRotationFor(Orientation.LANDSCAPE, Surface.ROTATION_0))
        assertEquals(Surface.ROTATION_270, targetRotationFor(Orientation.LANDSCAPE, Surface.ROTATION_270))
        assertEquals(Surface.ROTATION_0, targetRotationFor(Orientation.PORTRAIT, Surface.ROTATION_0))
        assertEquals(Surface.ROTATION_180, targetRotationFor(Orientation.PORTRAIT, Surface.ROTATION_180))
        assertEquals(Surface.ROTATION_0, targetRotationFor(Orientation.PORTRAIT, Surface.ROTATION_90))
        assertEquals(Surface.ROTATION_90, targetRotationFor(Orientation.AUTO, Surface.ROTATION_90))
        assertEquals(Surface.ROTATION_0, targetRotationFor(Orientation.AUTO, Surface.ROTATION_0))
    }

    @Test
    fun heatDrops1080pTo720pOnlyAtCritical() {
        val p1080 = VideoPlan.of("1080p", 30, LinkKind.USB_TETHERING)
        assertEquals(p1080, thermalPlan(p1080, 3))
        val cooler = thermalPlan(p1080, 4)
        assertEquals(720, cooler.shortSide)
        assertEquals(30, cooler.fps)
        assertTrue(cooler.bitrate < p1080.bitrate)
        val p720 = VideoPlan.of("720p", 30, LinkKind.WIFI)
        assertEquals(p720, thermalPlan(p720, 6))
    }

    @Test
    fun aRefusedPictureHalvesTheBitrateDownToAQuarter() {
        val b = BitrateController(8_000_000) { 0 }
        assertEquals(4_000_000, b.refused())
        assertEquals(2_000_000, b.refused())
        assertNull(b.refused())
    }

    /** [fps] camera frames over 3 s, 2 ms early or late in turn, through a gate set to [wanted]. */
    private fun passed(cameraFps: Int, wanted: Int): Int {
        val gate = FrameGate()
        val period = 1_000_000_000L / cameraFps
        return (0 until cameraFps * 3).count { i -> gate.pass(i * period + if (i % 2 == 0) 2_000_000 else -2_000_000, wanted) }
    }

    @Test
    fun theFrameGateKeepsTheWantedRate() {
        assertTrue(passed(30, 24) in 71..73)
        assertEquals(90, passed(30, 30))
        assertTrue(passed(30, 15) in 44..46)
        assertTrue(passed(60, 30) in 89..91)
    }
}
