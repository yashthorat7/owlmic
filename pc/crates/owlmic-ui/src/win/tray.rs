//! The tray icon: always the same owl, white on dark taskbars and black on light ones; the state
//! lives only in the tooltip. A click opens the panel; there is no menu.

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS,
    DeleteObject, HBITMAP,
};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::UI::HiDpi::GetSystemMetricsForDpi;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NIM_SETVERSION,
    NOTIFYICON_VERSION_4, NOTIFYICONDATAW, NOTIFYICONIDENTIFIER, Shell_NotifyIconGetRect,
    Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, DestroyIcon, HICON, ICONINFO, SM_CXSMICON,
};
use windows::core::w;

pub const CALLBACK: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 2;
const ID: u32 = 1;

pub struct Tray {
    hwnd: HWND,
    icon: HICON,
    white: bool,
    tip: String,
}

/// Light taskbars get the black owl. Without the setting (older Windows 10) the taskbar is dark.
fn taskbar_is_light() -> bool {
    let (mut value, mut size) = (0u32, 4u32);
    let found = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        )
    };
    found.is_ok() && value == 1
}

fn owl_icon(white: bool, dpi: u32) -> HICON {
    let size = unsafe { GetSystemMetricsForDpi(SM_CXSMICON, dpi) }.max(16) as usize;
    let pixels = crate::owl::bgra(size, white);
    unsafe {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size as i32,
                biHeight: -(size as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let Ok(color) = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0) else {
            return HICON::default();
        };
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits as *mut u8, pixels.len());
        let mask: HBITMAP = CreateBitmap(size as i32, size as i32, 1, 1, None);
        let icon = CreateIconIndirect(&ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        });
        let _ = DeleteObject(color.into());
        let _ = DeleteObject(mask.into());
        icon.unwrap_or_default()
    }
}

impl Tray {
    pub fn new(hwnd: HWND, dpi: u32) -> Self {
        let white = !taskbar_is_light();
        let mut tray = Self {
            hwnd,
            icon: owl_icon(white, dpi),
            white,
            tip: String::new(),
        };
        tray.add();
        tray
    }

    fn data(&self) -> NOTIFYICONDATAW {
        let mut d = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: ID,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP,
            uCallbackMessage: CALLBACK,
            hIcon: self.icon,
            ..Default::default()
        };
        for (dst, src) in d.szTip.iter_mut().zip(self.tip.encode_utf16().take(127)) {
            *dst = src;
        }
        d.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        d
    }

    /// Also after Explorer restarts, which forgets every tray icon.
    pub fn add(&mut self) {
        let d = self.data();
        unsafe {
            let _ = Shell_NotifyIconW(NIM_ADD, &d);
            let _ = Shell_NotifyIconW(NIM_SETVERSION, &d);
        }
    }

    pub fn set_tip(&mut self, tip: &str) {
        if self.tip != tip {
            self.tip = tip.to_owned();
            unsafe {
                let _ = Shell_NotifyIconW(NIM_MODIFY, &self.data());
            }
        }
    }

    /// The taskbar switched between light and dark.
    pub fn theme_changed(&mut self, dpi: u32) {
        let white = !taskbar_is_light();
        if white != self.white {
            self.white = white;
            let old = std::mem::replace(&mut self.icon, owl_icon(white, dpi));
            unsafe {
                let _ = Shell_NotifyIconW(NIM_MODIFY, &self.data());
                let _ = DestroyIcon(old);
            }
        }
    }

    /// Where the icon sits on screen.
    pub fn rect(&self) -> Option<RECT> {
        let id = NOTIFYICONIDENTIFIER {
            cbSize: size_of::<NOTIFYICONIDENTIFIER>() as u32,
            hWnd: self.hwnd,
            uID: ID,
            ..Default::default()
        };
        unsafe { Shell_NotifyIconGetRect(&id).ok() }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.data());
            let _ = DestroyIcon(self.icon);
        }
    }
}
