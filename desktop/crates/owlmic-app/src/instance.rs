//! One Owlmic per user session (SYSTEM_DESIGN section 13.1): a second copy couldn't bind the
//! ports, so it opens the running panel instead.

use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::w;

/// True when Owlmic already runs in this session. The mutex is never closed, so it marks this
/// process until it exits.
pub fn already_running() -> bool {
    unsafe {
        CreateMutexW(None, false, w!("Local\\OwlmicTray")).is_ok()
            && GetLastError() == ERROR_ALREADY_EXISTS
    }
}
