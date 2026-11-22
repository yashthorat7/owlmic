//! The tray icon: always the same owl, white on dark taskbars and black on light ones; the state
//! lives only in the tooltip. A click opens the panel; there is no menu.

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateCompatibleDC, CreateDIBSection,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, HBITMAP, SelectObject,
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
use windows_numerics::Vector2;

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

        let hdc = CreateCompatibleDC(None);
        let old_bmp = SelectObject(hdc, color.into());
        if let Ok(d2d) = D2D1CreateFactory::<ID2D1Factory>(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)
        {
            let props = D2D1_RENDER_TARGET_PROPERTIES {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                ..Default::default()
            };
            if let Ok(target) = d2d.CreateDCRenderTarget(&props) {
                let rc = RECT {
                    left: 0,
                    top: 0,
                    right: size as i32,
                    bottom: size as i32,
                };
                if target.BindDC(hdc, &rc).is_ok() {
                    target.BeginDraw();
                    target.Clear(Some(&D2D1_COLOR_F {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.0,
                    }));
                    let color_rgb = if white {
                        D2D1_COLOR_F {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 1.0,
                        }
                    } else {
                        D2D1_COLOR_F {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }
                    };
                    if let Ok(brush) = target.CreateSolidColorBrush(&color_rgb, None) {
                        let k = size as f32 / 24.0;
                        let p = |x: f32, y: f32| Vector2 { X: x * k, Y: y * k };
                        if let Ok(geom) = d2d.CreatePathGeometry()
                            && let Ok(sink) = geom.Open()
                        {
                            sink.SetFillMode(D2D1_FILL_MODE_ALTERNATE);
                            let mut open = false;
                            for seg in crate::brand::OWL_BODY {
                                match *seg {
                                    crate::glyph::Seg::Move(x, y) => {
                                        if open {
                                            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                                        }
                                        sink.BeginFigure(p(x, y), D2D1_FIGURE_BEGIN_FILLED);
                                        open = true;
                                    }
                                    crate::glyph::Seg::Line(x, y) => sink.AddLine(p(x, y)),
                                    crate::glyph::Seg::Cubic(c) => {
                                        sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                                            point1: p(c[0], c[1]),
                                            point2: p(c[2], c[3]),
                                            point3: p(c[4], c[5]),
                                        })
                                    }
                                    crate::glyph::Seg::Close if open => {
                                        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                                        open = false;
                                    }
                                    _ => {}
                                }
                            }
                            if open {
                                sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                            }
                            let _ = sink.Close();
                            target.FillGeometry(&geom, &brush, None);
                        }
                        for shape in crate::brand::OWL_PUPILS {
                            if let crate::glyph::Shape::Circle { c, r } = *shape {
                                target.FillEllipse(
                                    &D2D1_ELLIPSE {
                                        point: p(c.0, c.1),
                                        radiusX: r * k,
                                        radiusY: r * k,
                                    },
                                    &brush,
                                );
                            }
                        }
                    }
                    let _ = target.EndDraw(None, None);
                }
            }
        }
        SelectObject(hdc, old_bmp);
        let _ = DeleteDC(hdc);

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
