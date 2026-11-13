//! The Device Hub (SYSTEM_DESIGN sections 13.2 and 17): Owlmic Mic, Owlmic Cam, the speaker's
//! loopback capture, Quiet PC speakers, device health and Repair.

pub mod firewall;
pub mod repair;

#[cfg(windows)]
mod audio;
#[cfg(windows)]
pub mod camera;
#[cfg(windows)]
pub mod decoder;
#[cfg(windows)]
pub mod dpapi;
#[cfg(windows)]
pub mod health;
#[cfg(windows)]
mod hub;
#[cfg(windows)]
pub mod loopback;
#[cfg(windows)]
pub mod quiet;
#[cfg(windows)]
pub mod render;
#[cfg(windows)]
pub mod softcam;

#[cfg(windows)]
pub use hub::{DeviceEvent, DeviceHub, DeviceMsg, Shared};

/// What the Repair tile reports (SYSTEM_DESIGN section 19): each part is fine or needs a repair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceHealth {
    pub mic: bool,
    pub cam: bool,
    pub net: bool,
}

impl Default for DeviceHealth {
    /// Assumed fine until the first check says otherwise, so the panel never flashes a repair.
    fn default() -> Self {
        Self {
            mic: true,
            cam: true,
            net: true,
        }
    }
}

impl DeviceHealth {
    pub fn all_ok(&self) -> bool {
        self.mic && self.cam && self.net
    }
}
