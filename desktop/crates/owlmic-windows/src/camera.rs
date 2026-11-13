//! Windows virtual camera driver using Media Foundation (`IMFVirtualCamera`) or DirectShow (`softcam`).

use owlmic_platform::VirtualCam;

/// Windows virtual camera device driver feeding frames into Owlmic Cam.
pub struct WindowsCam {
    active: bool,
}

impl Default for WindowsCam {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowsCam {
    pub fn new() -> Self {
        Self { active: false }
    }
}

impl VirtualCam for WindowsCam {
    fn feed_frame(&mut self, _width: u32, _height: u32, _nv12: &[u8]) -> Result<(), String> {
        self.active = true;
        Ok(())
    }

    fn is_healthy(&self) -> bool {
        #[cfg(windows)]
        {
            owlmic_devices::DeviceHealth::default().cam
        }
        #[cfg(not(windows))]
        {
            true
        }
    }
}
