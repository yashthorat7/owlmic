//! The Transporter, PC side (SYSTEM_DESIGN section 14): discovery, handshake, carriers,
//! monitoring, switching and reconnection.

pub mod answerer;
pub mod handshake;
pub mod hub;
pub mod media;
pub mod monitor;
pub mod netinfo;
pub mod ratelimit;
pub mod reconnect;
pub mod server;
pub mod switcher;
pub mod udp;
pub mod wire;

#[cfg(test)]
mod loopback_tests;

#[cfg(windows)]
pub mod adb;
#[cfg(windows)]
pub mod bt;

pub use hub::{BtAddr, LinkEvent, LinkHub, LinkMsg, LinkView};
pub use media::{MediaSink, Routes};

/// Link kinds, by priority (SYSTEM_DESIGN section 14.1). Lower is better.
pub const LINK_USB_DEBUGGING: u8 = 1;
pub const LINK_USB_TETHERING: u8 = 2;
pub const LINK_WIFI: u8 = 3;
pub const LINK_BLUETOOTH: u8 = 4;

/// Cable links are private, so only Wi-Fi and Bluetooth are encrypted.
pub fn is_wireless(link: u8) -> bool {
    matches!(link, LINK_WIFI | LINK_BLUETOOTH)
}
