//! Owlmic Cam on Windows 11 (SYSTEM_DESIGN section 24.2, step 5): a Media Foundation media
//! source that the Windows camera service loads for every app, serving the pictures owlmic.exe
//! writes into a shared frame ring, or a placeholder when there are none.

pub mod shared;

#[cfg(windows)]
mod activate;
#[cfg(windows)]
pub mod placeholder;
#[cfg(windows)]
pub mod register;
#[cfg(windows)]
mod source;
#[cfg(windows)]
mod stream;

/// The media source's class id, registered under HKLM and given to `MFCreateVirtualCamera`.
pub const CLSID: u128 = 0x7c1e9a3f_5b2d_4e8a_9f41_0c6d2b8e5a17;

#[cfg(windows)]
mod exports {
    use crate::activate::Factory;
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicIsize, Ordering};
    use windows::Win32::Foundation::{
        CLASS_E_CLASSNOTAVAILABLE, E_FAIL, E_POINTER, HMODULE, S_FALSE, S_OK,
    };
    use windows::Win32::System::Com::IClassFactory;
    use windows::Win32::System::LibraryLoader::{
        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
        GetModuleFileNameW, GetModuleHandleExW,
    };
    use windows::core::{GUID, HRESULT, Interface, PCWSTR};

    /// Objects alive in this DLL; the camera service unloads it only at zero.
    pub(crate) static LIVE: AtomicIsize = AtomicIsize::new(0);

    #[unsafe(no_mangle)]
    extern "system" fn DllGetClassObject(
        clsid: *const GUID,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if out.is_null() || clsid.is_null() || iid.is_null() {
            return E_POINTER;
        }
        unsafe { *out = std::ptr::null_mut() };
        if unsafe { *clsid } != GUID::from_u128(crate::CLSID) {
            return CLASS_E_CLASSNOTAVAILABLE;
        }
        let factory: IClassFactory = Factory.into();
        unsafe { factory.query(iid, out) }
    }

    #[unsafe(no_mangle)]
    extern "system" fn DllCanUnloadNow() -> HRESULT {
        if LIVE.load(Ordering::Acquire) == 0 {
            S_OK
        } else {
            S_FALSE
        }
    }

    /// This DLL's own path, found from one of its functions.
    fn own_path() -> Option<std::path::PathBuf> {
        let mut module = HMODULE::default();
        let mut buf = [0u16; 1024];
        unsafe {
            GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
                    | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                PCWSTR(own_path as *const u16),
                &mut module,
            )
            .ok()?;
            let n = GetModuleFileNameW(Some(module), &mut buf) as usize;
            (n > 0).then(|| String::from_utf16_lossy(&buf[..n]).into())
        }
    }

    /// For regsvr32: the COM class only. The camera itself is created by
    /// `owlmic.exe --register-camera`.
    #[unsafe(no_mangle)]
    extern "system" fn DllRegisterServer() -> HRESULT {
        match own_path().map(|p| crate::register::register_class(&p)) {
            Some(Ok(())) => S_OK,
            Some(Err(e)) => e.code(),
            None => E_FAIL,
        }
    }

    #[unsafe(no_mangle)]
    extern "system" fn DllUnregisterServer() -> HRESULT {
        crate::register::unregister_class().map_or_else(|e| e.code(), |()| S_OK)
    }
}
