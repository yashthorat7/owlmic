//! Owlmic host service and CLI entrypoint for Windows.

use owlmic_windows::{WasapiLoopback, WasapiMic, WindowsCam, WindowsTray};

/// Runs the Owlmic host service on Windows.
pub fn run(show_panel: bool) {
    let mut _mic = WasapiMic::new();
    let mut _speaker = WasapiLoopback::new();
    let mut _cam = WindowsCam::new();
    let mut _tray = WindowsTray::new();

    crate::start::run(show_panel);
}
