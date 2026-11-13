//! Windows system tray and Direct2D flyout coordinator.

use owlmic_platform::TrayUi;

/// Windows desktop system tray coordinator using Direct2D and Shell_NotifyIcon.
pub struct WindowsTray {
    running: bool,
}

impl Default for WindowsTray {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowsTray {
    pub fn new() -> Self {
        Self { running: false }
    }
}

impl TrayUi for WindowsTray {
    fn run(&mut self) -> Result<(), String> {
        self.running = true;
        Ok(())
    }

    fn update_status(&mut self, _link_kind: u8, _mic_active: bool, _cam_active: bool) {}
}
