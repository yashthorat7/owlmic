package com.owlmic.core.link

import com.owlmic.core.link.Reconnector.Step
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class MonitorTest {
    private var t = 1_000_000L

    @Test
    fun theLadderClimbsOneStepPerStallAndStopsAtHold() {
        val r = Reconnector { t }
        assertEquals(Step.RESTART_SOURCE, r.stall())
        t += 1_000
        assertEquals(Step.RECREATE_MEDIA, r.stall())
        t += 1_000
        assertEquals(Step.REHANDSHAKE, r.stall())
        t += 1_000
        assertEquals(Step.SWITCH_LINK, r.stall())
        t += 1_000
        assertEquals(Step.HOLD, r.stall())
        t += 1_000
        assertEquals(Step.HOLD, r.stall())
    }

    @Test
    fun aQuietSpellOrRecoveryStartsANewIncident() {
        val r = Reconnector { t }
        r.stall()
        r.stall()
        t += Reconnector.QUIET_MS + 1
        assertEquals(Step.RESTART_SOURCE, r.stall())
        r.stall()
        r.recovered()
        assertEquals(Step.RESTART_SOURCE, r.stall())
    }

    @Test
    fun aLinkIsDeadAfterThreeSilentSecondsOnWirelessTwoOnCable() {
        val wifi = Monitor(wireless = true) { t }
        val usb = Monitor(wireless = false) { t }
        wifi.heard()
        usb.heard()
        t += 2_001
        assertFalse(wifi.dead())
        assertTrue(usb.dead())
        t += 1_000
        assertTrue(wifi.dead())
        wifi.heard()
        assertFalse(wifi.dead())
    }

    @Test
    fun aWarmStandbyIsJudgedByItsSlowerHeartbeat() {
        val standby = Monitor(wireless = false) { t }
        standby.heard()
        // Pinged every 2 s, its answer can be just over 2 s old: not dead yet.
        t += Monitor.STANDBY_HEARTBEAT_MS + 100
        assertFalse(standby.dead(Monitor.STANDBY_HEARTBEAT_MS))
        t += Monitor.STANDBY_HEARTBEAT_MS
        assertTrue(standby.dead(Monitor.STANDBY_HEARTBEAT_MS))
    }

    @Test
    fun reportsEveryTenSeconds() {
        val m = Monitor(wireless = true) { t }
        val stats = InboundStats { t * 1_000 }
        assertNull(m.report(stats, null))
        t += Monitor.REPORT_EVERY_MS
        val report = m.report(stats, 3)
        assertEquals(3, report!!.thermal)
    }

    @Test
    fun reportsEveryTwoSecondsWhenSpeakerIsActive() {
        val m = Monitor(wireless = true) { t }
        val stats = InboundStats { t * 1_000 }
        t += Monitor.SPEAKER_REPORT_EVERY_MS
        assertEquals(0, m.report(stats, null, fast = true)!!.rttMs)
    }

    @Test
    fun aLinkIsWeakWhenAnswersAreLateSlowOrLossy() {
        val m = Monitor(wireless = true) { t }
        m.heard()
        assertFalse(m.weak())
        t += 2_001
        assertTrue(m.weak())
        m.heard()
        assertFalse(m.weak())
        m.peerReport(Monitor.WEAK_LOSS_PCT + 1)
        assertTrue(m.weak())
        // The next report of our own starts the count again.
        t += Monitor.REPORT_EVERY_MS
        m.heard()
        m.report(InboundStats { t * 1_000 }, null)
        assertFalse(m.weak())
    }

    @Test
    fun theSendersClockWrappingIsNotJitter() {
        var us = 0L
        val s = InboundStats { us }
        // Packets 10 ms apart, on time, across the 32-bit wrap of the PC's microsecond clock.
        var ts = 0xFFFFFFFFL - 25_000
        repeat(6) { seq ->
            us += 10_000
            s.add(seq.toLong(), ts, 100)
            ts = (ts + 10_000) and 0xFFFFFFFFL
        }
        assertEquals(0, s.take(1_000).second)
    }

    @Test
    fun inboundLossAndJitterFollowRfc3550() {
        var us = 0L
        val s = InboundStats { us }
        // Seq 0..9 with 2 and 5 missing, every packet exactly on time.
        for (seq in 0L..9L) {
            if (seq == 2L || seq == 5L) continue
            us = seq * 10_000
            s.add(seq, seq * 10_000, 100)
        }
        val (loss, jitterMs, kbps) = s.take(1_000)
        assertEquals(20.0, loss, 0.01)
        assertEquals(0, jitterMs)
        assertEquals(6, kbps)
    }
}
