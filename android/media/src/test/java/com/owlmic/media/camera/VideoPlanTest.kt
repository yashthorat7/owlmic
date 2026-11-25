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
    fun theRatioSetsTheSizeTheShortSideStays() {
        val p1080 = VideoPlan.of("1080p", 30, LinkKind.USB_DEBUGGING)
        assertEquals(1920 to 1080, outputSize(p1080, Ratio.WIDE))
        assertEquals(1440 to 1080, outputSize(p1080, Ratio.CLASSIC))
        assertEquals(1920 to 960, outputSize(p1080, Ratio.CINEMA))
        assertEquals(1080 to 1080, outputSize(p1080, Ratio.SQUARE))
        assertEquals(960 to 1920, outputSize(p1080, Ratio.PORTRAIT_CINEMA))
        assertEquals(1080 to 1440, outputSize(p1080, Ratio.PORTRAIT_CLASSIC))
        assertEquals(1080 to 1920, outputSize(p1080, Ratio.TALL))
        val p720 = VideoPlan.of("720p", 30, LinkKind.WIFI)
        assertEquals(1280 to 720, outputSize(p720, Ratio.WIDE))
        assertEquals(960 to 720, outputSize(p720, Ratio.CLASSIC))
        assertEquals(1280 to 640, outputSize(p720, Ratio.CINEMA))
        assertEquals(720 to 720, outputSize(p720, Ratio.SQUARE))
        assertEquals(640 to 1280, outputSize(p720, Ratio.PORTRAIT_CINEMA))
        assertEquals(720 to 960, outputSize(p720, Ratio.PORTRAIT_CLASSIC))
        assertEquals(720 to 1280, outputSize(p720, Ratio.TALL))
    }

    @Test
    fun presetsKeepTheirShapeEvenWhenPhoneRotates() {
        val p720 = VideoPlan.of("720p", 30, LinkKind.WIFI)
        val upright = 720 to 960
        val sideways = 960 to 720
        for (frame in listOf(upright, sideways)) {
            assertEquals(720 to 960, outputSize(p720, Ratio.PORTRAIT_CLASSIC, frame))
            assertEquals(960 to 720, outputSize(p720, Ratio.CLASSIC, frame))
            assertEquals(640 to 1280, outputSize(p720, Ratio.PORTRAIT_CINEMA, frame))
            assertEquals(1280 to 640, outputSize(p720, Ratio.CINEMA, frame))
            assertEquals(720 to 1280, outputSize(p720, Ratio.TALL, frame))
            assertEquals(1280 to 720, outputSize(p720, Ratio.WIDE, frame))
            assertEquals(720 to 720, outputSize(p720, Ratio.SQUARE, frame))
        }
    }

    @Test
    fun ratiosReadFromTheSettingAndTheDefaultIsTheFullFrame() {
        assertEquals(Ratio.CLASSIC, Ratio.of("4:3"))
        assertEquals(Ratio.CINEMA, Ratio.of("2:1"))
        assertEquals(Ratio.PORTRAIT_CINEMA, Ratio.of("1:2"))
        assertEquals(Ratio.PORTRAIT_CLASSIC, Ratio.of("3:4"))
        assertEquals(Ratio.TALL, Ratio.of("9:16"))
        assertEquals(Ratio.FULL, Ratio.of("full"))
        assertEquals(Ratio.FULL, Ratio.of(null))
        assertEquals(Ratio.FULL, Ratio.of("21:9"))
    }

    @Test
    fun theFullFrameKeepsTheSensorsShapeUprightForTheHold() {
        val p1080 = VideoPlan.of("1080p", 30, LinkKind.USB_DEBUGGING)
        val p720 = VideoPlan.of("720p", 30, LinkKind.WIFI)
        // A 4:3 sensor: sideways 4:3, upright 3:4, nothing cut.
        assertEquals(1440 to 1080, outputSize(p1080, Ratio.FULL, 1440 to 1080))
        assertEquals(1080 to 1440, outputSize(p1080, Ratio.FULL, 1080 to 1440))
        assertEquals(960 to 720, outputSize(p720, Ratio.FULL, 4032 to 3024))
        assertEquals(720 to 960, outputSize(p720, Ratio.FULL, 3024 to 4032))
        // A 16:9 sensor keeps 16:9.
        assertEquals(1920 to 1080, outputSize(p1080, Ratio.FULL, 1920 to 1080))
        assertEquals(720 to 1280, outputSize(p720, Ratio.FULL, 1080 to 1920))
        // Until the camera has a size there is nothing to size by.
        assertNull(outputSize(p1080, Ratio.FULL, null))
        assertNull(outputSize(p1080, Ratio.FULL, 0 to 0))
    }

    @Test
    fun anOddSensorRoundsTheLongSideToSixteenAndBarelyMoves() {
        // 4080×3072 is 1.328:1; 1080 × 1.328 = 1434.4, rounded to 1440.
        val (w, h) = fullSize(1080, 4080 to 3072)
        assertEquals(1440 to 1080, w to h)
        assertTrue(Math.abs(w.toDouble() / h - 4080.0 / 3072) < 0.01)
        assertEquals(1080 to 1440, fullSize(1080, 3072 to 4080))
        assertEquals(0, fullSize(720, 1600 to 1200).first % 16)
    }

    @Test
    fun theFullFrameBindsTheSensorsOwnFramePresetsTheOneTheyLoseLeastOf() {
        assertTrue(boundFourByThree(Ratio.FULL, sensorWide = false))
        assertTrue(!boundFourByThree(Ratio.FULL, sensorWide = true))
        assertTrue(boundFourByThree(Ratio.CLASSIC, sensorWide = true))
        assertTrue(boundFourByThree(Ratio.SQUARE, sensorWide = false))
        assertTrue(!boundFourByThree(Ratio.WIDE, sensorWide = false))
        assertTrue(!boundFourByThree(Ratio.TALL, sensorWide = false))
        assertTrue(!sensorIsWide(4032, 3024))
        assertTrue(sensorIsWide(5120, 2880))
        assertTrue(!sensorIsWide(0, 0))
    }

    @Test
    fun theBackdropDuplicatesTheShownPictureNotWhatWasCut() {
        // The full frame, upright 3:4, fitted in a tall preview: the backdrop covers it from the same picture.
        // A preview taller than 3:4 is covered by the picture's full height and its middle 0.54 / 0.75 of the width.
        val (fx, fy) = coverCrop(1080, 1440, 1080, 1440, 1080, 2000)
        assertEquals(1f, fy, 0.001f)
        assertEquals(0.54f / 0.75f, fx, 0.001f)
        // A 16:9 preset cut from an upright 3:4 picture: the backdrop never reaches past the 16:9 band.
        val (_, by) = coverCrop(1080, 1440, 1920, 1080, 1080, 2000)
        val (_, band) = centerCrop(1080, 1440, 1920, 1080)
        assertTrue(by <= band + 1e-6f)
    }

    @Test
    fun fewerPixelsStartAtAProportionalBitrate() {
        val p = VideoPlan.of("1080p", 30, LinkKind.USB_DEBUGGING)
        assertEquals(p.bitrate, bitrateFor(p, 1920, 1080))
        assertEquals(p.bitrate, bitrateFor(p, 1080, 1920))
        assertEquals(p.bitrate * 3 / 4, bitrateFor(p, 1440, 1080))
        assertEquals(9_000_000, bitrateFor(p, 1080, 1080))
    }

    @Test
    fun theCameraIsBoundInTheSensorsLandscapeTermsForItsFrame() {
        val p = VideoPlan.of("1080p", 30, LinkKind.USB_DEBUGGING)
        assertEquals(1920 to 1080, boundSize(p, fourByThree = false))
        assertEquals(1440 to 1080, boundSize(p, fourByThree = true))
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

    @Test
    fun fpsRangeSelectionNeverPicksBelow24LowerWhenStableRangeExists() {
        val ranges = listOf(15_000 to 60_000, 24_000 to 60_000, 30_000 to 30_000, 12_000 to 30_000)
        assertEquals(30_000 to 30_000, selectFpsRange(ranges, 30))
        assertEquals(24_000 to 60_000, selectFpsRange(ranges, 60))
        assertEquals(15_000 to 60_000, selectFpsRange(listOf(15_000 to 60_000, 15_000 to 30_000), 60))
    }

    @Test
    fun streamOptimizerDrops60To30FirstAndNeverGoesBelow24Fps() {
        var t = 10_000L
        val opt = StreamOptimizer("1080p", 60, LinkKind.WIFI) { t }
        assertEquals(1080, opt.current.shortSide)
        assertEquals(60, opt.current.fps)

        // First stress step at 60 fps lowers fps to 30 while preserving 1080p.
        t += 2_000
        val s1 = opt.report(lossPct = 4.0, rttMs = 20)
        assertEquals(30, s1?.plan?.fps)
        assertEquals(1080, s1?.plan?.shortSide)

        // Sustained severe drops step 1080p -> 720p -> 480p -> 360p and stop at 24 fps floor.
        repeat(12) {
            t += 2_000
            opt.report(lossPct = 10.0, rttMs = 20, camDropPct = 15.0)
        }
        assertTrue(opt.current.fps >= 24)
        assertEquals(360, opt.current.shortSide)
    }

    @Test
    fun streamOptimizerSlidesQualityBackUpWhenCleanHeadroomReturns() {
        var t = 10_000L
        val opt = StreamOptimizer("auto", 30, LinkKind.WIFI) { t }
        // Starts at ideal 720p 30fps, steps down to 480p under pressure, and recovers back to 720p.
        assertEquals(720, opt.current.shortSide)
        t += 2_000
        opt.report(lossPct = 10.0, rttMs = 20, camDropPct = 12.0)
        assertEquals(480, opt.current.shortSide)

        // Sustained clean reports climb 480p -> 720p cleanly without bloated 1080p promotions.
        repeat(6) {
            t += 8_500
            opt.report(lossPct = 0.0, rttMs = 15, camLossPct = 0.0, camFps = opt.current.fps, camDropPct = 0.0)
        }
        assertEquals(720, opt.current.shortSide)
        assertEquals(30, opt.current.fps)
    }
}
