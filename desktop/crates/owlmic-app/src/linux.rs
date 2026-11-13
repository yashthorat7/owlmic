//! Owlmic host service and CLI entrypoint for Linux.

use owlmic_linux::{LinuxTray, PipeWireLoopback, PipeWireMic, TrayUi, V4l2Cam};

/// Runs the Owlmic host service on Linux.
///
/// When `daemon` is true, runs headlessly in the background (e.g. under systemd).
/// Otherwise, starts the desktop system tray coordinator.
pub fn run(daemon: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("Owlmic starting on Linux (daemon: {daemon})...");

    let mut _mic = PipeWireMic::new();
    let mut _speaker = PipeWireLoopback::new();
    let mut _cam = V4l2Cam::new();

    if daemon {
        println!("Owlmic running in daemon mode (systemd). Press Ctrl+C to stop.");
        let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let r = running.clone();
        ctrlc_handler(move || {
            r.store(false, std::sync::atomic::Ordering::SeqCst);
        });
        while running.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        println!("Owlmic daemon stopped.");
    } else {
        println!("Owlmic running with system tray...");
        let mut tray = LinuxTray::new();
        if let Err(e) = tray.run() {
            eprintln!("Tray error: {e}");
        }
    }

    Ok(())
}

fn ctrlc_handler<F: FnOnce() + Send + 'static>(_f: F) {
    // Placeholder signal handler hook for Linux daemon lifecycle
}
