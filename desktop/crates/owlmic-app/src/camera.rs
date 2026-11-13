//! `--register-camera` and `--unregister-camera`, run elevated by the installer and by Repair:
//! Windows 11 gets the Media Foundation virtual camera (owlmic_vcam.dll), Windows 10 the
//! softcam filter (softcam.dll), both from owlmic.exe's folder.

use std::path::PathBuf;

fn beside_exe(name: &str) -> Option<PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.join(name))
}

pub fn register() -> bool {
    if owlmic_vcam::register::is_windows_11() {
        beside_exe("owlmic_vcam.dll")
            .is_some_and(|dll| dll.exists() && owlmic_vcam::register::register(&dll).is_ok())
    } else {
        owlmic_devices::softcam::dll_path()
            .is_some_and(|dll| dll.exists() && owlmic_devices::softcam::register(&dll))
    }
}

/// Removes whichever camera is registered; a PC upgraded from Windows 10 may have both.
pub fn unregister() -> bool {
    let mut ok = true;
    if owlmic_vcam::register::is_windows_11() {
        ok &= owlmic_vcam::register::unregister().is_ok();
    }
    if let Some(dll) = owlmic_devices::softcam::dll_path().filter(|d| d.exists()) {
        ok &= owlmic_devices::softcam::unregister(&dll);
    }
    ok
}
