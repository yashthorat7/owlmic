package com.owlmic.media.audio

import java.util.concurrent.TimeUnit
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock

/**
 * The speaker's jitter buffer (section 17.3): holds just enough audio to ride out the network's timing. The target is
 * the 95th percentile of arrival jitter plus 10 ms, kept within [minMs, maxMs]. It fills before it plays, conceals
 * losses (with the next packet for Opus FEC), and skips only on severe runaway backlog while the drift resampler
 * handles clock mismatch smoothly.
 *
 * The playback thread shares its lock with the network thread, so nothing under it allocates or sorts: packets sit in
 * a ring indexed by seq, and the percentile comes from a running histogram.
 */
class Smoother(private val frameMs: Int, private val minMs: Int, private val maxMs: Int, private val now: () -> Long = System::currentTimeMillis) {
    sealed interface Pull {
        class Frame(val data: ByteArray) : Pull

        /** The frame for this slot is missing; [next] is the following packet, for FEC, when it has arrived. */
        class Lost(val next: ByteArray?) : Pull

        data object Silence : Pull
    }

    private val lock = ReentrantLock()
    private val ready = lock.newCondition()
    private val slots = arrayOfNulls<ByteArray>(CAPACITY)
    private val tags = LongArray(CAPACITY) { NONE }
    private var count = 0
    private var highest = -1L
    private var next = -1L
    private var started = false
    private var playing = false
    private var misses = 0
    private var lastArrival = 0L

    private val jitter = IntArray(WINDOW)
    private var jitterCount = 0
    private var jitterAt = 0
    private val histogram = IntArray(HISTOGRAM_MS)

    var targetMs = minMs
        private set

    fun push(seq32: Long, data: ByteArray) = lock.withLock {
        val seq = unwrap(seq32)
        val t = now()
        if (lastArrival > 0) {
            addJitter(kotlin.math.abs((t - lastArrival).toInt() - frameMs))
            targetMs = (percentile95() + 10).coerceIn(minMs, maxMs)
        }
        lastArrival = t
        if (started) {
            if (seq < next) return
            // Too far ahead to fit: what lies before it is too late to wait for.
            if (seq - next >= CAPACITY) {
                next = seq - CAPACITY + 1
                for (i in tags.indices) {
                    if (tags[i] != NONE && tags[i] < next) {
                        slots[i] = null
                        tags[i] = NONE
                        count--
                    }
                }
            }
        }
        val i = index(seq)
        val held = tags[i]
        if (held != NONE) {
            // A newer packet already has the slot, or this one is a repeat.
            if (held >= seq) return
            count--
        }
        slots[i] = data
        tags[i] = seq
        count++
        if (!playing && count * frameMs >= targetMs) {
            ready.signalAll()
        }
    }

    fun pull(): Pull = lock.withLock {
        if (!playing) {
            if (count * frameMs < targetMs) return Pull.Silence
            playing = true
            misses = 0
            next = if (!started) {
                started = true
                lowest()
            } else {
                maxOf(next, lowest())
            }
        }
        // Only hard-skip on severe backlog past maxMs; normal drift is handled smoothly by the resampler.
        val hardCapMs = maxOf(maxMs, targetMs + CATCH_UP_FRAMES * frameMs)
        if (count * frameMs > hardCapMs && take(next) != null) next++
        take(next)?.let {
            next++
            misses = 0
            return Pull.Frame(it)
        }
        if (count == 0) {
            if (++misses > MAX_CONCEALED) {
                playing = false
                return Pull.Silence
            }
            next++
            return Pull.Lost(null)
        }
        val following = peek(next + 1)
        next++
        return Pull.Lost(following)
    }

    fun awaitReady(timeoutMs: Long): Boolean = lock.withLock {
        if (playing || count * frameMs >= targetMs) return true
        try {
            ready.await(timeoutMs, TimeUnit.MILLISECONDS)
        } catch (_: InterruptedException) {
        }
        playing || count * frameMs >= targetMs
    }

    fun wakeup() = lock.withLock {
        ready.signalAll()
    }

    fun buffered() = lock.withLock { count }

    fun bufferedMs() = lock.withLock { count * frameMs }

    private fun index(seq: Long) = (seq and (CAPACITY - 1).toLong()).toInt()

    private fun take(seq: Long): ByteArray? {
        val i = index(seq)
        if (tags[i] != seq) return null
        val data = slots[i]
        slots[i] = null
        tags[i] = NONE
        count--
        return data
    }

    private fun peek(seq: Long): ByteArray? = index(seq).let { if (tags[it] == seq) slots[it] else null }

    private fun lowest(): Long {
        var min = Long.MAX_VALUE
        for (tag in tags) if (tag != NONE && tag < min) min = tag
        return min
    }

    private fun addJitter(ms: Int) {
        val bucket = ms.coerceIn(0, HISTOGRAM_MS - 1)
        if (jitterCount == WINDOW) histogram[jitter[jitterAt]]-- else jitterCount++
        jitter[jitterAt] = bucket
        histogram[bucket]++
        jitterAt = (jitterAt + 1) % WINDOW
    }

    private fun percentile95(): Int {
        val rank = (jitterCount * 95 / 100).coerceAtMost(jitterCount - 1)
        var seen = 0
        for (ms in histogram.indices) {
            seen += histogram[ms]
            if (seen > rank) return ms
        }
        return HISTOGRAM_MS - 1
    }

    /** 32-bit seq numbers grow without bound here, so wrapping never reorders anything. */
    private fun unwrap(seq32: Long): Long {
        if (highest < 0) {
            highest = seq32
            return seq32
        }
        val base = highest and 0xFFFFFFFFL.inv()
        var seq = base or seq32
        if (seq - highest > HALF) seq -= 1L shl 32 else if (highest - seq > HALF) seq += 1L shl 32
        if (seq > highest) highest = seq
        return seq
    }

    private companion object {
        const val WINDOW = 50

        /** A power of two above the 100 frames the old buffer held, so seq maps to a slot with a mask. */
        const val CAPACITY = 128
        const val NONE = Long.MIN_VALUE

        /** Jitter is counted per millisecond up to here; anything longer is past [maxMs] anyway. */
        const val HISTOGRAM_MS = 128
        const val MAX_CONCEALED = 3
        const val CATCH_UP_FRAMES = 3
        const val HALF = 1L shl 31
    }
}
