//! Repair (SYSTEM_DESIGN section 19): one elevated PowerShell run, so one UAC prompt, fixes
//! whichever of Owlmic Mic, Owlmic Cam and the firewall rules is broken, using the same steps
//! as the installer.

use crate::DeviceHealth;
use base64::Engine;

/// The PowerShell script for the parts `health` marks broken. `dir` holds owlmic.exe and the
/// installer's setup-audio-device.ps1.
pub fn script(dir: &str, exe: &str, health: DeviceHealth) -> String {
    let quote = |s: &str| format!("'{}'", s.replace('\'', "''"));
    let mut lines = vec!["$ErrorActionPreference = 'Continue'".to_owned()];
    if !health.mic {
        lines.push(format!(
            "& {} -Silent",
            quote(&format!("{dir}\\setup-audio-device.ps1"))
        ));
    }
    if !health.cam {
        // owlmic.exe is a windowed program, so PowerShell would not wait for it without -Wait.
        lines.push(format!("Start-Process -FilePath {} -ArgumentList '--register-camera' -Wait -WindowStyle Hidden", quote(exe)));
    }
    if !health.net {
        lines.extend(
            crate::firewall::commands(exe)
                .into_iter()
                .map(|c| format!("{c} | Out-Null")),
        );
    }
    lines.join("\n")
}

/// `-EncodedCommand` takes base64 of UTF-16LE, which avoids every quoting problem.
pub fn encode(script: &str) -> String {
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Runs `script` elevated and waits for it. False if the user declined the prompt.
#[cfg(windows)]
pub fn run(script: &str) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{INFINITE, WaitForSingleObject};
    use windows::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
    use windows::core::{HSTRING, PCWSTR, w};

    let params = HSTRING::from(format!(
        "-NoProfile -NonInteractive -ExecutionPolicy Bypass -WindowStyle Hidden -EncodedCommand {}",
        encode(script)
    ));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: w!("runas"),
        lpFile: w!("powershell.exe"),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    unsafe {
        if ShellExecuteExW(&mut info).is_err() {
            return false;
        }
        if !info.hProcess.is_invalid() {
            WaitForSingleObject(info.hProcess, INFINITE);
            let _ = CloseHandle(info.hProcess);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_script_fixes_only_what_is_broken() {
        let all = DeviceHealth {
            mic: false,
            cam: false,
            net: false,
        };
        let s = script(
            "C:\\Program Files\\Owlmic",
            "C:\\Program Files\\Owlmic\\owlmic.exe",
            all,
        );
        assert!(s.contains("& 'C:\\Program Files\\Owlmic\\setup-audio-device.ps1' -Silent"));
        assert!(s.contains("--register-camera"));
        assert!(s.contains("name=\"Owlmic UDP Media\""));
        let cam_only = script(
            "D",
            "D\\owlmic.exe",
            DeviceHealth {
                cam: false,
                ..DeviceHealth::default()
            },
        );
        assert!(!cam_only.contains("setup-audio-device") && !cam_only.contains("netsh"));
    }

    #[test]
    fn quotes_in_paths_are_escaped() {
        let s = script(
            "C:\\O'Brien",
            "C:\\O'Brien\\owlmic.exe",
            DeviceHealth {
                mic: false,
                ..DeviceHealth::default()
            },
        );
        assert!(s.contains("'C:\\O''Brien\\setup-audio-device.ps1'"));
    }

    #[test]
    fn encoding_is_base64_of_utf16le() {
        assert_eq!(encode("ab"), "YQBiAA==");
    }
}
