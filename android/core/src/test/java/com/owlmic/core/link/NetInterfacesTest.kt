package com.owlmic.core.link

import com.owlmic.core.hub.LinkKind
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.net.InetAddress

class NetInterfacesTest {
    private fun nic(name: String) = NetIf(name, InetAddress.getByName("192.168.42.10"), 24, InetAddress.getByName("192.168.42.255"))

    @Test
    fun interfacesAreToldApartByName() {
        assertEquals(LinkKind.USB_TETHERING, nic("rndis0").kind)
        assertEquals(LinkKind.USB_TETHERING, nic("ncm0").kind)
        assertEquals(LinkKind.USB_TETHERING, nic("ecm0").kind)
        assertEquals(LinkKind.USB_TETHERING, nic("tether0").kind)
        assertEquals(LinkKind.WIFI, nic("wlan0").kind)
        assertTrue(nic("ap0").hotspot)
    }

    @Test
    fun theTetheringHintNeedsACableAndNoTetheringYet() {
        assertTrue(tetherHint(usbPlugged = true, tetheringAllowed = true, interfaces = listOf(nic("wlan0"))))
        assertFalse(tetherHint(usbPlugged = true, tetheringAllowed = true, interfaces = listOf(nic("rndis0"))))
        assertFalse(tetherHint(usbPlugged = false, tetheringAllowed = true, interfaces = emptyList()))
        assertFalse(tetherHint(usbPlugged = true, tetheringAllowed = false, interfaces = emptyList()))
    }
}
