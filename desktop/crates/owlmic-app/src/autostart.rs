//! Start with Windows (SYSTEM_DESIGN section 24.2, step 7): the per-user Run value the installer
//! also writes, kept in step with the setting.

use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_SZ, RRF_RT_REG_SZ, RegCloseKey,
    RegDeleteValueW, RegGetValueW, RegOpenKeyExW, RegSetValueExW,
};
use windows::core::w;

const RUN: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE: windows::core::PCWSTR = w!("Owlmic");

pub fn command(exe: &std::path::Path) -> String {
    format!("\"{}\" --autostart", exe.display())
}

fn current() -> Option<String> {
    let mut buf = [0u16; 1024];
    let mut len = (buf.len() * 2) as u32;
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            RUN,
            VALUE,
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut len),
        )
        .ok()
        .ok()?
    };
    Some(String::from_utf16_lossy(
        &buf[..(len as usize / 2).saturating_sub(1)],
    ))
}

/// Writes or removes the Run value; touches the registry only when it differs.
pub fn sync(on: bool) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let want = on.then(|| command(&exe));
    if current() == want {
        return;
    }
    let mut key = HKEY::default();
    unsafe {
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            RUN,
            None,
            KEY_SET_VALUE | KEY_READ,
            &mut key,
        )
        .is_err()
        {
            return;
        }
        match want {
            Some(cmd) => {
                let data: Vec<u16> = cmd.encode_utf16().chain([0]).collect();
                let bytes = std::slice::from_raw_parts(data.as_ptr().cast::<u8>(), data.len() * 2);
                let _ = RegSetValueExW(key, VALUE, None, REG_SZ, Some(bytes));
            }
            None => {
                let _ = RegDeleteValueW(key, VALUE);
            }
        }
        let _ = RegCloseKey(key);
    }
}
