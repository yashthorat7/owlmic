package com.owlmic.core.link

import com.owlmic.core.hub.LinkKind
import java.net.Inet4Address
import java.net.InetAddress
import java.net.NetworkInterface

/** An interface we can probe on: our IPv4 address there and its broadcast address. */
data class NetIf(val name: String, val address: InetAddress, val prefixLength: Int, val broadcast: InetAddress) {
    val kind: LinkKind = linkFor(name)

    /** The phone is the access point: the PC joined the phone's hotspot. */
    val hotspot: Boolean = name.startsWith("ap") || name.startsWith("swlan")

    fun contains(other: InetAddress): Boolean = inSubnet(other.address, address.address, prefixLength)
}

/** USB tethering interfaces are rndis*, usb*, ncm*, ecm* or tether*. Everything else with a broadcast address is Wi-Fi, hotspots included. */
fun linkFor(interfaceName: String): LinkKind =
    if (
        interfaceName.startsWith("rndis") ||
        interfaceName.startsWith("usb") ||
        interfaceName.startsWith("ncm") ||
        interfaceName.startsWith("ecm") ||
        interfaceName.startsWith("tether")
    ) {
        LinkKind.USB_TETHERING
    } else {
        LinkKind.WIFI
    }

/** Interfaces that are up, not loopback, with an IPv4 address and a broadcast address. Mobile data has none, so it is never probed. */
fun probeInterfaces(): List<NetIf> = runCatching {
    NetworkInterface.getNetworkInterfaces()?.toList().orEmpty()
        .filter { it.isUp && !it.isLoopback }
        .flatMap { nic ->
            nic.interfaceAddresses.mapNotNull { ia ->
                val broadcast = ia.broadcast ?: return@mapNotNull null
                if (ia.address !is Inet4Address) return@mapNotNull null
                NetIf(nic.name, ia.address, ia.networkPrefixLength.toInt(), broadcast)
            }
        }
}.getOrDefault(emptyList())

fun inSubnet(ip: ByteArray, network: ByteArray, prefixLength: Int): Boolean {
    if (ip.size != network.size) return false
    var bits = prefixLength
    for (i in ip.indices) {
        if (bits <= 0) return true
        val mask = if (bits >= 8) 0xFF else (0xFF shl (8 - bits)) and 0xFF
        if ((ip[i].toInt() and mask) != (network[i].toInt() and mask)) return false
        bits -= 8
    }
    return true
}

/**
 * USB tethering is the everyday cable (section 14.1), and Android needs it switched on by hand: offer the shortcut when
 * a cable to a computer is in, tethering is allowed, and no tethering interface is up.
 */
fun tetherHint(usbPlugged: Boolean, tetheringAllowed: Boolean, interfaces: List<NetIf>): Boolean =
    usbPlugged && tetheringAllowed && interfaces.none { it.kind == LinkKind.USB_TETHERING }
