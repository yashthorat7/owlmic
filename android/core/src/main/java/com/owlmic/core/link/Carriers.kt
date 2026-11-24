package com.owlmic.core.link

import android.net.Network
import com.owlmic.core.proto.Frames
import java.io.Closeable
import java.io.DataInputStream
import java.io.IOException
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.Socket
import java.net.SocketTimeoutException
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.TimeUnit
import kotlin.concurrent.thread

/** Moves media packets over one link. [send] never blocks the caller for long: stream carriers queue and drop the oldest. */
interface MediaCarrier : Closeable {
    fun send(packet: ByteArray)
}

/** UDP 7655 on USB tethering and Wi-Fi. [network] is the Wi-Fi network when the PC is on it. */
class UdpCarrier(host: InetAddress, port: Int, network: Network?, private val onReceive: (ByteArray, Int) -> Unit) : MediaCarrier {
    private val socket = DatagramSocket().apply {
        sendBufferSize = 512 * 1024
        receiveBufferSize = 256 * 1024
        network?.bindSocket(this)
        connect(host, port)
        soTimeout = 1_000
    }

    @Volatile private var open = true

    init {
        thread(name = "owlmic-receive", isDaemon = true) {
            val buf = ByteArray(2_048)
            val p = DatagramPacket(buf, buf.size)
            while (open) {
                try {
                    p.length = buf.size
                    socket.receive(p)
                    onReceive(buf, p.length)
                } catch (e: SocketTimeoutException) {
                    continue
                } catch (e: IOException) {
                    if (!open) break
                }
            }
        }
    }

    override fun send(packet: ByteArray) {
        try {
            socket.send(DatagramPacket(packet, packet.size))
        } catch (_: IOException) {
            // A full buffer or a vanished route: media is best effort, the monitor notices a dead link.
        }
    }

    override fun close() {
        open = false
        socket.close()
    }
}

/**
 * A queue and a writer thread in front of a blocking stream, so a stalled link never blocks an encoder. When it is
 * full the oldest packet goes: fresh media is worth more than old.
 */
private class WriterQueue(name: String, private val write: (ByteArray) -> Unit, private val onError: () -> Unit) {
    private val queue = ArrayBlockingQueue<ByteArray>(512)

    @Volatile var open = true

    init {
        thread(name = name, isDaemon = true) {
            while (open) {
                val packet = queue.poll(500, TimeUnit.MILLISECONDS) ?: continue
                try {
                    write(packet)
                } catch (e: IOException) {
                    open = false
                    onError()
                }
            }
        }
    }

    fun offer(packet: ByteArray) {
        while (!queue.offer(packet)) queue.poll()
    }
}

/** The media channel through the adb tunnel: a second connection to the control port, marked with channel byte 2. */
class TunnelCarrier(port: Int, private val onReceive: (ByteArray, Int) -> Unit, onClosed: () -> Unit) : MediaCarrier {
    private val socket = Socket().apply {
        tcpNoDelay = true
        sendBufferSize = 512 * 1024
        receiveBufferSize = 512 * 1024
        connect(InetSocketAddress(InetAddress.getLoopbackAddress(), port), 2_000)
        soTimeout = 5_000
    }
    private val output = socket.getOutputStream()
    private val writer = WriterQueue("owlmic-tunnel-out", { packet ->
        output.write(byteArrayOf(Frames.MEDIA_MARKER.toByte(), (packet.size shr 8).toByte(), packet.size.toByte()))
        output.write(packet)
        output.flush()
    }, onClosed)

    init {
        output.write(Frames.CHANNEL_MEDIA)
        output.flush()
        val input = DataInputStream(socket.getInputStream())
        thread(name = "owlmic-tunnel-in", isDaemon = true) {
            while (writer.open) {
                try {
                    if (input.readUnsignedByte() != Frames.MEDIA_MARKER) throw IOException("not a media packet")
                    val packet = ByteArray(input.readUnsignedShort()).also { input.readFully(it) }
                    onReceive(packet, packet.size)
                } catch (e: SocketTimeoutException) {
                    continue
                } catch (e: IOException) {
                    if (writer.open) {
                        writer.open = false
                        onClosed()
                    }
                }
            }
        }
    }

    override fun send(packet: ByteArray) = writer.offer(packet)

    override fun close() {
        writer.open = false
        runCatching { socket.close() }
    }
}

/** Bluetooth: media shares the RFCOMM stream with control (protocol section 3). Incoming media arrives through the control reader. */
class SharedStreamCarrier(private val channel: ControlChannel, onClosed: () -> Unit) : MediaCarrier {
    private val writer = WriterQueue("owlmic-bt-out", { channel.sendMedia(it, it.size) }, onClosed)

    override fun send(packet: ByteArray) = writer.offer(packet)

    override fun close() {
        writer.open = false
    }
}
