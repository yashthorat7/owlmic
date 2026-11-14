package com.owlmic.core.hub

import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

@OptIn(ExperimentalCoroutinesApi::class)
class HubTest {
    /** Negative messages throw; even ones are droppable. */
    private class Recorder(
        capacity: Int = 64,
        dispatcher: CoroutineDispatcher = HubDispatcher.default,
        now: () -> Long = System::currentTimeMillis,
        private val done: CountDownLatch? = null,
    ) : Hub<Int>("test", capacity, dispatcher, now) {
        val seen = mutableListOf<Int>()
        var failures = 0
        val restartedAt = mutableListOf<Long>()
        private val clock = now

        override fun handle(message: Int) {
            if (message < 0) error("bad")
            seen += message
            done?.countDown()
        }

        override fun droppable(message: Int) = message % 2 == 0

        override fun onFailure(message: Int, error: Throwable) {
            failures++
        }

        override fun restart() {
            restartedAt += clock()
        }
    }

    @Test
    fun handlesInOrderAndSurvivesAFailingMessage() {
        val done = CountDownLatch(3)
        val hub = Recorder(done = done)
        hub.post(1)
        hub.post(-1)
        hub.post(2)
        hub.post(3)
        assertTrue(done.await(2, TimeUnit.SECONDS))
        assertEquals(listOf(1, 2, 3), hub.seen)
        assertEquals(1, hub.failures)
        hub.close()
    }

    @Test
    fun aFullMailboxDropsStaleMessagesButNeverLifecycleOnes() {
        val dispatcher = StandardTestDispatcher()
        val hub = Recorder(capacity = 3, dispatcher = dispatcher)
        // Odd numbers stand for lifecycle messages (a closed link, a finished attempt).
        listOf(1, 2, 3, 4, 5, 7, 6, 9).forEach(hub::post)
        dispatcher.scheduler.runCurrent()
        assertEquals(listOf(1, 3, 5, 7, 9), hub.seen)
        hub.close()
    }

    @Test
    fun aFailingHubRestartsWithBackoffThenReportsFailed() {
        val dispatcher = StandardTestDispatcher()
        val clock = { dispatcher.scheduler.currentTime }
        val hub = Recorder(dispatcher = dispatcher, now = clock)
        hub.post(-1)
        dispatcher.scheduler.runCurrent()
        assertTrue(hub.health() is Health.Degraded)
        dispatcher.scheduler.advanceTimeBy(101)
        dispatcher.scheduler.runCurrent()
        assertEquals(listOf(100L), hub.restartedAt)
        assertEquals(Health.Ok, hub.health())

        repeat(5) {
            hub.post(-1)
            dispatcher.scheduler.runCurrent()
            dispatcher.scheduler.advanceTimeBy(2_001)
            dispatcher.scheduler.runCurrent()
        }
        // Five restarts in the minute: the sixth failure leaves the hub failed.
        assertEquals(5, hub.restartedAt.size)
        assertTrue(hub.health() is Health.Failed)
        // It still handles messages; it just isn't restarted any more.
        hub.post(1)
        dispatcher.scheduler.runCurrent()
        assertEquals(listOf(1), hub.seen)
        hub.close()
    }

    @Test
    fun theWorstHealthWins() {
        assertEquals(Health.Ok, worst(listOf(Health.Ok, Health.Ok)))
        assertEquals(Health.Degraded("x"), worst(listOf(Health.Ok, Health.Degraded("x"))))
        assertEquals(Health.Failed("y"), worst(listOf(Health.Degraded("x"), Health.Failed("y"))))
    }
}
