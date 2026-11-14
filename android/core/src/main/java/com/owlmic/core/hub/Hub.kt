package com.owlmic.core.hub

import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/** The one dispatcher every hub runs on: hubs never race each other (section 12.3). */
object HubDispatcher {
    @OptIn(ExperimentalCoroutinesApi::class)
    val default: CoroutineDispatcher = Dispatchers.Default.limitedParallelism(1)
}

/** What a hub reports to its parent once a second (section 11.2). */
sealed interface Health {
    data object Ok : Health

    data class Degraded(val reason: String) : Health

    data class Failed(val reason: String) : Health
}

/**
 * A hub: one mailbox, drained on [dispatcher], and the supervised unit of section 11.3. When the mailbox holds
 * [capacity] messages, a [droppable] one (a stale report, a repeated answer) gives way; lifecycle and decision
 * messages are never dropped. An exception in [handle] goes to [onFailure], then the hub [restart]s after 100 ms,
 * 500 ms or 2 s; past 5 restarts a minute it stays as it is and reports [Health.Failed].
 */
abstract class Hub<M>(
    val name: String,
    private val capacity: Int = 64,
    dispatcher: CoroutineDispatcher = HubDispatcher.default,
    now: () -> Long = System::currentTimeMillis,
) {
    protected val scope = CoroutineScope(SupervisorJob() + dispatcher)
    private val mailbox = ArrayDeque<M>()
    private val wake = Channel<Unit>(Channel.CONFLATED)
    private val restarts = RestartPolicy(now)
    private var restartPending = false
    private var closed = false

    @Volatile private var health: Health = Health.Ok

    init {
        scope.launch {
            for (signal in wake) drain()
        }
    }

    /** Any thread. Never blocks. */
    fun post(message: M) {
        synchronized(mailbox) {
            if (closed) return
            if (mailbox.size >= capacity) {
                val stale = mailbox.indexOfFirst(::droppable)
                when {
                    stale >= 0 -> mailbox.removeAt(stale)
                    droppable(message) -> return
                }
            }
            mailbox.addLast(message)
        }
        wake.trySend(Unit)
    }

    /** Any thread. */
    fun health(): Health = health

    protected abstract fun handle(message: M)

    /** Messages that may be lost when the mailbox is full. */
    protected open fun droppable(message: M) = false

    protected open fun onFailure(message: M, error: Throwable) = Unit

    /** Brings the hub back to a known state after a failure. */
    protected open fun restart() = Unit

    /** Runs [block] on this hub after [delayMs], guarded like a message. */
    protected fun later(delayMs: Long, block: () -> Unit) {
        scope.launch {
            delay(delayMs)
            guarded(block)
        }
    }

    private fun drain() {
        while (true) {
            val message = synchronized(mailbox) { mailbox.removeFirstOrNull() } ?: return
            try {
                handle(message)
            } catch (e: Throwable) {
                runCatching { onFailure(message, e) }
                failed(e)
            }
        }
    }

    private fun guarded(block: () -> Unit) {
        try {
            block()
        } catch (e: Throwable) {
            failed(e)
        }
    }

    private fun failed(error: Throwable) {
        val reason = error.message ?: error.javaClass.simpleName
        if (restartPending) return
        val delayMs = restarts.nextDelayMs()
        if (delayMs == null) {
            health = Health.Failed(reason)
            return
        }
        health = Health.Degraded(reason)
        restartPending = true
        scope.launch {
            delay(delayMs)
            restartPending = false
            guarded {
                restart()
                if (!restartPending) health = Health.Ok
            }
        }
    }

    open fun close() {
        synchronized(mailbox) {
            closed = true
            mailbox.clear()
        }
        wake.close()
        scope.cancel()
    }
}

/** The worst of [all]: one failed hub fails the app, one degraded hub degrades it. */
fun worst(all: List<Health>): Health = all.firstOrNull { it is Health.Failed } ?: all.firstOrNull { it is Health.Degraded } ?: Health.Ok
