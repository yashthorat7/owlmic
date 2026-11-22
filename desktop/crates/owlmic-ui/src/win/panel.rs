//! The panel window: 312 x 374 at 100% scale above the tray, never in the taskbar, closed by
//! Esc or by clicking elsewhere (SYSTEM_DESIGN section 33). It shows the newest state snapshot
//! the App Hub published, at most 60 times a second.

use super::draw::{Align, Font, Painter};
use super::tray::{self, Tray};
use crate::layout::{self as l, Drawer, HelpScreen, Rect, Screen, Target};
use crate::preview::{self, Preview};
use crate::view::{Action, Connection, Feature, Link, PanelState, indicator};
use crate::{color, icons};
use owlmic_hub::Publisher;
use std::sync::Arc;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWM_WINDOW_CORNER_PREFERENCE, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, GetMonitorInfoW, InvalidateRect, MONITOR_DEFAULTTONEAREST, MONITORINFO,
    MonitorFromPoint, PAINTSTRUCT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
use windows::Win32::UI::Shell::{NIN_SELECT, ShellExecuteW};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, PCWSTR, Result, w};

const CLASS: PCWSTR = w!("OwlmicPanel");
const WM_APP_STATE: u32 = WM_APP + 1;
const WM_APP_OPEN: u32 = WM_APP + 3;
const TIMER_PREVIEW: usize = 1;
const TIMER_STATE: usize = 2;
const FRAME: Duration = Duration::from_millis(16);
/// A tray click that closed the panel by taking its focus shouldn't reopen it.
const REOPEN_GUARD: Duration = Duration::from_millis(300);
/// NIN_SELECT with the keyboard flag: the icon chosen with Enter or Space.
const NIN_KEYSELECT: u32 = NIN_SELECT | 1;
const PAD: f32 = 12.0;
const ICON: f32 = crate::icon::PC_PX;

/// Lets other threads wake the panel. Usable before the window exists.
#[derive(Clone, Default)]
pub struct Waker(Arc<AtomicIsize>);

impl Waker {
    fn post(&self, msg: u32) {
        let h = self.0.load(Ordering::Acquire);
        if h != 0 {
            unsafe {
                let _ = PostMessageW(Some(HWND(h as _)), msg, WPARAM(0), LPARAM(0));
            }
        }
    }

    /// A new snapshot is waiting.
    pub fn wake(&self) {
        self.post(WM_APP_STATE);
    }

    /// Closes the panel and ends the message loop.
    pub fn quit(&self) {
        self.post(WM_CLOSE);
    }
}

/// For a second launch: opens the running Owlmic's panel. False if none runs.
pub fn open_running() -> bool {
    unsafe { FindWindowW(CLASS, None) }
        .is_ok_and(|h| unsafe { PostMessageW(Some(h), WM_APP_OPEN, WPARAM(0), LPARAM(0)) }.is_ok())
}

/// For `owlmic.exe --quit`: asks the running Owlmic to quit as Quit does, then waits up to
/// `wait` for its process to end. True once none runs.
pub fn quit_running(wait: Duration) -> bool {
    use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
    };
    let Ok(hwnd) = (unsafe { FindWindowW(CLASS, None) }) else {
        return true;
    };
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    let Ok(process) = (unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) }) else {
        return false;
    };
    let gone = unsafe {
        PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)).is_ok()
            && WaitForSingleObject(process, wait.as_millis() as u32) == WAIT_OBJECT_0
    };
    unsafe {
        let _ = CloseHandle(process);
    }
    gone
}

pub struct Ui {
    pub state: Publisher<PanelState>,
    pub preview: Arc<Preview>,
    pub on_action: Box<dyn Fn(Action)>,
}

struct Panel {
    hwnd: HWND,
    ui: Ui,
    painter: Painter,
    tray: Option<Tray>,
    state: Arc<PanelState>,
    drawer: Drawer,
    cam_menu_open: bool,
    quick_audio_open: bool,
    scale: f32,
    visible: bool,
    hidden_at: Option<Instant>,
    applied_at: Instant,
    copied_at: Option<Instant>,
    taskbar_created: u32,
    anim_frame: u64,
}

/// Creates the panel and the tray icon and runs the message loop until [`Waker::quit`].
/// `open` shows the panel once at start (a launch by hand, not at sign-in).
pub fn run(waker: &Waker, ui: Ui, open: bool) -> Result<()> {
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            lpszClassName: CLASS,
            ..Default::default()
        };
        RegisterClassExW(&class);
        let panel = Box::new(Panel {
            hwnd: HWND::default(),
            state: ui.state.read(),
            ui,
            painter: Painter::new()?,
            tray: None,
            drawer: Drawer::None,
            cam_menu_open: false,
            quick_audio_open: false,
            scale: 1.0,
            visible: false,
            hidden_at: None,
            applied_at: Instant::now(),
            copied_at: None,
            taskbar_created: RegisterWindowMessageW(w!("TaskbarCreated")),
            anim_frame: 0,
        });
        let raw = Box::into_raw(panel);
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            CLASS,
            &HSTRING::from(crate::names::PRODUCT),
            WS_POPUP,
            0,
            0,
            l::WIDTH as i32,
            l::HEIGHT as i32,
            None,
            None,
            Some(instance.into()),
            Some(raw as _),
        )?;
        let panel = &mut *raw;
        panel.hwnd = hwnd;
        eprintln!("HWND created: {:?}", hwnd);
        // Windows 11 rounds the corners and draws a hairline border; Windows 10 ignores both.
        let corners = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&corners as *const DWM_WINDOW_CORNER_PREFERENCE).cast(),
            size_of_val(&corners) as u32,
        );
        let border = crate::color::HAIRLINE.swap_bytes() >> 8;
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_BORDER_COLOR, (&border as *const u32).cast(), 4);
        panel.tray = Some(Tray::new(hwnd, GetDpiForWindow(hwnd)));
        waker.0.store(hwnd.0 as isize, Ordering::Release);
        panel.apply();
        if open {
            panel.show(true);
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        waker.0.store(0, Ordering::Release);
        drop(Box::from_raw(raw));
    }
    Ok(())
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_NCCREATE {
            let create = &*(lparam.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        let panel = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Panel;
        if !panel.is_null()
            && (*panel).hwnd == hwnd
            && let Some(r) = (*panel).message(msg, wparam, lparam)
        {
            return r;
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

fn lo(v: isize) -> i32 {
    (v & 0xFFFF) as i16 as i32
}

fn hi(v: isize) -> i32 {
    ((v >> 16) & 0xFFFF) as i16 as i32
}

impl Panel {
    fn message(&mut self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        if msg == WM_APP_OPEN || msg == WM_ACTIVATE || msg == WM_SHOWWINDOW {
            eprintln!("Panel message: {:#x}, visible: {}", msg, self.visible);
        }
        match msg {
            WM_PAINT => self.paint(),
            WM_APP_STATE => {
                let wait = FRAME.saturating_sub(self.applied_at.elapsed());
                if wait.is_zero() {
                    self.apply();
                } else {
                    unsafe {
                        SetTimer(
                            Some(self.hwnd),
                            TIMER_STATE,
                            wait.as_millis() as u32 + 1,
                            None,
                        )
                    };
                }
            }
            WM_TIMER if wparam.0 == TIMER_STATE => {
                unsafe {
                    let _ = KillTimer(Some(self.hwnd), TIMER_STATE);
                }
                self.apply();
            }
            WM_TIMER if wparam.0 == TIMER_PREVIEW => {
                self.anim_frame = self.anim_frame.wrapping_add(1);
                if self.visible {
                    self.invalidate();
                }
            }
            WM_APP_OPEN => self.show(true),
            tray::CALLBACK => {
                let event = (lparam.0 & 0xFFFF) as u32;
                if [
                    WM_LBUTTONUP,
                    WM_RBUTTONUP,
                    WM_CONTEXTMENU,
                    NIN_SELECT,
                    NIN_KEYSELECT,
                ]
                .contains(&event)
                {
                    if self.visible {
                        self.hide();
                    } else if self.hidden_at.is_none_or(|t| t.elapsed() > REOPEN_GUARD) {
                        self.show(true);
                    }
                }
            }
            WM_ACTIVATE if (wparam.0 & 0xFFFF) as u32 == WA_INACTIVE && self.visible => {
                if std::env::var("OWLMIC_NO_AUTO_HIDE").is_err() {
                    self.hide();
                }
            }
            WM_KEYDOWN if wparam.0 as u16 == VK_ESCAPE.0 => self.hide(),
            WM_LBUTTONUP => self.click(
                lo(lparam.0) as f32 / self.scale,
                hi(lparam.0) as f32 / self.scale,
            ),
            WM_DPICHANGED => {
                if self.visible {
                    self.show(false);
                }
            }
            WM_SETTINGCHANGE => {
                let dpi = unsafe { GetDpiForWindow(self.hwnd) };
                if let Some(t) = self.tray.as_mut() {
                    t.theme_changed(dpi);
                }
            }
            WM_CLOSE => unsafe {
                self.tray = None;
                let _ = DestroyWindow(self.hwnd);
            },
            WM_DESTROY => unsafe { PostQuitMessage(0) },
            m if m == self.taskbar_created && m != 0 => {
                if let Some(t) = self.tray.as_mut() {
                    t.add();
                }
            }
            _ => return None,
        }
        Some(LRESULT(0))
    }

    /// Takes the newest snapshot. A new approval request opens the panel without taking focus
    /// from a call (SYSTEM_DESIGN section 35).
    fn apply(&mut self) {
        self.applied_at = Instant::now();
        let new = self.ui.state.read();
        let asks = |s: &PanelState| matches!(s.connection, Connection::Approval { .. });
        let open_pairing = asks(&new) && !asks(&self.state);
        self.state = new;
        let tip = self.state.tooltip();
        if let Some(t) = self.tray.as_mut() {
            t.set_tip(&tip);
        }
        if open_pairing {
            self.drawer = Drawer::Wifi;
            if !self.visible {
                self.show(false);
            }
        }
        self.invalidate();
    }

    fn invalidate(&self) {
        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }

    fn show(&mut self, activate: bool) {
        let anchor = self.tray.as_ref().and_then(Tray::rect).unwrap_or_else(|| {
            let mut p = POINT::default();
            unsafe {
                let _ = GetCursorPos(&mut p);
            }
            RECT {
                left: p.x,
                top: p.y,
                right: p.x,
                bottom: p.y,
            }
        });
        let centre = POINT {
            x: (anchor.left + anchor.right) / 2,
            y: (anchor.top + anchor.bottom) / 2,
        };
        let monitor = unsafe { MonitorFromPoint(centre, MONITOR_DEFAULTTONEAREST) };
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let (mut dpi, mut dpi_y) = (96, 96);
        unsafe {
            let _ = GetMonitorInfoW(monitor, &mut info);
            let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut dpi_y);
        }
        self.scale = dpi as f32 / 96.0;
        let size = (
            (l::WIDTH * self.scale).round() as i32,
            (l::HEIGHT * self.scale).round() as i32,
        );
        let w = info.rcWork;
        let (x, y) = l::place(
            (anchor.left, anchor.top, anchor.right, anchor.bottom),
            (w.left, w.top, w.right, w.bottom),
            size,
            (PAD * self.scale) as i32,
        );
        let flags = if activate {
            SWP_SHOWWINDOW
        } else {
            SWP_SHOWWINDOW | SWP_NOACTIVATE
        };
        unsafe {
            let _ = SetWindowPos(self.hwnd, Some(HWND_TOPMOST), x, y, size.0, size.1, flags);
            if activate {
                let _ = SetForegroundWindow(self.hwnd);
            }
            SetTimer(
                Some(self.hwnd),
                TIMER_PREVIEW,
                preview::EVERY_MS as u32,
                None,
            );
        }
        self.visible = true;
        self.ui.preview.set_wanted(true);
        self.invalidate();
    }

    fn hide(&mut self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
            let _ = KillTimer(Some(self.hwnd), TIMER_PREVIEW);
        }
        self.visible = false;
        self.drawer = Drawer::None;
        self.cam_menu_open = false;
        self.quick_audio_open = false;
        self.hidden_at = Some(Instant::now());
        self.ui.preview.set_wanted(false);
    }

    fn click(&mut self, x: f32, y: f32) {
        let screen =
            self.state
                .active_screen(self.drawer, self.cam_menu_open, self.quick_audio_open);
        let Some(target) = l::hit(screen, x, y) else {
            return;
        };
        match target {
            Target::Wifi => {
                if self.drawer == Drawer::Wifi {
                    self.drawer = Drawer::None;
                } else {
                    self.drawer = Drawer::Wifi;
                    self.cam_menu_open = false;
                    self.quick_audio_open = false;
                }
            }
            Target::Help => {
                if matches!(self.drawer, Drawer::Help(_)) {
                    self.drawer = Drawer::None;
                } else {
                    self.drawer = Drawer::Help(HelpScreen::Bento);
                    self.cam_menu_open = false;
                    self.quick_audio_open = false;
                }
            }
            Target::Camera | Target::Mic | Target::Speaker => {
                self.drawer = Drawer::None;
                if let Some(action) = self.state.click(target) {
                    (self.ui.on_action)(action);
                }
            }
            Target::HelpBack => {
                self.drawer = Drawer::Help(HelpScreen::Bento);
            }
            Target::HelpHowTo => {
                self.drawer = Drawer::Help(HelpScreen::HowTo);
            }
            Target::HelpCredits => {
                self.drawer = Drawer::Help(HelpScreen::Credits);
            }
            Target::HelpPrivacy => {
                self.drawer = Drawer::Help(HelpScreen::Privacy);
            }
            Target::CamMenu => {
                self.cam_menu_open = !self.cam_menu_open;
            }
            Target::WaveDock => {
                self.quick_audio_open = !self.quick_audio_open;
            }
            Target::CopyIp => {
                let host = if self.state.hostname.is_empty() {
                    "127.0.0.1"
                } else {
                    &self.state.hostname
                };
                let ip_str = format!("{host}:7653");
                copy_to_clipboard(self.hwnd, &ip_str);
                self.copied_at = Some(Instant::now());
            }
            _ => {
                if let Some(link) = self.state.link_at(target) {
                    open(link);
                } else if let Some(action) = self.state.click(target) {
                    (self.ui.on_action)(action);
                }
            }
        }
        self.invalidate();
    }

    fn paint(&mut self) {
        let mut ps = PAINTSTRUCT::default();
        unsafe { BeginPaint(self.hwnd, &mut ps) };
        if self.painter.begin(self.hwnd, self.scale) {
            self.painter.clear(color::HAIRLINE);
            let screen =
                self.state
                    .active_screen(self.drawer, self.cam_menu_open, self.quick_audio_open);
            match screen {
                Screen::DrawerWifi(mode) => {
                    self.draw_drawer_wifi(mode);
                    self.draw_bottom_dock();
                }
                Screen::DrawerHelp(h) => {
                    self.draw_drawer_help(h);
                    self.draw_bottom_dock();
                }
                Screen::Main {
                    cam_on,
                    mic_on,
                    cam_menu,
                    quick_audio,
                } => {
                    self.draw_main(cam_on, mic_on, cam_menu, quick_audio);
                    self.draw_bottom_dock();
                }
            }
            self.painter.end();
        }
        unsafe {
            let _ = EndPaint(self.hwnd, &ps);
        }
    }

    fn draw_main(&mut self, cam_on: bool, mic_on: bool, cam_menu: bool, quick_audio: bool) {
        let s = self.state.clone();
        let p = &mut self.painter;
        let c = l::PLAYGROUND;
        p.fill(c, color::BG);
        let (mid_x, mid_y) = (c.x + c.w / 2.0, c.y + c.h / 2.0);

        if cam_on {
            if self.ui.preview.seq() > 0 {
                let seq = self.ui.preview.seq();
                let preview = self.ui.preview.clone();
                p.picture(c, preview::WIDTH, preview::HEIGHT, seq, |copy| {
                    preview.read(|px| copy(px))
                });
            } else {
                p.fill(c, 0x111318);
                let k = 14.0;
                let b = c.inset(10.0);
                p.fill(Rect::new(b.x, b.y, k, 1.5), color::HAIRLINE);
                p.fill(Rect::new(b.x, b.y, 1.5, k), color::HAIRLINE);
                p.fill(Rect::new(b.right() - k, b.y, k, 1.5), color::HAIRLINE);
                p.fill(Rect::new(b.right() - 1.5, b.y, 1.5, k), color::HAIRLINE);
                p.fill(Rect::new(b.x, b.bottom() - 1.5, k, 1.5), color::HAIRLINE);
                p.fill(Rect::new(b.x, b.bottom() - k, 1.5, k), color::HAIRLINE);
                p.fill(
                    Rect::new(b.right() - k, b.bottom() - 1.5, k, 1.5),
                    color::HAIRLINE,
                );
                p.fill(
                    Rect::new(b.right() - 1.5, b.bottom() - k, 1.5, k),
                    color::HAIRLINE,
                );
            }

            // Camera format / HUD menu
            if cam_menu {
                p.fill_rounded(l::CAM_EXPAND_MENU, 8.0, 0x0A0A0A);
                p.stroke_rounded(l::CAM_EXPAND_MENU, 8.0, color::HAIRLINE, 1.0);

                let cur_q = s.setting("camera.quality");
                let cur_fps = s.setting("camera.frameRate");
                let cur_ratio = s.setting("camera.ratio");

                let row0 = ["FHD", "HD", "480p"];
                let row1 = ["60", "30", "24"];
                let row2 = ["16:9", "4:3", "1:1"];

                for (col, &val) in row0.iter().enumerate() {
                    let r = l::cam_option_rect(0, col);
                    let sel = cur_q == val || (cur_q.is_empty() && val == "FHD");
                    if sel {
                        p.fill(r, 0x222222);
                    }
                    p.text(
                        val,
                        r,
                        Font::Caption,
                        if sel { color::TEXT } else { color::TEXT2 },
                        Align::Center,
                        false,
                    );
                }
                for (col, &val) in row1.iter().enumerate() {
                    let r = l::cam_option_rect(1, col);
                    let sel = cur_fps == val || (cur_fps.is_empty() && val == "60");
                    if sel {
                        p.fill(r, 0x222222);
                    }
                    let lbl = format!("{val}fps");
                    p.text(
                        &lbl,
                        r,
                        Font::Caption,
                        if sel { color::TEXT } else { color::TEXT2 },
                        Align::Center,
                        false,
                    );
                }
                for (col, &val) in row2.iter().enumerate() {
                    let r = l::cam_option_rect(2, col);
                    let sel = cur_ratio == val || (cur_ratio.is_empty() && val == "16:9");
                    if sel {
                        p.fill(r, 0x222222);
                    }
                    p.text(
                        val,
                        r,
                        Font::Caption,
                        if sel { color::TEXT } else { color::TEXT2 },
                        Align::Center,
                        false,
                    );
                }
            } else {
                p.fill_rounded(l::CAM_MENU_BTN, 6.0, 0x0A0A0A);
                p.stroke_rounded(l::CAM_MENU_BTN, 6.0, color::HAIRLINE, 1.0);
                let q_lbl = match s.setting("camera.quality") {
                    "HD" => "HD",
                    "480p" => "480p",
                    _ => "FHD",
                };
                p.text(
                    q_lbl,
                    l::CAM_MENU_BTN,
                    Font::Caption,
                    color::TEXT,
                    Align::Center,
                    false,
                );
            }

            // Mirror & Flip buttons
            let mirror_on = s.setting("camera.mirror") == "true";
            p.fill_rounded(
                l::CAM_MIRROR_BTN,
                6.0,
                if mirror_on { 0x333333 } else { 0x0A0A0A },
            );
            p.stroke_rounded(l::CAM_MIRROR_BTN, 6.0, color::HAIRLINE, 1.0);
            p.icon(
                icons::MIRROR,
                l::CAM_MIRROR_BTN.x + 12.5,
                l::CAM_MIRROR_BTN.y + 12.5,
                13.0,
                if mirror_on {
                    color::ON_CONTENT
                } else {
                    color::TEXT
                },
            );

            p.fill_rounded(l::CAM_FLIP_BTN, 6.0, 0x0A0A0A);
            p.stroke_rounded(l::CAM_FLIP_BTN, 6.0, color::HAIRLINE, 1.0);
            p.icon(
                icons::FLIP,
                l::CAM_FLIP_BTN.x + 12.5,
                l::CAM_FLIP_BTN.y + 12.5,
                13.0,
                color::TEXT,
            );

            // Docked waveform if mic also streaming
            if mic_on {
                let dock = l::WAVE_DOCK_DOCKED;
                p.fill_rounded(dock, 11.0, 0x0A0A0A);
                p.stroke_rounded(dock, 11.0, color::HAIRLINE, 1.0);
                p.waveform_bars(dock.inset(3.0), 18, true, self.anim_frame);
            }
        } else if mic_on {
            p.watermark(mid_x, mid_y - 20.0, 140.0, 0.08);

            let dock = l::WAVE_DOCK_CENTERED;
            p.fill_rounded(dock, 12.0, 0x0A0A0A);
            p.stroke_rounded(dock, 12.0, color::HAIRLINE, 1.0);

            let bars_rect = Rect::new(dock.x + 16.0, dock.y + 12.0, dock.w - 32.0, 24.0);
            p.waveform_bars(bars_rect, 22, true, self.anim_frame);

            p.text(
                "Microphone streaming · 48 kHz Opus",
                Rect::new(dock.x + 8.0, dock.y + 40.0, dock.w - 16.0, 18.0),
                Font::Caption,
                color::TEXT2,
                Align::Center,
                false,
            );
        } else if s.speaker == Feature::On && s.connected() {
            p.watermark(mid_x, mid_y - 20.0, 140.0, 0.08);

            let ripple = (self.anim_frame % 60) as f32 / 60.0;
            p.circle(mid_x, mid_y - 15.0, 6.0, color::TEXT);
            p.stroke_circle(mid_x, mid_y - 15.0, 16.0 + ripple * 8.0, color::TEXT, 2.0);
            p.stroke_circle(mid_x, mid_y - 15.0, 32.0 + ripple * 8.0, color::TEXT2, 1.5);
            p.stroke_circle(
                mid_x,
                mid_y - 15.0,
                48.0 + ripple * 8.0,
                color::DISABLED,
                1.0,
            );

            p.text(
                "Speaker loopback active · WASAPI",
                Rect::new(c.x + PAD, mid_y + 48.0, c.w - 2.0 * PAD, 18.0),
                Font::Caption,
                color::TEXT2,
                Align::Center,
                false,
            );
        } else {
            p.dots_grid(c, 18.0, 1.0, 0x181818);
            p.watermark(mid_x, mid_y, 152.0, 0.07);
        }

        // Quick Audio panel (overlaying bottom of playground when open)
        if quick_audio && mic_on {
            let qp = l::AUDIO_QUICK_PANEL;
            p.fill(qp, 0x000000);
            p.line(0.0, qp.y, l::WIDTH, qp.y, color::HAIRLINE, 1.0);

            // Voice Isolation
            p.text(
                "Voice isolation",
                Rect::new(14.0, qp.y + 11.0, 180.0, 20.0),
                Font::Body,
                color::TEXT,
                Align::Left,
                false,
            );
            p.switch(l::AUDIO_SWITCH_ISO, s.setting("mic.isolation") != "false");

            // Normalization
            p.text(
                "Normalization",
                Rect::new(14.0, qp.y + 37.0, 180.0, 20.0),
                Font::Body,
                color::TEXT,
                Align::Left,
                false,
            );
            p.switch(l::AUDIO_SWITCH_NORM, s.setting("mic.autoLevel") != "false");

            // Noise Reduction
            let nr_str = s.setting("mic.noiseReduction");
            let nr_val = nr_str.parse::<f32>().unwrap_or(5.0);
            p.text(
                "Noise reduction",
                Rect::new(14.0, qp.y + 62.0, 120.0, 16.0),
                Font::Caption,
                color::TEXT2,
                Align::Left,
                false,
            );
            p.text(
                &format!("{:.0}", nr_val),
                Rect::new(l::WIDTH - 60.0, qp.y + 62.0, 46.0, 16.0),
                Font::Caption,
                color::TEXT2,
                Align::Right,
                false,
            );
            p.fader(l::AUDIO_SLIDER_NR, nr_val / 10.0, 0xFFFFFF, 0x222222);

            // Volume
            let vol_str = s.setting("mic.volume");
            let vol_val = vol_str.parse::<f32>().unwrap_or(80.0);
            p.text(
                "Volume",
                Rect::new(14.0, qp.y + 104.0, 120.0, 16.0),
                Font::Caption,
                color::TEXT2,
                Align::Left,
                false,
            );
            p.text(
                &format!("{:.0}%", vol_val),
                Rect::new(l::WIDTH - 60.0, qp.y + 104.0, 46.0, 16.0),
                Font::Caption,
                color::TEXT2,
                Align::Right,
                false,
            );
            p.fader(l::AUDIO_SLIDER_VOL, vol_val / 100.0, 0xFFFFFF, 0x222222);
        }
    }

    fn draw_bottom_dock(&mut self) {
        let s = self.state.clone();
        let p = &mut self.painter;
        let connected = s.connected();

        // b0: Wi-Fi / Connection Drawer toggle
        let b0 = l::bottom(0);
        let wifi_active = self.drawer == Drawer::Wifi;
        p.fill(b0, if wifi_active { color::ON } else { color::TILE });
        let i = indicator(&s.connection);
        let b0_color = if wifi_active {
            color::ON_CONTENT
        } else {
            i.color
        };
        p.icon(i.icon, b0.x + b0.w / 2.0, b0.y + b0.h / 2.0, ICON, b0_color);

        // b1: Camera stream toggle
        let b1 = l::bottom(1);
        let cam_on = s.camera == Feature::On && connected;
        p.fill(b1, if cam_on { color::ON } else { color::TILE });
        let cam_icon = if cam_on {
            icons::CAMERA
        } else {
            icons::CAMERA_OFF
        };
        let cam_fg = if cam_on {
            color::ON_CONTENT
        } else if connected {
            color::TEXT
        } else {
            color::DISABLED
        };
        p.icon(cam_icon, b1.x + b1.w / 2.0, b1.y + b1.h / 2.0, ICON, cam_fg);

        // b2: Microphone stream toggle
        let b2 = l::bottom(2);
        let mic_on = s.mic == Feature::On && connected;
        p.fill(b2, if mic_on { color::ON } else { color::TILE });
        let mic_icon = if mic_on { icons::MIC } else { icons::MIC_OFF };
        let mic_fg = if mic_on {
            color::ON_CONTENT
        } else if connected {
            color::TEXT
        } else {
            color::DISABLED
        };
        p.icon(mic_icon, b2.x + b2.w / 2.0, b2.y + b2.h / 2.0, ICON, mic_fg);

        // b3: Speaker stream toggle
        let b3 = l::bottom(3);
        let spk_on = s.speaker == Feature::On && connected;
        p.fill(b3, if spk_on { color::ON } else { color::TILE });
        let spk_icon = if spk_on {
            icons::SPEAKER
        } else {
            icons::SPEAKER_OFF
        };
        let spk_fg = if spk_on {
            color::ON_CONTENT
        } else if connected {
            color::TEXT
        } else {
            color::DISABLED
        };
        p.icon(spk_icon, b3.x + b3.w / 2.0, b3.y + b3.h / 2.0, ICON, spk_fg);

        // b4: Help drawer toggle
        let b4 = l::bottom(4);
        let help_active = matches!(self.drawer, Drawer::Help(_));
        p.fill(b4, if help_active { color::ON } else { color::TILE });
        let help_fg = if help_active {
            color::ON_CONTENT
        } else {
            color::TEXT2
        };
        p.icon(
            icons::HELP,
            b4.x + b4.w / 2.0,
            b4.y + b4.h / 2.0,
            ICON,
            help_fg,
        );
    }

    fn draw_drawer_wifi(&mut self, mode: crate::layout::WifiMode) {
        use crate::layout::WifiMode;
        let s = self.state.clone();
        let p = &mut self.painter;
        p.fill(l::PLAYGROUND, color::BG);

        // Header: Host PC Name + IP and copy button
        let host = if s.hostname.is_empty() {
            "PC"
        } else {
            &s.hostname
        };
        p.text(
            host,
            Rect::new(14.0, 8.0, 200.0, 18.0),
            Font::Title,
            color::TEXT,
            Align::Left,
            false,
        );
        let ip_text = format!("{host}:7653");
        p.text(
            &ip_text,
            Rect::new(14.0, 26.0, 200.0, 16.0),
            Font::Caption,
            color::TEXT2,
            Align::Left,
            false,
        );

        let copy_btn = l::CONN_COPY_IP_BTN;
        p.fill_rounded(copy_btn, 6.0, color::TILE);
        p.stroke_rounded(copy_btn, 6.0, color::HAIRLINE, 1.0);
        let copied = self.copied_at.is_some_and(|t| t.elapsed().as_secs() < 2);
        if copied {
            p.icon(
                icons::CHECK,
                copy_btn.x + copy_btn.w / 2.0,
                copy_btn.y + copy_btn.h / 2.0,
                14.0,
                color::TEXT,
            );
        } else {
            p.icon(
                icons::COPY,
                copy_btn.x + copy_btn.w / 2.0,
                copy_btn.y + copy_btn.h / 2.0,
                14.0,
                color::TEXT2,
            );
        }
        p.line(0.0, 48.0, l::WIDTH, 48.0, color::HAIRLINE, 1.0);

        // Primary connection row
        let prim = l::CONN_PRIMARY_ROW;
        let ind = indicator(&s.connection);
        let (row_ic, row_fg) = match mode {
            WifiMode::Connected | WifiMode::Pairing => (ind.icon, color::TEXT),
            _ => (ind.icon, color::DISABLED),
        };
        p.icon(row_ic, prim.x + 22.0, prim.y + prim.h / 2.0, 18.0, row_fg);
        let name = s.phone_name().unwrap_or("Pixel 8");
        p.text(
            name,
            Rect::new(42.0, prim.y + 8.0, 140.0, 18.0),
            Font::Title,
            color::TEXT,
            Align::Left,
            false,
        );
        let subtitle = match mode {
            WifiMode::Pairing => "Wants to pair",
            WifiMode::Connected => "Connected",
            WifiMode::Blocked => "Blocked",
            WifiMode::Forget => "Offline",
            WifiMode::Offline => "Offline",
        };
        p.text(
            subtitle,
            Rect::new(42.0, prim.y + 26.0, 140.0, 16.0),
            Font::Caption,
            if mode == WifiMode::Pairing {
                color::TEXT
            } else {
                color::TEXT2
            },
            Align::Left,
            false,
        );

        match mode {
            WifiMode::Pairing => {
                let pair = l::CONN_PAIR_BTN;
                p.fill_rounded(pair, 6.0, color::ON);
                p.icon(
                    icons::CHECK,
                    pair.x + pair.w / 2.0,
                    pair.y + pair.h / 2.0,
                    14.0,
                    color::ON_CONTENT,
                );
                let dec = l::CONN_DECLINE_BTN;
                p.fill_rounded(dec, 6.0, 0x1A1A1A);
                p.icon(
                    icons::CLOSE,
                    dec.x + dec.w / 2.0,
                    dec.y + dec.h / 2.0,
                    14.0,
                    color::TEXT2,
                );

                // Animated sync beam along bottom of primary row
                let beam_w = 90.0;
                let progress = ((self.anim_frame % 120) as f32) / 120.0;
                let beam_x = prim.x + progress * (prim.w + beam_w) - beam_w;
                let beam_r = Rect::new(
                    beam_x.max(prim.x),
                    prim.bottom() - 1.5,
                    beam_w.min(prim.right() - beam_x.max(prim.x)),
                    1.5,
                );
                if beam_r.w > 0.0 {
                    p.fill(beam_r, color::TEXT);
                }
            }
            WifiMode::Connected => {
                let disc = l::CONN_DISCONNECT_BTN;
                p.icon(
                    icons::UNPLUG,
                    disc.x + disc.w / 2.0,
                    disc.y + disc.h / 2.0,
                    16.0,
                    color::TEXT2,
                );
                let blk = l::CONN_BLOCK_BTN;
                p.text(
                    "Block",
                    blk,
                    Font::Caption,
                    color::TEXT2,
                    Align::Center,
                    false,
                );
            }
            WifiMode::Blocked => {
                let unblk = l::CONN_UNBLOCK_BTN;
                p.text(
                    "Unblock",
                    unblk,
                    Font::Caption,
                    color::TEXT,
                    Align::Center,
                    false,
                );
            }
            WifiMode::Forget => {
                let fgt = l::CONN_FORGET_BTN;
                p.text(
                    "Forget",
                    fgt,
                    Font::Caption,
                    color::TEXT2,
                    Align::Center,
                    false,
                );
            }
            WifiMode::Offline => {
                let blk = l::CONN_BLOCK_BTN;
                p.text(
                    "Block",
                    blk,
                    Font::Caption,
                    color::DISABLED,
                    Align::Center,
                    false,
                );
            }
        }
        p.line(
            0.0,
            prim.bottom(),
            l::WIDTH,
            prim.bottom(),
            color::HAIRLINE,
            1.0,
        );

        // Paired row
        let paired = l::CONN_PAIRED_ROW;
        if let Some(first_phone) = s.phones.first() {
            p.icon(
                icons::WIFI,
                paired.x + 22.0,
                paired.y + paired.h / 2.0,
                16.0,
                color::DISABLED,
            );
            p.text(
                &first_phone.name,
                Rect::new(42.0, paired.y + 8.0, 150.0, 18.0),
                Font::Body,
                color::TEXT2,
                Align::Left,
                false,
            );
            p.text(
                "Paired",
                Rect::new(42.0, paired.y + 26.0, 150.0, 16.0),
                Font::Caption,
                color::DISABLED,
                Align::Left,
                false,
            );
            let rem = l::CONN_REMOVE_PAIRED_BTN;
            p.text(
                "Remove",
                rem,
                Font::Caption,
                color::TEXT2,
                Align::Center,
                false,
            );
        } else {
            p.text(
                "No other paired devices",
                Rect::new(14.0, paired.y + 15.0, l::WIDTH - 28.0, 20.0),
                Font::Caption,
                color::DISABLED,
                Align::Left,
                false,
            );
        }
        p.line(
            0.0,
            paired.bottom(),
            l::WIDTH,
            paired.bottom(),
            color::HAIRLINE,
            1.0,
        );

        // Transport footer
        let tf = l::CONN_FOOTER;
        p.line(0.0, tf.y, l::WIDTH, tf.y, color::HAIRLINE, 1.0);

        let ref_btn = l::CONN_REFRESH_BTN;
        p.stroke_rounded(ref_btn, 6.0, color::HAIRLINE, 1.0);
        p.icon(
            icons::REFRESH,
            ref_btn.x + ref_btn.w / 2.0,
            ref_btn.y + ref_btn.h / 2.0,
            15.0,
            color::TEXT2,
        );

        let transports: [(usize, &'static str, crate::glyph::Icon); 4] = [
            (0, "link.usbDebugging", icons::USB),
            (1, "link.usbTethering", icons::CABLE),
            (2, "link.wifi", icons::WIFI),
            (3, "link.bluetooth", icons::BLUETOOTH),
        ];

        for (idx, key, ic) in transports {
            let btn = l::conn_transport_btn(idx);
            let on = s.setting(key) != "false";
            p.fill_rounded(btn, 6.0, if on { color::TILE } else { color::BG });
            p.stroke_rounded(btn, 6.0, if on { color::HAIRLINE } else { 0x222222 }, 1.0);
            p.icon(
                ic,
                btn.x + btn.w / 2.0,
                btn.y + btn.h / 2.0,
                15.0,
                if on { color::TEXT } else { color::DISABLED },
            );
        }
    }

    fn draw_drawer_help(&mut self, screen: HelpScreen) {
        let s = self.state.clone();
        let p = &mut self.painter;
        p.fill(l::PLAYGROUND, color::BG);

        match screen {
            HelpScreen::Bento => {
                let items: [(usize, &'static str, crate::glyph::Icon); 6] = [
                    (0, "How-to", icons::LIST),
                    (1, "Privacy", icons::SHIELD),
                    (2, "Rate", icons::STAR),
                    (3, "GitHub", icons::GITHUB),
                    (4, "Donate", icons::COFFEE),
                    (5, "Credits", icons::USERS),
                ];
                for (idx, label, ic) in items {
                    let tile = l::help_bento_tile(idx);
                    p.fill_rounded(tile, 10.0, color::TILE);
                    p.stroke_rounded(tile, 10.0, color::HAIRLINE, 1.0);
                    p.icon(ic, tile.x + tile.w / 2.0, tile.y + 28.0, 22.0, color::TEXT);
                    p.text(
                        label,
                        Rect::new(tile.x, tile.y + 50.0, tile.w, 18.0),
                        Font::Caption,
                        color::TEXT2,
                        Align::Center,
                        false,
                    );
                }
                p.text(
                    &s.version,
                    Rect::new(0.0, l::PLAYGROUND.bottom() - 20.0, l::WIDTH, 16.0),
                    Font::Caption,
                    color::DISABLED,
                    Align::Center,
                    false,
                );
            }
            HelpScreen::HowTo => {
                let back = l::HELP_BACK_BTN;
                p.icon_rotated_180(
                    icons::ARROW,
                    back.x + back.w / 2.0,
                    back.y + back.h / 2.0,
                    16.0,
                    color::TEXT,
                );
                p.text(
                    "How-to",
                    Rect::new(42.0, back.y, 200.0, back.h),
                    Font::Title,
                    color::TEXT,
                    Align::Left,
                    false,
                );
                p.line(0.0, 40.0, l::WIDTH, 40.0, color::HAIRLINE, 1.0);

                let steps: [(i32, &'static str, &'static str); 3] = [
                    (
                        1,
                        "Get the Android App",
                        "Install and open Owlmic on your phone",
                    ),
                    (
                        2,
                        "Pair with this PC",
                        "Plug in USB or join same Wi-Fi, or copy this PC's IP",
                    ),
                    (
                        3,
                        "Pick Owlmic in apps",
                        "Select Owlmic Camera or Mic in Discord, OBS, or Zoom",
                    ),
                ];

                for (idx, title, desc) in steps {
                    let y = 56.0 + (idx - 1) as f32 * 58.0;
                    let num_r = Rect::new(14.0, y + 2.0, 20.0, 20.0);
                    p.fill_rounded(num_r, 10.0, color::TILE);
                    p.stroke_rounded(num_r, 10.0, color::HAIRLINE, 1.0);
                    p.text(
                        &idx.to_string(),
                        num_r,
                        Font::Caption,
                        color::TEXT,
                        Align::Center,
                        false,
                    );

                    p.text(
                        title,
                        Rect::new(42.0, y, 250.0, 18.0),
                        Font::Body,
                        color::TEXT,
                        Align::Left,
                        false,
                    );
                    p.text(
                        desc,
                        Rect::new(42.0, y + 18.0, 250.0, 32.0),
                        Font::Caption,
                        color::TEXT2,
                        Align::Left,
                        true,
                    );
                }
            }
            HelpScreen::Privacy => {
                let back = l::HELP_BACK_BTN;
                p.icon_rotated_180(
                    icons::ARROW,
                    back.x + back.w / 2.0,
                    back.y + back.h / 2.0,
                    16.0,
                    color::TEXT,
                );
                p.text(
                    "Privacy",
                    Rect::new(42.0, back.y, 200.0, back.h),
                    Font::Title,
                    color::TEXT,
                    Align::Left,
                    false,
                );
                p.line(0.0, 40.0, l::WIDTH, 40.0, color::HAIRLINE, 1.0);

                p.text(
                    "Owlmic receives camera, microphone, and speaker streams strictly over your local USB or Wi-Fi network.\n\nZero media or telemetry leaves your local network.",
                    Rect::new(16.0, 56.0, l::WIDTH - 32.0, 180.0),
                    Font::Body,
                    color::TEXT2,
                    Align::Left,
                    true,
                );
            }
            HelpScreen::Credits => {
                let back = l::HELP_BACK_BTN;
                p.icon_rotated_180(
                    icons::ARROW,
                    back.x + back.w / 2.0,
                    back.y + back.h / 2.0,
                    16.0,
                    color::TEXT,
                );
                p.text(
                    "Credits",
                    Rect::new(42.0, back.y, 200.0, back.h),
                    Font::Title,
                    color::TEXT,
                    Align::Left,
                    false,
                );
                p.line(0.0, 40.0, l::WIDTH, 40.0, color::HAIRLINE, 1.0);

                let credits: [(&'static str, &'static str); 4] = [
                    ("Rust", "@rust-lang"),
                    ("Direct2D", "@microsoft"),
                    ("Oboe", "@google"),
                    ("WebRTC & Opus", "Open Source"),
                ];

                for (idx, (lib, author)) in credits.iter().enumerate() {
                    let y = 50.0 + idx as f32 * 36.0;
                    p.text(
                        lib,
                        Rect::new(16.0, y, 140.0, 24.0),
                        Font::Body,
                        color::TEXT,
                        Align::Left,
                        false,
                    );
                    p.text(
                        author,
                        Rect::new(160.0, y, l::WIDTH - 176.0, 24.0),
                        Font::Caption,
                        color::TEXT2,
                        Align::Right,
                        false,
                    );
                    p.line(
                        16.0,
                        y + 30.0,
                        l::WIDTH - 16.0,
                        y + 30.0,
                        color::HAIRLINE,
                        1.0,
                    );
                }
            }
        }
    }
}

fn copy_to_clipboard(hwnd: HWND, text: &str) {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};

    unsafe {
        if OpenClipboard(Some(hwnd)).is_ok() {
            let _ = EmptyClipboard();
            let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            let bytes = wide.len() * 2;
            if let Ok(hmem) = GlobalAlloc(GMEM_MOVEABLE, bytes) {
                let ptr = GlobalLock(hmem);
                if !ptr.is_null() {
                    std::ptr::copy_nonoverlapping(wide.as_ptr().cast::<u8>(), ptr.cast(), bytes);
                    let _ = GlobalUnlock(hmem);
                    let _ = SetClipboardData(13, Some(HANDLE(hmem.0)));
                }
            }
            let _ = CloseClipboard();
        }
    }
}

fn open(link: Link) {
    let target = match link {
        Link::Licences => std::env::current_exe().ok().and_then(|e| {
            Some(
                e.parent()?
                    .join("THIRD-PARTY-NOTICES.txt")
                    .to_string_lossy()
                    .into_owned(),
            )
        }),
        Link::Website => Some("https://owlmic.app".to_owned()),
        Link::Donate => Some("https://github.com/sponsors/diveshpatil9104".to_owned()),
        Link::GitHub | Link::Rate => Some("https://github.com/diveshpatil9104/owlmic".to_owned()),
    };
    if let Some(t) = target {
        unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                &HSTRING::from(t),
                None,
                None,
                SW_SHOWNORMAL,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::{LINK_USB_DEBUGGING, LINK_WIFI, PhoneEntry, Repair};
    use std::collections::BTreeMap;
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;

    unsafe extern "system" fn test_wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    fn write_bmp(path: &std::path::Path, width: i32, height: i32, bgra: &[u8]) {
        use std::io::Write;
        if let Ok(mut file) = std::fs::File::create(path) {
            let file_size = 54 + bgra.len() as u32;
            let _ = file.write_all(b"BM");
            let _ = file.write_all(&file_size.to_le_bytes());
            let _ = file.write_all(&[0u8; 4]);
            let _ = file.write_all(&54u32.to_le_bytes());
            let _ = file.write_all(&40u32.to_le_bytes());
            let _ = file.write_all(&width.to_le_bytes());
            let _ = file.write_all(&(-height).to_le_bytes());
            let _ = file.write_all(&1u16.to_le_bytes());
            let _ = file.write_all(&32u16.to_le_bytes());
            let _ = file.write_all(&0u32.to_le_bytes());
            let _ = file.write_all(&(bgra.len() as u32).to_le_bytes());
            let _ = file.write_all(&2835u32.to_le_bytes());
            let _ = file.write_all(&2835u32.to_le_bytes());
            let _ = file.write_all(&0u32.to_le_bytes());
            let _ = file.write_all(&0u32.to_le_bytes());
            let _ = file.write_all(bgra);
        }
    }

    use windows::Win32::Graphics::Direct2D::Common::*;
    use windows::Win32::Graphics::Direct2D::*;
    use windows::Win32::Graphics::Dxgi::Common::*;

    #[test]
    fn test_dc_render_target() {
        unsafe {
            let d2d: ID2D1Factory =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).unwrap();
            let props = D2D1_RENDER_TARGET_PROPERTIES {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                ..Default::default()
            };
            let dc_target = d2d.CreateDCRenderTarget(&props);
            assert!(
                dc_target.is_ok(),
                "CreateDCRenderTarget failed: {:?}",
                dc_target.err()
            );
        }
    }

    #[test]
    fn test_panel_creation() {
        let p = Painter::new();
        assert!(p.is_ok(), "Painter::new() failed: {:?}", p.err());
    }

    #[test]
    #[allow(clippy::vec_init_then_push)]
    fn test_render_all_pc_states() {
        unsafe {
            let instance = GetModuleHandleW(None).unwrap();
            let class_name = w!("OwlmicCaptureWindow");
            let class = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(test_wndproc),
                hInstance: instance.into(),
                lpszClassName: class_name,
                ..Default::default()
            };
            let _ = RegisterClassExW(&class);
            let w = l::WIDTH as i32;
            let h = l::HEIGHT as i32;
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                class_name,
                w!("OwlmicCapture"),
                WS_POPUP,
                100,
                100,
                w,
                h,
                None,
                None,
                Some(instance.into()),
                None,
            )
            .ok();

            if let Some(hwnd) = hwnd {
                let corners = DWMWCP_ROUND;
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_WINDOW_CORNER_PREFERENCE,
                    (&corners as *const DWM_WINDOW_CORNER_PREFERENCE).cast(),
                    size_of_val(&corners) as u32,
                );
                let border = crate::color::HAIRLINE.swap_bytes() >> 8;
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_BORDER_COLOR,
                    (&border as *const u32).cast(),
                    4,
                );
                let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 100, 100, w, h, SWP_SHOWWINDOW);
            }

            let preview = Arc::new(Preview::default());

            let mut settings = BTreeMap::new();
            settings.insert("mic.noiseReduction".into(), "5".into());
            settings.insert("mic.volume".into(), "0".into());
            settings.insert("mic.autoLevel".into(), "on".into());
            settings.insert("camera.lens".into(), "front".into());
            settings.insert("camera.quality".into(), "1080p".into());
            settings.insert("camera.frameRate".into(), "60".into());
            settings.insert("camera.framing".into(), "fit".into());
            settings.insert("camera.mirror".into(), "off".into());
            settings.insert("speaker.quietPc".into(), "on".into());
            settings.insert("link.usbDebugging".into(), "on".into());
            settings.insert("link.usbTethering".into(), "off".into());
            settings.insert("link.wifi".into(), "on".into());
            settings.insert("link.bluetooth".into(), "off".into());
            settings.insert("general.startWithWindows".into(), "on".into());

            let mut states: Vec<(&str, PanelState, Drawer, bool, bool)> = Vec::new();

            states.push((
                "01_standby_no_phone.bmp",
                PanelState {
                    connection: Connection::NotConnected,
                    settings: settings.clone(),
                    version: "v0.1.0".into(),
                    ..Default::default()
                },
                Drawer::None,
                false,
                false,
            ));

            states.push((
                "02_connecting_handshake.bmp",
                PanelState {
                    connection: Connection::Connecting {
                        link: LINK_USB_DEBUGGING,
                    },
                    settings: settings.clone(),
                    version: "v0.1.0".into(),
                    ..Default::default()
                },
                Drawer::None,
                false,
                false,
            ));

            states.push((
                "03_drawer_wifi_pairing.bmp",
                PanelState {
                    connection: Connection::Approval {
                        phone: "Pixel 8".into(),
                        code: "742 918".into(),
                        link: LINK_WIFI,
                    },
                    settings: settings.clone(),
                    version: "v0.1.0".into(),
                    ..Default::default()
                },
                Drawer::Wifi,
                false,
                false,
            ));

            states.push((
                "04_connected_usb.bmp",
                PanelState {
                    connection: Connection::Connected {
                        phone: "Pixel 8".into(),
                        link: LINK_USB_DEBUGGING,
                        weak: false,
                    },
                    settings: settings.clone(),
                    paired: true,
                    version: "v0.1.0".into(),
                    ..Default::default()
                },
                Drawer::None,
                false,
                false,
            ));

            states.push((
                "05_connected_wifi.bmp",
                PanelState {
                    connection: Connection::Connected {
                        phone: "Pixel 8".into(),
                        link: LINK_WIFI,
                        weak: false,
                    },
                    settings: settings.clone(),
                    paired: true,
                    version: "v0.1.0".into(),
                    ..Default::default()
                },
                Drawer::None,
                false,
                false,
            ));

            states.push((
                "06_connected_weak_link.bmp",
                PanelState {
                    connection: Connection::Connected {
                        phone: "Pixel 8".into(),
                        link: LINK_WIFI,
                        weak: true,
                    },
                    settings: settings.clone(),
                    paired: true,
                    version: "v0.1.0".into(),
                    ..Default::default()
                },
                Drawer::None,
                false,
                false,
            ));

            states.push((
                "07_mic_active.bmp",
                PanelState {
                    connection: Connection::Connected {
                        phone: "Pixel 8".into(),
                        link: LINK_USB_DEBUGGING,
                        weak: false,
                    },
                    mic: Feature::On,
                    settings: settings.clone(),
                    paired: true,
                    version: "v0.1.0".into(),
                    ..Default::default()
                },
                Drawer::None,
                false,
                false,
            ));

            states.push((
                "08_camera_active.bmp",
                PanelState {
                    connection: Connection::Connected {
                        phone: "Pixel 8".into(),
                        link: LINK_USB_DEBUGGING,
                        weak: false,
                    },
                    camera: Feature::On,
                    settings: settings.clone(),
                    paired: true,
                    version: "v0.1.0".into(),
                    ..Default::default()
                },
                Drawer::None,
                false,
                false,
            ));

            states.push((
                "09_speaker_loopback_active.bmp",
                PanelState {
                    connection: Connection::Connected {
                        phone: "Pixel 8".into(),
                        link: LINK_USB_DEBUGGING,
                        weak: false,
                    },
                    speaker: Feature::On,
                    settings: settings.clone(),
                    paired: true,
                    version: "v0.1.0".into(),
                    ..Default::default()
                },
                Drawer::None,
                false,
                false,
            ));

            states.push((
                "10_all_features_active.bmp",
                PanelState {
                    connection: Connection::Connected {
                        phone: "Pixel 8".into(),
                        link: LINK_USB_DEBUGGING,
                        weak: false,
                    },
                    mic: Feature::On,
                    camera: Feature::On,
                    speaker: Feature::On,
                    settings: settings.clone(),
                    paired: true,
                    version: "v0.1.0".into(),
                    ..Default::default()
                },
                Drawer::None,
                false,
                false,
            ));

            let phones = vec![
                PhoneEntry {
                    id: "p1".into(),
                    name: "Pixel 8".into(),
                    blocked: false,
                },
                PhoneEntry {
                    id: "p2".into(),
                    name: "Galaxy S24".into(),
                    blocked: true,
                },
            ];
            let addresses = vec!["192.168.1.105:7653".into(), "10.0.2.2:7653".into()];

            states.push((
                "11_drawer_wifi_connected.bmp",
                PanelState {
                    connection: Connection::Connected {
                        phone: "Pixel 8".into(),
                        link: LINK_USB_DEBUGGING,
                        weak: false,
                    },
                    settings: settings.clone(),
                    paired: true,
                    phones: phones.clone(),
                    addresses: addresses.clone(),
                    hostname: "YASH-PC".into(),
                    version: "v0.1.0".into(),
                    ..Default::default()
                },
                Drawer::Wifi,
                false,
                false,
            ));

            states.push((
                "12_drawer_wifi_blocked.bmp",
                PanelState {
                    connection: Connection::NotConnected,
                    settings: settings.clone(),
                    paired: true,
                    phones: vec![PhoneEntry {
                        id: "p1".into(),
                        name: "Pixel 8".into(),
                        blocked: true,
                    }],
                    addresses: addresses.clone(),
                    hostname: "YASH-PC".into(),
                    version: "v0.1.0".into(),
                    ..Default::default()
                },
                Drawer::Wifi,
                false,
                false,
            ));

            states.push((
                "13_drawer_wifi_offline_forget.bmp",
                PanelState {
                    connection: Connection::NotConnected,
                    settings: settings.clone(),
                    paired: true,
                    phones: phones.clone(),
                    addresses: addresses.clone(),
                    hostname: "YASH-PC".into(),
                    version: "v0.1.0".into(),
                    ..Default::default()
                },
                Drawer::Wifi,
                false,
                false,
            ));

            states.push((
                "14_repair_needed.bmp",
                PanelState {
                    connection: Connection::NotConnected,
                    repair: Repair {
                        mic: true,
                        cam: false,
                        net: false,
                    },
                    settings: settings.clone(),
                    version: "v0.1.0".into(),
                    ..Default::default()
                },
                Drawer::None,
                false,
                false,
            ));

            states.push((
                "15_drawer_help_bento.bmp",
                PanelState {
                    connection: Connection::Connected {
                        phone: "Pixel 8".into(),
                        link: LINK_WIFI,
                        weak: false,
                    },
                    settings: settings.clone(),
                    version: "v0.1.0".into(),
                    ..Default::default()
                },
                Drawer::Help(HelpScreen::Bento),
                false,
                false,
            ));

            states.push((
                "16_drawer_help_howto.bmp",
                PanelState {
                    connection: Connection::Connected {
                        phone: "Pixel 8".into(),
                        link: LINK_WIFI,
                        weak: false,
                    },
                    settings: settings.clone(),
                    version: "v0.1.0".into(),
                    ..Default::default()
                },
                Drawer::Help(HelpScreen::HowTo),
                false,
                false,
            ));

            states.push((
                "17_drawer_help_privacy.bmp",
                PanelState {
                    connection: Connection::Connected {
                        phone: "Pixel 8".into(),
                        link: LINK_WIFI,
                        weak: false,
                    },
                    settings: settings.clone(),
                    version: "v0.1.0".into(),
                    ..Default::default()
                },
                Drawer::Help(HelpScreen::Privacy),
                false,
                false,
            ));

            states.push((
                "18_drawer_help_credits.bmp",
                PanelState {
                    connection: Connection::Connected {
                        phone: "Pixel 8".into(),
                        link: LINK_WIFI,
                        weak: false,
                    },
                    settings: settings.clone(),
                    version: "v0.1.0".into(),
                    ..Default::default()
                },
                Drawer::Help(HelpScreen::Credits),
                false,
                false,
            ));

            states.push((
                "19_cam_with_hud_menu.bmp",
                PanelState {
                    connection: Connection::Connected {
                        phone: "Pixel 8".into(),
                        link: LINK_USB_DEBUGGING,
                        weak: false,
                    },
                    camera: Feature::On,
                    settings: settings.clone(),
                    paired: true,
                    version: "v0.1.0".into(),
                    ..Default::default()
                },
                Drawer::None,
                true,
                false,
            ));

            states.push((
                "20_mic_with_quick_audio.bmp",
                PanelState {
                    connection: Connection::Connected {
                        phone: "Pixel 8".into(),
                        link: LINK_USB_DEBUGGING,
                        weak: false,
                    },
                    mic: Feature::On,
                    settings: settings.clone(),
                    paired: true,
                    version: "v0.1.0".into(),
                    ..Default::default()
                },
                Drawer::None,
                false,
                true,
            ));

            let out_dir =
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../ui/pc");
            let _ = std::fs::create_dir_all(&out_dir);

            for (file_name, st, drawer, cam_menu, quick_audio) in states {
                let arc_st = Arc::new(st);
                let mut panel = Panel {
                    hwnd: HWND::default(),
                    ui: Ui {
                        state: Publisher::new((*arc_st).clone(), || {}),
                        preview: preview.clone(),
                        on_action: Box::new(|_| {}),
                    },
                    painter: Painter::new().unwrap(),
                    tray: None,
                    state: arc_st,
                    drawer,
                    cam_menu_open: cam_menu,
                    quick_audio_open: quick_audio,
                    scale: 1.0,
                    visible: true,
                    hidden_at: None,
                    applied_at: Instant::now(),
                    copied_at: None,
                    taskbar_created: 0,
                    anim_frame: 0,
                };

                let screen_dc = GetDC(None);
                if screen_dc.is_invalid() {
                    continue;
                }
                let mem_dc = CreateCompatibleDC(Some(screen_dc));
                if mem_dc.is_invalid() {
                    let _ = ReleaseDC(None, screen_dc);
                    continue;
                }
                let bmp = CreateCompatibleBitmap(screen_dc, w, h);
                if bmp.is_invalid() {
                    let _ = DeleteDC(mem_dc);
                    let _ = ReleaseDC(None, screen_dc);
                    continue;
                }
                let old_bmp = SelectObject(mem_dc, bmp.into());

                let rc = RECT {
                    left: 0,
                    top: 0,
                    right: w,
                    bottom: h,
                };
                if panel.painter.begin_dc(mem_dc, &rc, 1.0) {
                    panel.painter.clear(color::HAIRLINE);
                    let screen = panel.state.active_screen(
                        panel.drawer,
                        panel.cam_menu_open,
                        panel.quick_audio_open,
                    );
                    match screen {
                        Screen::DrawerWifi(mode) => {
                            panel.draw_drawer_wifi(mode);
                            panel.draw_bottom_dock();
                        }
                        Screen::DrawerHelp(h) => {
                            panel.draw_drawer_help(h);
                            panel.draw_bottom_dock();
                        }
                        Screen::Main {
                            cam_on,
                            mic_on,
                            cam_menu,
                            quick_audio,
                        } => {
                            panel.draw_main(cam_on, mic_on, cam_menu, quick_audio);
                            panel.draw_bottom_dock();
                        }
                    }
                    panel.painter.end();
                }

                let mut bmi = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: w,
                        biHeight: -h,
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let mut pixels = vec![0u8; (w * h * 4) as usize];
                GetDIBits(
                    mem_dc,
                    bmp,
                    0,
                    h as u32,
                    Some(pixels.as_mut_ptr().cast()),
                    &mut bmi,
                    DIB_RGB_COLORS,
                );

                SelectObject(mem_dc, old_bmp);
                let _ = DeleteObject(bmp.into());
                let _ = DeleteDC(mem_dc);
                let _ = ReleaseDC(None, screen_dc);

                let path = out_dir.join(file_name);
                write_bmp(&path, w, h, &pixels);
                println!("Rendered {file_name}");
            }

            if let Some(hwnd) = hwnd {
                let _ = DestroyWindow(hwnd);
            }
        }
    }
}
