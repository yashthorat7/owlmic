package com.owlmic.core.link

import com.owlmic.core.proto.Report
import com.owlmic.core.proto.Stream

/** Loss, jitter and rate of one incoming stream, from seq numbers and timestamps (RFC 3550 style jitter). */
class InboundStats(private val nowUs: () -> Long = { System.nanoTime() / 1_000 }) {
    private var highest = -1L
    private var received = 0L
    private var expectedBase = -1L
    private var bytes = 0L
    private var jitterUs = 0.0
    private var lastTransit = Long.MIN_VALUE

    @Volatile var lastArrivalUs = 0L
        private set

    @Synchronized
    fun add(seq: Long, timestampUs: Long, size: Int) {
        val arrival = nowUs()
        lastArrivalUs = arrival
        if (expectedBase < 0) expectedBase = seq
        if (highest < 0 || ((seq - highest) and 0xFFFFFFFFL) in 1 until (1L shl 31)) highest = seq
        received++
        bytes += size
        val transit = arrival - timestampUs
        if (lastTransit != Long.MIN_VALUE) {
            // The sender's clock is 32 bits of microseconds and wraps every 71.6 minutes: compare modulo 2^32.
            var d = (transit - lastTransit) and 0xFFFFFFFFL
            if (d >= 1L shl 31) d = (1L shl 32) - d
            jitterUs += (d - jitterUs) / 16
        }
        lastTransit = transit
    }

    /** Loss in percent, jitter in ms and rate in kbps since the last call, which starts a new interval. */
    @Synchronized
    fun take(intervalMs: Long): Triple<Double, Int, Int> {
        if (expectedBase < 0 || highest < 0) return Triple(0.0, 0, 0)
        val expected = ((highest - expectedBase) and 0xFFFFFFFFL) + 1
        val loss = if (expected > 0) (100.0 * (expected - received) / expected).coerceIn(0.0, 100.0) else 0.0
        val kbps = if (intervalMs > 0) (bytes * 8 / intervalMs).toInt() else 0
        val result = Triple(loss, (jitterUs / 1_000).toInt(), kbps)
        expectedBase = (highest + 1) and 0xFFFFFFFFL
        received = 0
        bytes = 0
        return result
    }
}

/**
 * Link health (section 14.6): a heartbeat a second, a report every 10 s, a dead link after 3 missed heartbeats on
 * wireless and 2 on cable. Memory only. All calls on the Link Hub.
 */
class Monitor(private val wireless: Boolean, private val now: () -> Long = System::currentTimeMillis) {
    private var lastHeard = now()
    private var lastReport = now()
    private var lossPct = 0.0
    var rttMs = 0
        private set

    fun heard() {
        lastHeard = now()
    }

    fun pong(sentUs: Long) {
        heard()
        val rtt = ((System.nanoTime() / 1_000 - sentUs) / 1_000).toInt()
        if (rtt >= 0) rttMs = if (rttMs == 0) rtt else (rttMs * 7 + rtt) / 8
    }

    /** Three missed heartbeats of [heartbeatMs] on wireless, two on cable. A warm standby is pinged half as often. */
    fun dead(heartbeatMs: Long = HEARTBEAT_MS): Boolean = now() - lastHeard > heartbeatMs * if (wireless) MISSED_WIRELESS else MISSED_CABLE

    /** A REPORT when one is due, else null. Sends every 2 s while Speaker is active for fast adaptation. */
    fun report(speaker: InboundStats, thermal: Int?, fast: Boolean = false): Report? {
        val t = now()
        val every = if (fast) SPEAKER_REPORT_EVERY_MS else REPORT_EVERY_MS
        if (t - lastReport < every) return null
        val interval = t - lastReport
        lastReport = t
        val (loss, jitter, kbps) = speaker.take(interval)
        lossPct = loss
        return Report(loss, jitter, rttMs, if (kbps > 0) mapOf(Stream.SPEAKER.toString() to kbps) else emptyMap(), thermal)
    }

    /** The PC's REPORT: the loss it sees on what the phone sends. */
    fun peerReport(peerLossPct: Double) {
        lossPct = maxOf(lossPct, peerLossPct)
    }

    /**
     * A link that works but struggles (section 36): an answer more than two heartbeats late, a round trip over
     * [WEAK_RTT_MS], or more than [WEAK_LOSS_PCT] loss in the last report either way.
     */
    fun weak(): Boolean = now() - lastHeard > HEARTBEAT_MS * 2 || rttMs > WEAK_RTT_MS || lossPct > WEAK_LOSS_PCT

    companion object {
        const val HEARTBEAT_MS = 1_000L
        const val STANDBY_HEARTBEAT_MS = 2_000L
        const val REPORT_EVERY_MS = 10_000L
        const val SPEAKER_REPORT_EVERY_MS = 2_000L
        const val MISSED_WIRELESS = 3
        const val MISSED_CABLE = 2
        const val WEAK_RTT_MS = 150
        const val WEAK_LOSS_PCT = 5.0

        fun nowUs() = System.nanoTime() / 1_000
    }
}

/**
 * The recovery ladder (section 14.8): each stall in an incident takes the next step, once; ten quiet seconds end it.
 * Pure, so it is tested without a network.
 */
class Reconnector(private val now: () -> Long = System::currentTimeMillis) {
    enum class Step { RESTART_SOURCE, RECREATE_MEDIA, REHANDSHAKE, SWITCH_LINK, HOLD }

    private var lastStall = 0L
    private var next = 0

    /** The step for a new stall. After [Step.HOLD] it keeps answering [Step.HOLD]. */
    fun stall(): Step {
        val t = now()
        if (next == 0 || t - lastStall > QUIET_MS) next = 0
        lastStall = t
        val step = Step.entries[minOf(next, Step.entries.lastIndex)]
        next++
        return step
    }

    /** Media is flowing again. */
    fun recovered() {
        next = 0
    }

    companion object {
        const val QUIET_MS = 10_000L
    }
}
