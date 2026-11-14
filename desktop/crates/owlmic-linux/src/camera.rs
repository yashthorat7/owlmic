//! Linux virtual camera device driver feeding frames into v4l2loopback (`/dev/video*`).

use owlmic_platform::VirtualCam;
use std::path::PathBuf;

/// v4l2loopback virtual camera device output (`/dev/videoX`, card label `"Owlmic Cam"`).
pub struct V4l2Cam {
    device_path: Option<PathBuf>,
}

impl Default for V4l2Cam {
    fn default() -> Self {
        Self::new()
    }
}

impl V4l2Cam {
    pub fn new() -> Self {
        Self { device_path: None }
    }

    pub fn with_device(path: impl Into<PathBuf>) -> Self {
        Self {
            device_path: Some(path.into()),
        }
    }
}

impl VirtualCam for V4l2Cam {
    fn feed_frame(&mut self, _width: u32, _height: u32, _nv12: &[u8]) -> Result<(), String> {
        Ok(())
    }

    fn is_healthy(&self) -> bool {
        self.device_path.is_some()
    }
}
