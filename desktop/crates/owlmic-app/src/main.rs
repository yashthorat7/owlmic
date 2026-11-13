#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);
    // The installer runs these elevated, possibly while another Owlmic runs: no single-instance
    // check and no tray.
    if has("--register-camera") {
        std::process::exit(if owlmic_app::camera::register() { 0 } else { 1 });
    }
    if has("--unregister-camera") {
        std::process::exit(if owlmic_app::camera::unregister() {
            0
        } else {
            1
        });
    }
    // The installer runs this before replacing files and on uninstall: the running Owlmic
    // quits as if Quit were clicked, so the PC speakers are unmuted and the tray icon goes.
    if has("--quit") {
        let gone = owlmic_ui::win::quit_running(std::time::Duration::from_secs(5));
        std::process::exit(if gone { 0 } else { 1 });
    }
    if owlmic_app::instance::already_running() {
        owlmic_ui::win::open_running();
        return;
    }
    owlmic_app::windows::run(!has("--autostart"));
}

#[cfg(target_os = "linux")]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let daemon = args.iter().any(|a| a == "--daemon");
    if let Err(e) = owlmic_app::linux::run(daemon) {
        eprintln!("Owlmic Linux error: {e}");
        std::process::exit(1);
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
fn main() {
    eprintln!("Unsupported operating system.");
    std::process::exit(1);
}
