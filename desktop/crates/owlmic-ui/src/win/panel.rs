//! The panel window: 560 x 255 at 100% scale above the tray, never in the taskbar, closed by
//! Esc or by clicking elsewhere (SYSTEM_DESIGN section 33). It shows the newest state snapshot
//! the App Hub published, at most 60 times a second.

use super::draw::{Align, Font, Painter};
use super::tray::{self, Tray};
use crate::layout::{self as l, Rect, Screen, Target};
use crate::preview::{self, Preview};
use crate::view::{Action, Connection, Feature, Link, PanelState, Row, feature_text, indicator};
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
    rows: Vec<Row>,
    settings_open: bool,
    scroll: f32,
    scale: f32,
    visible: bool,
    hidden_at: Option<Instant>,
    applied_at: Instant,
    taskbar_created: u32,
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
            rows: Vec::new(),
            settings_open: false,
            scroll: 0.0,
            scale: 1.0,
            visible: false,
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
                if self.state.camera == Feature::On && !self.settings_open {
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
            WM_ACTIVATE if (wparam.0 & 0xFFFF) as u32 == WA_INACTIVE && self.visible => self.hide(),
            WM_KEYDOWN if wparam.0 as u16 == VK_ESCAPE.0 => self.hide(),
            WM_LBUTTONUP => self.click(
                lo(lparam.0) as f32 / self.scale,
                hi(lparam.0) as f32 / self.scale,
            ),
            WM_MOUSEWHEEL if self.settings_open => {
                let notches = hi(wparam.0 as isize) as f32 / WHEEL_DELTA as f32;
                let max = (l::list_height(self.rows.len()) - l::LIST.h).max(0.0);
                self.scroll = (self.scroll - notches * 3.0 * (l::ROW + 1.0)).clamp(0.0, max);
                self.invalidate();
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

    /// Takes the newest snapshot. A new approval request opens the panel without taking focus
    /// from a call (SYSTEM_DESIGN section 35).
    fn apply(&mut self) {
        self.applied_at = Instant::now();
        let new = self.ui.state.read();
        let asks = |s: &PanelState| matches!(s.connection, Connection::Approval { .. });
        let open_gate = asks(&new) && !asks(&self.state);
        self.state = new;
        self.rows = self.state.rows();
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
        self.settings_open = false;
        self.hidden_at = Some(Instant::now());
        self.ui.preview.set_wanted(false);
    }

    fn click(&mut self, x: f32, y: f32) {
        let screen = self.state.screen(self.settings_open);
        let Some(target) = l::hit(screen, x, y, self.scroll, self.rows.len()) else {
            return;
        };
        match target {
            Target::Settings => {
                self.settings_open = true;
                self.scroll = 0.0;
            }
            Target::Back => self.settings_open = false,
            _ => {
                if let Some(link) = self.state.link_at(target, &self.rows) {
                    open(link);
                } else if let Some(action) = self.state.click(target, &self.rows) {
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
            match self.state.screen(self.settings_open) {
                Screen::Gate => self.draw_gate(),
                Screen::Settings => self.draw_settings(),
                Screen::Main { repair } => self.draw_main(repair),
            }
            self.painter.end();
        }
        unsafe {
            let _ = EndPaint(self.hwnd, &ps);
        }
    }

    fn draw_main(&mut self, repair: bool) {
        let s = self.state.clone();
        let p = &mut self.painter;
        let c = l::CAMERA;
        p.fill(c, color::TILE);
        let (mid_x, mid_y) = (c.x + c.w / 2.0, c.y + c.h / 2.0);
        let lines = |p: &Painter, first: &str, second: &str| {
            p.text(
                first,
                Rect::new(c.x + PAD, mid_y + 8.0, c.w - 2.0 * PAD, 18.0),
                Font::Title,
                color::TEXT,
                Align::Center,
                false,
            );
            p.text(
                second,
                Rect::new(c.x + PAD, mid_y + 28.0, c.w - 2.0 * PAD, 17.0),
                Font::Body,
                color::TEXT2,
                Align::Center,
                false,
            );
        };
        if !s.connected() {
            p.icon(icons::VIDEO_OFF, mid_x, mid_y - 16.0, ICON, color::DISABLED);
            lines(p, m::PC_NO_PHONE, m::PC_OPEN_PHONE);
        } else if s.camera == Feature::On && self.ui.preview.seq() > 0 {
            let seq = self.ui.preview.seq();
            let preview = self.ui.preview.clone();
            p.picture(c, preview::WIDTH, preview::HEIGHT, seq, |copy| {
                preview.read(|px| copy(px))
            });
        } else {
            let icon = if s.camera == Feature::On {
                icons::VIDEO
            } else {
                icons::VIDEO_OFF
            };
            p.icon(icon, mid_x, mid_y - 16.0, ICON, color::TEXT2);
            let text = if s.camera == Feature::Paused {
                m::FEATURE_PAUSED
            } else {
                m::PC_CAMERA_OFF
            };
            p.text(
                text,
                Rect::new(c.x + PAD, mid_y + 8.0, c.w - 2.0 * PAD, 18.0),
                Font::Body,
                color::TEXT2,
                Align::Center,
                false,
            );
        }
        if repair {
            let r = l::REPAIR;
            p.fill(r, color::TILE);
            p.icon(
                icons::ALERT,
                r.x + PAD + ICON / 2.0,
                r.y + r.h / 2.0,
                ICON,
                color::YELLOW,
            );
            let button = l::repair_button();
            let text_x = r.x + 2.0 * PAD + ICON;
            p.text(
                s.repair_text(),
                Rect::new(text_x, r.y, button.x - PAD - text_x, r.h),
                Font::Body,
                color::TEXT,
                Align::Left,
                true,
            );
            let (bg, fg) = if s.repairing {
                (color::DISABLED, color::TEXT2)
            } else {
                (color::ON, color::ON_CONTENT)
            };
            p.fill(button, bg);
            p.text(m::PC_REPAIR, button, Font::Title, fg, Align::Center, false);
        }
        let connected = s.connected();
        feature_tile(p, l::MIC, icons::MIC, m::FEATURE_MIC, s.mic, connected);
        feature_tile(
            p,
            l::SPEAKER,
            icons::SPEAKER,
            m::FEATURE_SPEAKER,
            s.speaker,
            connected,
        );
        let ph = l::PHONE;
        p.fill(ph, color::TILE);
        let name = s.phone_name().unwrap_or(m::PC_NO_PHONE);
        p.text(
            name,
            Rect::new(ph.x + PAD, ph.y + PAD, ph.w - 2.0 * PAD, 18.0),
            Font::Title,
            color::TEXT,
            Align::Left,
            false,
        );
        p.text(
            s.phone_state(),
            Rect::new(ph.x + PAD, ph.y + PAD + 20.0, ph.w - 2.0 * PAD, 17.0),
            Font::Body,
            color::TEXT2,
            Align::Left,
            false,
        );
        let value = |id: &str| m::text(&format!("value.{}", s.value(id))).unwrap_or("");
        let dim = |on: bool| {
            if s.paired && on {
                color::TEXT
            } else {
                color::DISABLED
            }
        };
        let mirror_on = s.value("camera.mirror") == "on";
        let bottom: [(&str, bool); 4] = [
            (value("camera.framing"), false),
            (m::SETTING_CAMERA_MIRROR, mirror_on && s.paired),
            ("", false),
            (m::UI_SETTINGS, false),
        ];
        let lens = fill(m::PC_LENS, &[("lens", value("camera.lens"))]);
        for (i, (label, on)) in bottom.into_iter().enumerate() {
            let r = l::bottom(i);
            p.fill(r, if on { color::ON } else { color::TILE });
            let (label, fg) = match i {
                2 => (lens.as_str(), dim(true)),
                3 => (label, color::TEXT),
                _ if on => (label, color::ON_CONTENT),
                _ => (label, dim(true)),
            };
            p.text(
                label,
                r.inset(PAD / 2.0),
                Font::Body,
                fg,
                Align::Center,
                false,
            );
        }
        let ind = l::bottom(4);
        p.fill(ind, color::TILE);
        let i = indicator(&s.connection);
        p.icon(
            i.icon,
            ind.x + ind.w / 2.0,
            ind.y + ind.h / 2.0,
            ICON,
            i.color,
        );
    }

    fn draw_gate(&mut self) {
        let s = self.state.clone();
        let Connection::Approval { phone, code, .. } = &s.connection else {
            return;
        };
        let p = &self.painter;
        let g = l::GATE;
        p.fill(g, color::TILE);
        let title = fill(m::PC_APPROVE_TITLE, &[("phone", phone)]);
        p.text(
            &title,
            Rect::new(PAD, PAD, g.w - 2.0 * PAD, 18.0),
            Font::Title,
            color::TEXT,
            Align::Left,
            false,
        );
        p.text(
            m::PC_APPROVE_CODE,
            Rect::new(PAD, PAD + 20.0, g.w - 2.0 * PAD, 17.0),
            Font::Body,
            color::TEXT2,
            Align::Left,
            false,
        );
        p.text(
            code,
            Rect::new(PAD, 60.0, g.w - 2.0 * PAD, g.h - 72.0),
            Font::Code,
            color::TEXT,
            Align::Center,
            false,
        );
        p.fill(l::ALLOW, color::ON);
        p.text(
            m::PC_ALLOW,
            l::ALLOW,
            Font::Title,
            color::ON_CONTENT,
            Align::Center,
            false,
        );
        p.fill(l::DENY, color::TILE);
        p.text(
            m::PC_DENY,
            l::DENY,
            Font::Title,
            color::TEXT,
            Align::Center,
            false,
        );
    }

    fn draw_settings(&mut self) {
        let p = &self.painter;
        let b = l::BACK;
        p.fill(b, color::TILE);
        p.icon(
            icons::ARROW_LEFT,
            b.x + PAD + ICON / 2.0,
            b.y + b.h / 2.0,
            ICON,
            color::TEXT,
        );
        let title_x = 2.0 * PAD + ICON;
        p.text(
            m::UI_SETTINGS,
            Rect::new(title_x, b.y, b.w - title_x - PAD, b.h),
            Font::Title,
            color::TEXT,
            Align::Left,
            false,
        );
        p.clip(l::LIST);
        for (i, row) in self.rows.iter().enumerate() {
            let y = l::LIST.y + i as f32 * (l::ROW + 1.0) - self.scroll;
            if y + l::ROW < l::LIST.y || y > l::HEIGHT {
                continue;
            }
            let r = Rect::new(0.0, y, l::WIDTH, l::ROW);
            if row.header {
                p.fill(r, color::BG);
                p.text(
                    &row.label,
                    Rect::new(PAD, y, r.w - 2.0 * PAD, l::ROW),
                    Font::Caption,
                    color::TEXT2,
                    Align::Left,
                    false,
                );
                continue;
            }
            p.fill(r, color::TILE);
            let fg = if row.enabled {
                color::TEXT
            } else {
                color::DISABLED
            };
            let buttons = if row.buttons.is_some() {
                2.0 * l::ROW_BUTTON + 1.0
            } else {
                0.0
            };
            let half = (r.w - buttons) / 2.0;
            p.text(
                &row.label,
                Rect::new(PAD, y, half - PAD, l::ROW),
                Font::Body,
                fg,
                Align::Left,
                false,
            );
            p.text(
                &row.value,
                Rect::new(half, y, half - PAD, l::ROW),
                Font::Body,
                color::TEXT2,
                Align::Right,
                false,
            );
            if let Some(labels) = row.buttons {
                for (k, label) in labels.into_iter().enumerate() {
                    let bx = l::WIDTH - (2 - k) as f32 * (l::ROW_BUTTON + 1.0) + 1.0;
                    let br = Rect::new(bx, y, l::ROW_BUTTON, l::ROW);
                    p.fill(br, color::BG);
                    p.text(label, br, Font::Caption, color::TEXT, Align::Center, false);
                }
            }
        }
        p.unclip();
    }
}

fn feature_tile(
    p: &Painter,
    r: Rect,
    icon: crate::glyph::Icon,
    label: &str,
    state: Feature,
    connected: bool,
) {
    let (bg, fg) = match state {
        Feature::On if connected => (color::ON, color::ON_CONTENT),
        Feature::Paused | Feature::Restarting if connected => (color::TILE, color::TEXT),
        _ => (color::TILE, color::DISABLED),
    };
    p.fill(r, bg);
    p.icon(
        icon,
        r.x + PAD + ICON / 2.0,
        r.y + PAD + ICON / 2.0,
        ICON,
        fg,
    );
    p.text(
        label,
        Rect::new(r.x + PAD, r.bottom() - PAD - 37.0, r.w - 2.0 * PAD, 18.0),
        Font::Title,
        fg,
        Align::Left,
        false,
    );
    let state_text = feature_text(if connected { state } else { Feature::Off });
    p.text(
        state_text,
        Rect::new(r.x + PAD, r.bottom() - PAD - 17.0, r.w - 2.0 * PAD, 17.0),
        Font::Body,
        fg,
        Align::Left,
        false,
    );
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
