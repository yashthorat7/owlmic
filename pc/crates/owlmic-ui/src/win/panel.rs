//! The panel window: 312 x 374 compact Bento 6-square layout at 100% scale above the tray,
//! never in the taskbar, closed by Esc or by clicking elsewhere (SYSTEM_DESIGN section 33).
//! It shows the newest state snapshot the App Hub published, at most 60 times a second.

use super::draw::{Align, Font, Painter};
use super::tray::{self, Tray};
use crate::layout::{self as l, Drawer, Rect, Screen, Target};
use crate::preview::{self, Preview};
use crate::view::{Action, Connection, Feature, Link, PanelState, indicator, value_text};
use crate::{color, fill, icons, messages as m};
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
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, SetFocus, VK_ESCAPE,
};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DraggingSlider {
    Noise,
    Volume,
}

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
    current_drawer: Option<Drawer>,
    watching: bool,
    wave_time: f32,
    dragging_slider: Option<DraggingSlider>,
    scale: f32,
    visible: bool,
    shown_at: Option<Instant>,
    hidden_at: Option<Instant>,
    applied_at: Instant,
    taskbar_created: u32,
}

/// Creates the panel and the tray icon and runs the message loop until [`Waker::quit`].
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
            current_drawer: None,
            watching: true,
            wave_time: 0.0,
            dragging_slider: None,
            scale: 1.0,
            visible: false,
            shown_at: None,
            hidden_at: None,
            applied_at: Instant::now(),
            taskbar_created: RegisterWindowMessageW(w!("TaskbarCreated")),
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
        let corners = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&corners as *const DWM_WINDOW_CORNER_PREFERENCE).cast(),
            size_of_val(&corners) as u32,
        );
        let border = crate::color::GAP.swap_bytes() >> 8;
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
                self.wave_time += 0.055;
                self.invalidate();
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
                        if self.shown_at.is_none_or(|t| t.elapsed() > REOPEN_GUARD) {
                            self.hide();
                        }
                    } else if self.hidden_at.is_none_or(|t| t.elapsed() > REOPEN_GUARD) {
                        self.show(true);
                    }
                }
            }
            WM_ACTIVATE if (wparam.0 & 0xFFFF) as u32 == WA_INACTIVE && self.visible => {
                if self.shown_at.is_none_or(|t| t.elapsed() > REOPEN_GUARD) {
                    self.hide();
                }
            }
            WM_KEYDOWN if wparam.0 as u16 == VK_ESCAPE.0 => {
                if self.current_drawer.is_some() {
                    self.current_drawer = None;
                    self.invalidate();
                } else {
                    self.hide();
                }
            }
            WM_LBUTTONDOWN => {
                let x = lo(lparam.0) as f32 / self.scale;
                let y = hi(lparam.0) as f32 / self.scale;
                self.mouse_down(x, y);
            }
            WM_MOUSEMOVE => {
                let x = lo(lparam.0) as f32 / self.scale;
                self.mouse_move(x);
            }
            WM_LBUTTONUP => {
                let x = lo(lparam.0) as f32 / self.scale;
                let y = hi(lparam.0) as f32 / self.scale;
                self.mouse_up(x, y);
            }
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

    fn apply(&mut self) {
        self.applied_at = Instant::now();
        let new = self.ui.state.read();
        let asks = |s: &PanelState| matches!(s.connection, Connection::Approval { .. });
        let open_gate = asks(&new) && !asks(&self.state);
        self.state = new;
        let tip = self.state.tooltip();
        if let Some(t) = self.tray.as_mut() {
            t.set_tip(&tip);
        }
        if open_gate && !self.visible {
            self.show(false);
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
        let margin = (20.0 * self.scale).round() as i32;
        let w = info.rcWork;
        let (x, y) = l::place(
            (anchor.left, anchor.top, anchor.right, anchor.bottom),
            (w.left, w.top, w.right, w.bottom),
            size,
            margin,
        );
        let flags = SWP_NOZORDER | SWP_NOACTIVATE;
        unsafe {
            let _ = SetWindowPos(self.hwnd, Some(HWND_TOPMOST), x, y, size.0, size.1, flags);
            if activate {
                let _ = AnimateWindow(self.hwnd, 100, AW_BLEND | AW_ACTIVATE);
                let _ = ShowWindow(self.hwnd, SW_SHOW);
                let _ = SetForegroundWindow(self.hwnd);
                let _ = SetFocus(Some(self.hwnd));
            } else {
                let _ = AnimateWindow(self.hwnd, 100, AW_BLEND);
                let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            }
            SetTimer(
                Some(self.hwnd),
                TIMER_PREVIEW,
                preview::EVERY_MS as u32,
                None,
            );
        }
        self.shown_at = Some(Instant::now());
        self.visible = true;
        self.ui.preview.set_wanted(true);
        self.invalidate();
    }

    fn hide(&mut self) {
        unsafe {
            let _ = AnimateWindow(self.hwnd, 80, AW_BLEND | AW_HIDE);
            let _ = ShowWindow(self.hwnd, SW_HIDE);
            let _ = KillTimer(Some(self.hwnd), TIMER_PREVIEW);
        }
        self.visible = false;
        self.current_drawer = None;
        self.dragging_slider = None;
        self.hidden_at = Some(Instant::now());
        self.ui.preview.set_wanted(false);
    }

    fn mouse_down(&mut self, x: f32, y: f32) {
        let screen = self.state.screen(self.current_drawer);
        if let Some(target) = l::hit(screen, x, y, self.state.phones.len()) {
            match target {
                Target::NoiseSlider(frac) => {
                    self.dragging_slider = Some(DraggingSlider::Noise);
                    unsafe { SetCapture(self.hwnd) };
                    if let Some(action) = self.state.click(Target::NoiseSlider(frac)) {
                        (self.ui.on_action)(action);
                    }
                    self.invalidate();
                }
                Target::VolumeSlider(frac) => {
                    self.dragging_slider = Some(DraggingSlider::Volume);
                    unsafe { SetCapture(self.hwnd) };
                    if let Some(action) = self.state.click(Target::VolumeSlider(frac)) {
                        (self.ui.on_action)(action);
                    }
                    self.invalidate();
                }
                _ => {}
            }
        }
    }

    fn mouse_move(&mut self, x: f32) {
        if let Some(drag) = self.dragging_slider {
            match drag {
                DraggingSlider::Noise => {
                    let frac =
                        ((x - l::SLIDER_NOISE_TRACK.x) / l::SLIDER_NOISE_TRACK.w).clamp(0.0, 1.0);
                    if let Some(action) = self.state.click(Target::NoiseSlider(frac)) {
                        (self.ui.on_action)(action);
                    }
                }
                DraggingSlider::Volume => {
                    let frac =
                        ((x - l::SLIDER_VOLUME_TRACK.x) / l::SLIDER_VOLUME_TRACK.w).clamp(0.0, 1.0);
                    if let Some(action) = self.state.click(Target::VolumeSlider(frac)) {
                        (self.ui.on_action)(action);
                    }
                }
            }
            self.invalidate();
        }
    }

    fn mouse_up(&mut self, x: f32, y: f32) {
        if self.dragging_slider.is_some() {
            self.dragging_slider = None;
            unsafe {
                let _ = ReleaseCapture();
            };
            self.invalidate();
            return;
        }

        let screen = self.state.screen(self.current_drawer);
        let Some(target) = l::hit(screen, x, y, self.state.phones.len()) else {
            return;
        };
        match target {
            Target::Help => {
                self.current_drawer = if self.current_drawer == Some(Drawer::Help) {
                    None
                } else {
                    Some(Drawer::Help)
                };
            }
            Target::Wifi => {
                self.current_drawer = if self.current_drawer == Some(Drawer::Connection) {
                    None
                } else {
                    Some(Drawer::Connection)
                };
            }
            Target::MicSettings | Target::Waveform => {
                self.current_drawer = if self.current_drawer == Some(Drawer::Mic) {
                    None
                } else {
                    Some(Drawer::Mic)
                };
            }
            Target::DrawerBack => {
                self.current_drawer = None;
            }
            Target::Watch => {
                self.watching = !self.watching;
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
            self.painter.clear(color::GAP);
            match self.state.screen(self.current_drawer) {
                Screen::Gate => self.draw_gate(),
                Screen::Main { .. } => {
                    if let Some(d) = self.current_drawer {
                        self.draw_drawer(d);
                    } else {
                        self.draw_playground();
                    }
                    self.draw_bottom_bar();
                }
            }
            self.painter.end();
        }
        unsafe {
            let _ = EndPaint(self.hwnd, &ps);
        }
    }

    fn draw_bottom_bar(&self) {
        let p = &self.painter;
        let s = &self.state;

        // Button 0: Help
        let help_open = self.current_drawer == Some(Drawer::Help);
        let (bg, fg) = if help_open {
            (color::INK, color::BG)
        } else {
            (color::BG, color::INK)
        };
        p.fill_rounded(l::BTN_HELP, bg, 12.0);
        p.icon(
            icons::HELP,
            l::BTN_HELP.x + l::BTN_HELP.w / 2.0,
            l::BTN_HELP.y + l::BTN_HELP.h / 2.0,
            22.0,
            fg,
        );

        // Button 1: Camera
        p.fill_rounded(l::BTN_CAM, color::BG, 12.0);
        let (cam_icon, cam_fg) = if s.camera == Feature::On {
            (icons::VIDEO, color::INK)
        } else {
            (icons::VIDEO_OFF, color::DISABLED)
        };
        p.icon(
            cam_icon,
            l::BTN_CAM.x + l::BTN_CAM.w / 2.0,
            l::BTN_CAM.y + l::BTN_CAM.h / 2.0,
            22.0,
            cam_fg,
        );

        // Button 2: Mic
        p.fill_rounded(l::BTN_MIC, color::BG, 12.0);
        let (mic_icon, mic_fg) = if s.mic == Feature::On {
            (icons::MIC, color::INK)
        } else {
            (icons::MIC_OFF, color::DISABLED)
        };
        p.icon(
            mic_icon,
            l::BTN_MIC.x + l::BTN_MIC.w / 2.0,
            l::BTN_MIC.y + l::BTN_MIC.h / 2.0,
            22.0,
            mic_fg,
        );

        // Button 3: Speaker
        p.fill_rounded(l::BTN_SPK, color::BG, 12.0);
        let (spk_icon, spk_fg) = if s.speaker == Feature::On {
            (icons::SPEAKER, color::INK)
        } else {
            (icons::SPEAKER_OFF, color::DISABLED)
        };
        p.icon(
            spk_icon,
            l::BTN_SPK.x + l::BTN_SPK.w / 2.0,
            l::BTN_SPK.y + l::BTN_SPK.h / 2.0,
            22.0,
            spk_fg,
        );

        // Button 4: Connection
        let conn_open = self.current_drawer == Some(Drawer::Connection);
        let ind = indicator(&s.connection);
        let (conn_bg, conn_fg) = if conn_open {
            (color::INK, color::BG)
        } else {
            (color::BG, ind.color)
        };
        p.fill_rounded(l::BTN_WIFI, conn_bg, 12.0);
        p.icon(
            ind.icon,
            l::BTN_WIFI.x + l::BTN_WIFI.w / 2.0,
            l::BTN_WIFI.y + l::BTN_WIFI.h / 2.0,
            22.0,
            conn_fg,
        );
    }

    fn draw_playground(&mut self) {
        let s = self.state.clone();
        let p = &mut self.painter;
        let pg = l::PLAYGROUND;

        p.fill_rounded(pg, color::BG, 12.0);

        let cam_live = s.camera == Feature::On && self.watching && self.ui.preview.seq() > 0;
        if cam_live {
            let seq = self.ui.preview.seq();
            let preview = self.ui.preview.clone();
            let mirror = s.value("camera.mirror") == "on";
            p.clip(pg);
            p.picture(pg, preview::WIDTH, preview::HEIGHT, seq, mirror, |copy| {
                preview.read(|px| copy(px))
            });
            p.unclip();
        } else {
            // Dot matrix and owl watermark
            p.dot_grid(pg, 18.0, 1.0, 0.14);
            p.watermark(pg, 0.05);

            if !s.connected() {
                let mid_x = pg.x + pg.w / 2.0;
                let mid_y = pg.y + pg.h / 2.0;
                p.icon(icons::VIDEO_OFF, mid_x, mid_y - 20.0, 28.0, color::FAINT);
                p.text(
                    m::PC_NO_PHONE,
                    Rect::new(pg.x + PAD, mid_y + 4.0, pg.w - 2.0 * PAD, 18.0),
                    Font::Title,
                    color::INK,
                    Align::Center,
                    false,
                );
                p.text(
                    m::PC_OPEN_PHONE,
                    Rect::new(pg.x + PAD, mid_y + 24.0, pg.w - 2.0 * PAD, 16.0),
                    Font::Caption,
                    color::TEXT2,
                    Align::Center,
                    false,
                );
            }
        }

        // Floating Camera Controls (Eye, Mirror, Flip)
        if s.camera == Feature::On {
            let draw_round_btn = |p: &Painter, r: Rect, icon: crate::glyph::Icon, active: bool| {
                if active {
                    p.fill_rounded(r, color::INK, 16.0);
                    p.icon(icon, r.x + r.w / 2.0, r.y + r.h / 2.0, 16.0, color::BG);
                } else {
                    p.fill_rounded_alpha(r, color::BG, 0.72, 16.0);
                    p.stroke_rounded(r, 0xFFFFFF, 0.16, 1.0, 16.0);
                    p.icon(icon, r.x + r.w / 2.0, r.y + r.h / 2.0, 16.0, color::INK);
                }
            };

            let watch_icon = if self.watching {
                icons::EYE
            } else {
                icons::EYE_OFF
            };
            draw_round_btn(p, l::BTN_WATCH, watch_icon, false);

            let mirror_on = s.value("camera.mirror") == "on";
            draw_round_btn(p, l::BTN_MIRROR, icons::MIRROR, mirror_on && s.paired);

            draw_round_btn(p, l::BTN_FLIP, icons::FLIP, false);
        }

        // Microphone Waveform
        if s.mic == Feature::On {
            let is_docked = s.camera == Feature::On && self.watching;
            let min_h = if is_docked { 3.0 } else { 4.0 };
            let max_h = if is_docked { 16.0 } else { 64.0 };
            let center_idx = 12.0f32;
            let mut bars = [0.0f32; 25];
            for (i, bar) in bars.iter_mut().enumerate() {
                let dist = ((i as f32 - center_idx).abs() / center_idx).clamp(0.0, 1.0);
                let envelope = (0.2f32).max((dist * std::f32::consts::PI * 0.46).cos());
                let w1 = (self.wave_time * 5.2 + i as f32 * 0.52).sin();
                let w2 = (self.wave_time * 9.1 + i as f32 * 1.1).sin() * 0.5;
                let w3 = (self.wave_time * 14.0 + i as f32 * 0.28).cos() * 0.25;
                let dynamic_motion = (w1 + w2 + w3) / 1.75;
                let frac = ((0.35 + (dynamic_motion + 1.0) * 0.32) * envelope).clamp(0.08, 1.0);
                *bar = min_h + (max_h - min_h) * frac;
            }

            if is_docked {
                let r = l::WAVEFORM_DOCKED;
                p.fill_rounded(r, color::BG, 10.0);
                p.stroke_rounded(r, 0xFFFFFF, 0.16, 1.0, 10.0);
                p.waveform_bars(
                    r.x + r.w / 2.0,
                    r.y + r.h / 2.0,
                    &bars,
                    3.0,
                    3.0,
                    color::INK,
                );
            } else {
                p.waveform_bars(
                    pg.x + pg.w / 2.0,
                    pg.y + pg.h / 2.0 - 16.0,
                    &bars,
                    4.0,
                    4.0,
                    color::INK,
                );
            }

            // Floating Mic Settings Button
            let r = l::BTN_MIC_SETTINGS;
            p.fill_rounded_alpha(r, color::BG, 0.72, 16.0);
            p.stroke_rounded(r, 0xFFFFFF, 0.16, 1.0, 16.0);
            p.icon(
                icons::SLIDERS,
                r.x + r.w / 2.0,
                r.y + r.h / 2.0,
                16.0,
                color::INK,
            );
        }

        // Repair Banner
        if s.repair.needed() {
            let r = l::REPAIR_BANNER;
            p.fill_rounded_alpha(r, 0x0A0A0A, 0.95, 10.0);
            p.stroke_rounded(r, color::YELLOW, 0.35, 1.0, 10.0);
            p.icon(
                icons::ALERT,
                r.x + 20.0,
                r.y + r.h / 2.0,
                18.0,
                color::YELLOW,
            );
            p.text(
                s.repair_text(),
                Rect::new(r.x + 36.0, r.y, r.w - 110.0, r.h),
                Font::Caption,
                color::TEXT,
                Align::Left,
                false,
            );
            let btn = l::REPAIR_BTN;
            let (bg, fg) = if s.repairing {
                (color::DISABLED, color::TEXT2)
            } else {
                (color::ON, color::ON_CONTENT)
            };
            p.fill_rounded(btn, bg, 6.0);
            p.text(m::PC_REPAIR, btn, Font::Caption, fg, Align::Center, false);
        }
    }

    fn draw_drawer(&mut self, drawer: Drawer) {
        let s = self.state.clone();
        let p = &mut self.painter;
        let pg = l::PLAYGROUND;

        p.fill_rounded(pg, color::BG, 12.0);

        // Header
        let h = l::DRAWER_HEADER;
        p.icon(
            icons::ARROW_LEFT,
            l::DRAWER_BACK.x + l::DRAWER_BACK.w / 2.0,
            l::DRAWER_BACK.y + l::DRAWER_BACK.h / 2.0,
            20.0,
            color::TEXT,
        );
        let title = match drawer {
            Drawer::Connection => m::PC_CONNECTION,
            Drawer::Help => m::PC_SETUP_AND_HELP,
            Drawer::Mic => m::PC_MICROPHONE,
        };
        p.text(
            title,
            Rect::new(h.x + 44.0, h.y, h.w - 50.0, h.h),
            Font::Title,
            color::TEXT,
            Align::Left,
            false,
        );
        p.line((h.x, h.bottom()), (h.right(), h.bottom()), color::RULE, 1.0);

        match drawer {
            Drawer::Connection => {
                let mut y = l::DRAWER_BODY.y;

                // Active Phone
                if let Some(phone) = s.phone_name() {
                    let r = Rect::new(pg.x, y, pg.w, l::PHONE_ROW_H);
                    p.text(
                        phone,
                        Rect::new(r.x + 14.0, r.y + 6.0, r.w - 80.0, 18.0),
                        Font::Title,
                        color::TEXT,
                        Align::Left,
                        false,
                    );
                    let meta = format!(
                        "{}:7653 · {}",
                        s.addresses.first().map_or("127.0.0.1", String::as_str),
                        indicator(&s.connection).label
                    );
                    p.text(
                        &meta,
                        Rect::new(r.x + 14.0, r.y + 24.0, r.w - 80.0, 16.0),
                        Font::Caption,
                        color::TEXT2,
                        Align::Left,
                        false,
                    );
                    // Active badge
                    p.fill_circle(r.right() - 62.0, r.y + r.h / 2.0, 3.0, color::GREEN, 1.0);
                    p.text(
                        m::PC_ACTIVE,
                        Rect::new(r.right() - 54.0, r.y, 44.0, r.h),
                        Font::Caption,
                        color::GREEN,
                        Align::Left,
                        false,
                    );
                    p.line((r.x, r.bottom()), (r.right(), r.bottom()), color::RULE, 1.0);
                    y += l::PHONE_ROW_H;
                }

                // Paired phones
                for p_entry in s
                    .phones
                    .iter()
                    .filter(|p| Some(p.name.as_str()) != s.phone_name())
                {
                    let r = Rect::new(pg.x, y, pg.w, l::PHONE_ROW_H);
                    p.text(
                        &p_entry.name,
                        Rect::new(r.x + 14.0, r.y + 6.0, r.w - 120.0, 18.0),
                        Font::Title,
                        color::TEXT,
                        Align::Left,
                        false,
                    );
                    let sub = if p_entry.blocked {
                        m::PC_BLOCKED
                    } else {
                        m::PC_STANDBY
                    };
                    p.text(
                        sub,
                        Rect::new(r.x + 14.0, r.y + 24.0, r.w - 120.0, 16.0),
                        Font::Caption,
                        color::TEXT2,
                        Align::Left,
                        false,
                    );
                    p.line((r.x, r.bottom()), (r.right(), r.bottom()), color::RULE, 1.0);
                    y += l::PHONE_ROW_H;
                }

                // Direct IP row
                let r = Rect::new(pg.x, y, pg.w, l::PHONE_ROW_H);
                p.text(
                    m::PHONE_PC_DIRECT_IP,
                    Rect::new(r.x + 14.0, r.y + 6.0, r.w - 80.0, 18.0),
                    Font::Title,
                    color::TEXT,
                    Align::Left,
                    false,
                );
                p.text(
                    m::PC_DIRECT_IP_PORT,
                    Rect::new(r.x + 14.0, r.y + 24.0, r.w - 80.0, 16.0),
                    Font::Caption,
                    color::TEXT2,
                    Align::Left,
                    false,
                );
                p.text(
                    m::PC_READY,
                    Rect::new(r.right() - 54.0, r.y, 44.0, r.h),
                    Font::Caption,
                    color::TEXT2,
                    Align::Left,
                    false,
                );
                p.line((r.x, r.bottom()), (r.right(), r.bottom()), color::RULE, 1.0);

                // Footer with 5 link buttons
                let f = l::CONN_FOOTER;
                p.line((f.x, f.y), (f.right(), f.y), color::RULE, 1.0);
                let link_keys = [
                    "link.usbDebugging",
                    "link.usbTethering",
                    "link.wifi",
                    "link.bluetooth",
                    "",
                ];
                let link_icons = [
                    icons::USB,
                    icons::CABLE,
                    icons::WIFI,
                    icons::BLUETOOTH,
                    icons::GLOBE,
                ];
                for i in 0..5 {
                    let btn = l::conn_type_btn(i);
                    let is_on = if i < 4 {
                        s.value(link_keys[i]) != "off"
                    } else {
                        true
                    };
                    if is_on {
                        p.fill_rounded(btn, color::CHIP_ON, 7.0);
                        p.stroke_rounded(btn, color::CHIP_ON_EDGE, 1.0, 1.0, 7.0);
                        p.icon(
                            link_icons[i],
                            btn.x + btn.w / 2.0,
                            btn.y + btn.h / 2.0,
                            18.0,
                            color::TEXT,
                        );
                    } else {
                        p.fill_rounded(btn, color::BG, 7.0);
                        p.stroke_rounded(btn, color::CHIP_OFF_EDGE, 1.0, 1.0, 7.0);
                        p.icon(
                            link_icons[i],
                            btn.x + btn.w / 2.0,
                            btn.y + btn.h / 2.0,
                            18.0,
                            color::DISABLED,
                        );
                    }
                }
            }
            Drawer::Help => {
                let draw_step = |p: &Painter,
                                 r: Rect,
                                 num: &str,
                                 title: &str,
                                 desc: &str,
                                 tag: Option<&str>,
                                 is_link: bool| {
                    p.text(
                        num,
                        Rect::new(r.x + 14.0, r.y + 8.0, 20.0, 18.0),
                        Font::Caption,
                        color::TEXT2,
                        Align::Left,
                        false,
                    );
                    p.text(
                        title,
                        Rect::new(r.x + 38.0, r.y + 6.0, r.w - 90.0, 18.0),
                        Font::Body,
                        color::TEXT,
                        Align::Left,
                        false,
                    );
                    p.text(
                        desc,
                        Rect::new(r.x + 38.0, r.y + 24.0, r.w - 90.0, 16.0),
                        Font::Caption,
                        color::TEXT2,
                        Align::Left,
                        false,
                    );
                    if let Some(t) = tag {
                        let tag_color = if t == "Live" {
                            color::GREEN
                        } else {
                            color::TEXT2
                        };
                        p.text(
                            t,
                            Rect::new(r.right() - 54.0, r.y, 44.0, r.h),
                            Font::Caption,
                            tag_color,
                            Align::Right,
                            false,
                        );
                    }
                    if is_link {
                        p.icon(
                            icons::EXTERNAL,
                            r.right() - 24.0,
                            r.y + r.h / 2.0,
                            14.0,
                            color::TEXT2,
                        );
                    }
                    p.line((r.x, r.bottom()), (r.right(), r.bottom()), color::RULE, 1.0);
                };

                draw_step(
                    p,
                    l::HELP_STEP_1,
                    "01",
                    m::PHONE_STEP_DOWNLOAD,
                    "Get Windows (.exe) & APK",
                    None,
                    true,
                );
                draw_step(
                    p,
                    l::HELP_STEP_2,
                    "02",
                    m::PHONE_STEP_CONNECT,
                    m::PC_STEP_CONNECT_DETAIL,
                    Some("Auto"),
                    false,
                );
                draw_step(
                    p,
                    l::HELP_STEP_3,
                    "03",
                    m::PHONE_STEP_STREAM,
                    m::PC_STEP_STREAM_DETAIL,
                    Some("Live"),
                    false,
                );

                // Privacy Policy
                let r = l::HELP_PRIVACY;
                p.text(
                    m::UI_PRIVACY,
                    Rect::new(r.x + 14.0, r.y, r.w - 40.0, r.h),
                    Font::Body,
                    color::TEXT,
                    Align::Left,
                    false,
                );
                p.icon(
                    icons::ARROW_RIGHT,
                    r.right() - 24.0,
                    r.y + r.h / 2.0,
                    14.0,
                    color::TEXT2,
                );
                p.line((r.x, r.bottom()), (r.right(), r.bottom()), color::RULE, 1.0);

                // Donate / GitHub
                let r = l::HELP_DONATE;
                p.text(
                    m::UI_DONATE,
                    Rect::new(r.x + 14.0, r.y, r.w - 40.0, r.h),
                    Font::Body,
                    color::TEXT,
                    Align::Left,
                    false,
                );
                p.icon(
                    icons::ARROW_RIGHT,
                    r.right() - 24.0,
                    r.y + r.h / 2.0,
                    14.0,
                    color::TEXT2,
                );
                p.line((r.x, r.bottom()), (r.right(), r.bottom()), color::RULE, 1.0);

                // Version
                let r = l::HELP_VERSION;
                p.text(
                    "Owlmic PC",
                    Rect::new(r.x + 14.0, r.y, 100.0, r.h),
                    Font::Caption,
                    color::FAINT,
                    Align::Left,
                    false,
                );
                p.text(
                    "v1.0.0",
                    Rect::new(r.right() - 60.0, r.y, 46.0, r.h),
                    Font::Caption,
                    color::FAINT,
                    Align::Right,
                    false,
                );
            }
            Drawer::Mic => {
                // 1. Noise Reduction
                let noise = s.noise_reduction();
                let noise_str = if noise == 0 {
                    m::VALUE_OFF.to_string()
                } else {
                    noise.to_string()
                };
                p.text(
                    m::SETTING_MIC_NOISE_REDUCTION,
                    Rect::new(15.0, 48.0, 180.0, 18.0),
                    Font::Body,
                    color::TEXT,
                    Align::Left,
                    false,
                );
                p.text(
                    &noise_str,
                    Rect::new(pg.right() - 65.0, 48.0, 50.0, 18.0),
                    Font::Body,
                    color::INK,
                    Align::Right,
                    false,
                );

                let track = l::SLIDER_NOISE_TRACK;
                let track_y = track.y + track.h / 2.0;
                p.line(
                    (track.x, track_y),
                    (track.right(), track_y),
                    color::RULE,
                    4.0,
                );
                let frac = (noise as f32 / 10.0).clamp(0.0, 1.0);
                let thumb_x = track.x + frac * track.w;
                p.line((track.x, track_y), (thumb_x, track_y), color::INK, 4.0);
                p.fill_circle(thumb_x, track_y, 8.0, color::INK, 1.0);
                // 11 ticks
                for i in 0..=10 {
                    let tx = track.x + (i as f32 / 10.0) * track.w;
                    p.fill_circle(tx, track.bottom() + 4.0, 1.0, color::FAINT, 1.0);
                }

                // 2. Volume
                let vol = s.volume();
                let vol_str = value_text("mic.volume", &vol.to_string());
                p.text(
                    m::SETTING_MIC_VOLUME,
                    Rect::new(15.0, 106.0, 180.0, 18.0),
                    Font::Body,
                    color::TEXT,
                    Align::Left,
                    false,
                );
                p.text(
                    &vol_str,
                    Rect::new(pg.right() - 65.0, 106.0, 50.0, 18.0),
                    Font::Body,
                    color::INK,
                    Align::Right,
                    false,
                );

                let track_v = l::SLIDER_VOLUME_TRACK;
                let track_vy = track_v.y + track_v.h / 2.0;
                p.line(
                    (track_v.x, track_vy),
                    (track_v.right(), track_vy),
                    color::RULE,
                    4.0,
                );
                let v_frac = ((vol as f32 + 12.0) / 24.0).clamp(0.0, 1.0);
                let center_x = track_v.x + 0.5 * track_v.w;
                let v_thumb_x = track_v.x + v_frac * track_v.w;
                p.line((center_x, track_vy), (v_thumb_x, track_vy), color::INK, 4.0);
                p.fill_circle(v_thumb_x, track_vy, 8.0, color::INK, 1.0);
                // 9 ticks
                for i in 0..=8 {
                    let tx = track_v.x + (i as f32 / 8.0) * track_v.w;
                    let (tr, tc) = if i == 4 {
                        (1.5, color::TEXT2)
                    } else {
                        (1.0, color::FAINT)
                    };
                    p.fill_circle(tx, track_v.bottom() + 4.0, tr, tc, 1.0);
                }

                // Divider
                p.line((15.0, 168.0), (pg.right() - 15.0, 168.0), color::RULE, 1.0);

                // 3. Voice Isolation Switch
                let r_iso = l::TOGGLE_ISOLATION_ROW;
                p.text(
                    m::SETTING_MIC_ISOLATION,
                    Rect::new(15.0, r_iso.y + 6.0, 180.0, 18.0),
                    Font::Body,
                    color::TEXT,
                    Align::Left,
                    false,
                );
                p.text(
                    m::PC_VOICE_ISOLATION_DESC,
                    Rect::new(15.0, r_iso.y + 24.0, 220.0, 16.0),
                    Font::Caption,
                    color::TEXT2,
                    Align::Left,
                    false,
                );
                let sw_iso = l::TOGGLE_ISOLATION_SWITCH;
                let iso_on = s.voice_isolation();
                if iso_on {
                    p.fill_rounded(sw_iso, color::INK, 12.0);
                    p.stroke_rounded(sw_iso, color::CHIP_ON_EDGE, 1.0, 1.0, 12.0);
                    p.fill_circle(sw_iso.right() - 11.0, sw_iso.y + 12.0, 9.0, color::BG, 1.0);
                    p.icon(
                        icons::CHECK,
                        sw_iso.right() - 11.0,
                        sw_iso.y + 12.0,
                        10.0,
                        color::INK,
                    );
                } else {
                    p.fill_rounded(sw_iso, color::CHIP, 12.0);
                    p.stroke_rounded(sw_iso, color::CHIP_OFF_EDGE, 1.0, 1.0, 12.0);
                    p.fill_circle(sw_iso.x + 11.0, sw_iso.y + 12.0, 9.0, color::TEXT2, 1.0);
                }

                // 4. Auto Level Switch
                let r_al = l::TOGGLE_AUTOLEVEL_ROW;
                p.text(
                    m::SETTING_MIC_AUTO_LEVEL,
                    Rect::new(15.0, r_al.y + 6.0, 180.0, 18.0),
                    Font::Body,
                    color::TEXT,
                    Align::Left,
                    false,
                );
                p.text(
                    m::PC_AUTO_LEVEL_DESC,
                    Rect::new(15.0, r_al.y + 24.0, 220.0, 16.0),
                    Font::Caption,
                    color::TEXT2,
                    Align::Left,
                    false,
                );
                let sw_al = l::TOGGLE_AUTOLEVEL_SWITCH;
                let al_on = s.auto_level();
                if al_on {
                    p.fill_rounded(sw_al, color::INK, 12.0);
                    p.stroke_rounded(sw_al, color::CHIP_ON_EDGE, 1.0, 1.0, 12.0);
                    p.fill_circle(sw_al.right() - 11.0, sw_al.y + 12.0, 9.0, color::BG, 1.0);
                    p.icon(
                        icons::CHECK,
                        sw_al.right() - 11.0,
                        sw_al.y + 12.0,
                        10.0,
                        color::INK,
                    );
                } else {
                    p.fill_rounded(sw_al, color::CHIP, 12.0);
                    p.stroke_rounded(sw_al, color::CHIP_OFF_EDGE, 1.0, 1.0, 12.0);
                    p.fill_circle(sw_al.x + 11.0, sw_al.y + 12.0, 9.0, color::TEXT2, 1.0);
                }
            }
        }
    }

    fn draw_gate(&mut self) {
        let s = self.state.clone();
        let Connection::Approval { phone, code, .. } = &s.connection else {
            return;
        };
        let p = &self.painter;

        let g = l::GATE_BODY;
        p.fill_rounded(g, color::BG, 12.0);

        let title = fill(m::PC_APPROVE_TITLE, &[("phone", phone)]);
        p.text(
            &title,
            Rect::new(g.x + PAD, g.y + 40.0, g.w - 2.0 * PAD, 22.0),
            Font::Title,
            color::TEXT,
            Align::Center,
            false,
        );
        p.text(
            m::PC_APPROVE_CODE,
            Rect::new(g.x + PAD, g.y + 68.0, g.w - 2.0 * PAD, 18.0),
            Font::Caption,
            color::TEXT2,
            Align::Center,
            false,
        );
        p.text(
            code,
            Rect::new(g.x + PAD, g.y + 110.0, g.w - 2.0 * PAD, 48.0),
            Font::Code,
            color::TEXT,
            Align::Center,
            false,
        );

        p.fill_rounded(l::GATE_DENY, color::BG, 12.0);
        p.text(
            m::PC_DENY,
            l::GATE_DENY,
            Font::Title,
            color::TEXT2,
            Align::Center,
            false,
        );

        p.fill_rounded(l::GATE_ALLOW, color::ON, 12.0);
        p.text(
            m::PC_ALLOW,
            l::GATE_ALLOW,
            Font::Title,
            color::ON_CONTENT,
            Align::Center,
            false,
        );
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
        Link::Privacy => Some("https://owlmic.app/privacy".to_owned()),
        Link::Donate => Some("https://github.com/sponsors/diveshpatil9104".to_owned()),
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
