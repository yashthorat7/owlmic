//! Owlmic Cam on Windows 10 (SYSTEM_DESIGN section 13.1): the softcam DirectShow filter, which
//! the installer puts next to owlmic.exe. owlmic.exe registers it system-wide when run with
//! `--register-camera`, and feeds it pictures while running.

use owlmic_vcam::shared::{HEIGHT, WIDTH};
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE, REG_SZ, RRF_RT_REG_SZ, RegCloseKey,
    RegGetValueW, RegOpenKeyExW, RegSetValueExW,
};
use windows::core::{HSTRING, PCSTR, s, w};

const CLSID: &str = "{AEF3B972-5FA5-4647-9571-358EB472BC9E}";
/// Where DirectShow lists video capture filters; softcam registers its instance here.
const INSTANCE: &str = "Software\\Classes\\CLSID\\{860BB310-5D01-11D0-BD3B-00A0C911CE86}\\Instance\\DirectShow Softcam";
const FPS: f32 = 30.0;

type Symbol = unsafe extern "system" fn() -> isize;
type Create = unsafe extern "C" fn(i32, i32, f32) -> *mut c_void;
type Send = unsafe extern "C" fn(*mut c_void, *const u8);
type Delete = unsafe extern "C" fn(*mut c_void);

pub fn dll_path() -> Option<PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.join("softcam.dll"))
}

struct Library(HMODULE);

impl Library {
    fn load(path: &Path) -> Option<Self> {
        unsafe { LoadLibraryW(&HSTRING::from(path)).ok().map(Library) }
    }

    fn symbol(&self, name: PCSTR) -> Option<Symbol> {
        unsafe { GetProcAddress(self.0, name) }
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        unsafe {
            let _ = FreeLibrary(self.0);
        }
    }
}

fn call_registration(path: &Path, name: PCSTR) -> bool {
    let Some(lib) = Library::load(path) else {
        return false;
    };
    let Some(f) = lib.symbol(name) else {
        return false;
    };
    let f = unsafe { std::mem::transmute::<Symbol, unsafe extern "system" fn() -> i32>(f) };
    unsafe { f() >= 0 }
}

/// Registers the filter for every user and names it Owlmic Cam. Needs administrator rights.
pub fn register(path: &Path) -> bool {
    if !call_registration(path, s!("DllRegisterServer")) {
        return false;
    }
    let mut key = HKEY::default();
    unsafe {
        if RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            &HSTRING::from(INSTANCE),
            None,
            KEY_WRITE,
            &mut key,
        )
        .is_err()
        {
            return false;
        }
        let name: Vec<u16> = owlmic_ui::names::CAM_DEVICE
            .encode_utf16()
            .chain([0])
            .collect();
        let bytes = std::slice::from_raw_parts(name.as_ptr().cast::<u8>(), name.len() * 2);
        let ok = RegSetValueExW(key, w!("FriendlyName"), None, REG_SZ, Some(bytes)).is_ok();
        let _ = RegCloseKey(key);
        ok
    }
}

pub fn unregister(path: &Path) -> bool {
    call_registration(path, s!("DllUnregisterServer"))
}

/// Registered system-wide, pointing at a file that exists.
pub fn is_registered() -> bool {
    let mut buf = [0u16; 520];
    let mut len = (buf.len() * 2) as u32;
    let key = HSTRING::from(format!("Software\\Classes\\CLSID\\{CLSID}\\InprocServer32"));
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
    let path = String::from_utf16_lossy(&buf[..(len as usize / 2).saturating_sub(1)]);
    let mut instance = HKEY::default();
    let listed = unsafe {
        RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            &HSTRING::from(INSTANCE),
            None,
            KEY_READ,
            &mut instance,
        )
        .is_ok()
    };
    if listed {
        unsafe {
            let _ = RegCloseKey(instance);
        }
    }
    found && listed && Path::new(path.trim()).exists()
}

/// The running camera: created once, fed BGR pictures of the fixed size.
pub struct Softcam {
    _lib: Library,
    create: Create,
    send: Send,
    delete: Delete,
    camera: Mutex<usize>,
}

// softcam's functions are thread-safe; the camera pointer is only used under the lock.
unsafe impl std::marker::Send for Softcam {}
unsafe impl Sync for Softcam {}

impl Softcam {
    pub fn load() -> Option<Self> {
        let lib = Library::load(&dll_path()?)?;
        unsafe {
            let create = std::mem::transmute::<Symbol, Create>(lib.symbol(s!("scCreateCamera"))?);
            let send = std::mem::transmute::<Symbol, Send>(lib.symbol(s!("scSendFrame"))?);
            let delete = std::mem::transmute::<Symbol, Delete>(lib.symbol(s!("scDeleteCamera"))?);
            Some(Self {
                _lib: lib,
                create,
                send,
                delete,
                camera: Mutex::new(0),
            })
        }
    }

    /// One WIDTH x HEIGHT BGR picture. softcam paces sends to its frame rate, so this can wait
    /// up to a frame.
    pub fn send(&self, bgr: &[u8]) {
        if bgr.len() != WIDTH * HEIGHT * 3 {
            return;
        }
        let mut camera = self.camera.lock().unwrap_or_else(|p| p.into_inner());
        if *camera == 0 {
            *camera = unsafe { (self.create)(WIDTH as i32, HEIGHT as i32, FPS) } as usize;
        }
        if *camera != 0 {
            unsafe { (self.send)(*camera as *mut c_void, bgr.as_ptr()) };
        }
    }
}

impl Drop for Softcam {
    fn drop(&mut self) {
        let camera = *self.camera.lock().unwrap_or_else(|p| p.into_inner());
        if camera != 0 {
            unsafe { (self.delete)(camera as *mut c_void) };
        }
    }
}
