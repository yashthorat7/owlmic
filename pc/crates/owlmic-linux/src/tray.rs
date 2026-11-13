//! Linux system tray and notification coordinator.

use owlmic_platform::TrayUi;

/// Linux desktop system tray implementation (StatusNotifierItem / X11 / Wayland).
pub struct LinuxTray {
    running: bool,
}

impl Default for LinuxTray {
    fn default() -> Self {
        Self::new()
    }
}

impl LinuxTray {
    pub fn new() -> Self {
        Self { running: false }
    }
}

impl TrayUi for LinuxTray {
    fn run(&mut self) -> Result<(), String> {
        self.running = true;
        Ok(())
    }

    fn update_status(&mut self, _link_kind: u8, _mic_active: bool, _cam_active: bool) {}
}
