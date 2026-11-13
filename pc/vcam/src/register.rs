//! Registering Owlmic Cam (run by `owlmic.exe --register-camera`, elevated): the COM class under
//! HKLM, then a Media Foundation virtual camera for all users that outlives this process.

use std::path::Path;
use windows::Win32::Media::MediaFoundation::{
    IMFAsyncCallback, IMFVirtualCamera, MF_VERSION, MFCreateVirtualCamera, MFSTARTUP_FULL,
    MFShutdown, MFStartup, MFVirtualCameraAccess_AllUsers, MFVirtualCameraLifetime_System,
    MFVirtualCameraType_SoftwareCameraSource,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RRF_RT_REG_SZ,
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegGetValueW, RegSetValueExW,
};
use windows::core::{GUID, HSTRING, PCWSTR, Result};

fn clsid_string() -> String {
    format!("{{{:?}}}", GUID::from_u128(crate::CLSID))
}

fn class_key() -> String {
    format!("Software\\Classes\\CLSID\\{}", clsid_string())
}

/// Windows 11 is build 22000 and later; only it has Media Foundation virtual cameras.
pub fn is_windows_11() -> bool {
    let mut buf = [0u16; 16];
    let mut len = (buf.len() * 2) as u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            &HSTRING::from("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion"),
            &HSTRING::from("CurrentBuildNumber"),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut len),
        )
    };
    if r.is_err() {
        return false;
    }
    let text = String::from_utf16_lossy(&buf[..(len as usize / 2).saturating_sub(1)]);
    text.trim().parse::<u32>().is_ok_and(|b| b >= 22_000)
}

fn set_string(key: HKEY, name: Option<&str>, value: &str) -> Result<()> {
    let data: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = unsafe { std::slice::from_raw_parts(data.as_ptr().cast::<u8>(), data.len() * 2) };
    let name = name.map(HSTRING::from);
    unsafe {
        RegSetValueExW(
            key,
            name.as_ref().map_or(PCWSTR::null(), |n| PCWSTR(n.as_ptr())),
            None,
            REG_SZ,
            Some(bytes),
        )
        .ok()
    }
}

fn create_key(path: &str) -> Result<HKEY> {
    let mut key = HKEY::default();
    unsafe {
        RegCreateKeyExW(
            HKEY_LOCAL_MACHINE,
            &HSTRING::from(path),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut key,
            None,
        )
        .ok()?;
    }
    Ok(key)
}

fn virtual_camera() -> Result<IMFVirtualCamera> {
    unsafe {
        MFCreateVirtualCamera(
            MFVirtualCameraType_SoftwareCameraSource,
            MFVirtualCameraLifetime_System,
            MFVirtualCameraAccess_AllUsers,
            &HSTRING::from(owlmic_ui::names::CAM_DEVICE),
            &HSTRING::from(clsid_string()),
            None,
        )
    }
}

/// The COM class for `dll` under HKLM, as `DllRegisterServer` and `--register-camera` write it.
pub fn register_class(dll: &Path) -> Result<()> {
    let class = create_key(&class_key())?;
    set_string(class, None, owlmic_ui::names::CAM_DEVICE)?;
    unsafe {
        let _ = RegCloseKey(class);
    }
    let server = create_key(&format!("{}\\InprocServer32", class_key()))?;
    set_string(server, None, &dll.to_string_lossy())?;
    set_string(server, Some("ThreadingModel"), "Both")?;
    unsafe {
        let _ = RegCloseKey(server);
    }
    Ok(())
}

pub fn unregister_class() -> Result<()> {
    unsafe { RegDeleteTreeW(HKEY_LOCAL_MACHINE, &HSTRING::from(class_key())).ok() }
}

/// Registers `dll` (owlmic_vcam.dll) and creates the camera for all users.
pub fn register(dll: &Path) -> Result<()> {
    register_class(dll)?;
    unsafe {
        MFStartup(MF_VERSION, MFSTARTUP_FULL)?;
    }
    let result = virtual_camera().and_then(|cam| unsafe {
        cam.Start(None::<&IMFAsyncCallback>)?;
        cam.Shutdown()
    });
    unsafe {
        let _ = MFShutdown();
    }
    result
}

/// Removes the camera and its class.
pub fn unregister() -> Result<()> {
    unsafe {
        MFStartup(MF_VERSION, MFSTARTUP_FULL)?;
    }
    let removed = virtual_camera().and_then(|cam| unsafe { cam.Remove() });
    unsafe {
        let _ = MFShutdown();
    }
    let _ = unregister_class();
    removed
}

/// Whether the class is registered with a DLL that exists, for the health check behind the
/// Repair tile.
pub fn is_registered() -> bool {
    let mut buf = [0u16; 520];
    let mut len = (buf.len() * 2) as u32;
    let key = HSTRING::from(format!("{}\\InprocServer32", class_key()));
    let found = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            &key,
            None,
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut len),
        )
        .is_ok()
    };
    found
        && Path::new(String::from_utf16_lossy(&buf[..(len as usize / 2).saturating_sub(1)]).trim())
            .exists()
}
