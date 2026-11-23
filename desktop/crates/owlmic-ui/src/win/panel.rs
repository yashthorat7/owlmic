//! The panel window: 312 x 374 at 100% scale above the tray, never in the taskbar, closed by
//! Esc or by clicking elsewhere (SYSTEM_DESIGN section 33). Visual layout, palette, and
//! interaction behavior mirror the PC prototype 1:1.

use super::draw::{Align, Font, Painter};
use super::tray::{self, Tray};
use crate::icons;
use crate::layout::{self as l, CropMode, Drawer, HelpScreen, Rect, Screen, Target, WifiMode};
use crate::preview::{self, Preview};
use crate::view::{Action, Connection, Feature, Link, PanelState, indicator};
use owlmic_hub::Publisher;
use std::sync::Arc;
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
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
    ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, VK_ESCAPE,
};
use windows::Win32::UI::Shell::{NIN_SELECT, ShellExecuteW};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, PCWSTR, Result, w};

const CLASS: PCWSTR = w!("OwlmicPanel");
const WM_APP_STATE: u32 = WM_APP + 1;
const WM_APP_OPEN: u32 = WM_APP + 3;
const WM_MOUSELEAVE: u32 = 0x02A3;
const TIMER_PREVIEW: usize = 1;
const TIMER_STATE: usize = 2;
const FRAME: Duration = Duration::from_millis(16);
/// A tray click that closed the panel by taking its focus shouldn't reopen it.
const REOPEN_GUARD: Duration = Duration::from_millis(300);
/// NIN_SELECT with the keyboard flag: the icon chosen with Enter or Space.
const NIN_KEYSELECT: u32 = NIN_SELECT | 1;
const PAD: f32 = 12.0;
const DOCK_ICON: f32 = 21.0;
const HUD_IDLE: Duration = Duration::from_secs(4);
const COPY_FLASH: Duration = Duration::from_millis(1400);
const SYNC_FLASH: Duration = Duration::from_millis(520);
const WAVE_BARS: usize = 25;

// Prototype monochrome palette (pre-blended on #000000 to match styles.css rgba values)
const BENTO_BG: u32 = 0x727272; // #727272 outer frame & 1 px grid gaps
const BLACK: u32 = 0x000000;
const WHITE: u32 = 0xFFFFFF;
const BTN_HOVER: u32 = 0x121212; // .pc-btn:hover
const BTN_ON_HOVER: u32 = 0xEDEDED; // .pc-btn.is-on:hover
const ROW_HOVER: u32 = 0x0D0D0D; // hover:bg-white/5
const WHITE_10: u32 = 0x1A1A1A; // borders, decline bg, active transport bg
const WHITE_14: u32 = 0x242424; // fader track, off transport border
const WHITE_15: u32 = 0x262626; // drawer headers, help tiles, audio panel top border
const WHITE_16: u32 = 0x292929; // --line (transport footer & divider)
const WHITE_20: u32 = 0x333333; // cam btn border, how-to spine, refresh border
const WHITE_22: u32 = 0x383838; // closed cam menu border, crop seam
const WHITE_24: u32 = 0x3D3D3D; // on transport border
const WHITE_25: u32 = 0x404040; // offline row icon
const WHITE_28: u32 = 0x474747; // --line-hi, disconnected media icons, docked wave border
const WHITE_30: u32 = 0x4D4D4D; // help tile hover border
const WHITE_32: u32 = 0x525252; // expanded docked wave border
const WHITE_35: u32 = 0x595959; // muted meta: IP, timestamps, version, Remove
const WHITE_36: u32 = 0x5C5C5C; // open cam menu border, muted norm row
const WHITE_40: u32 = 0x666666; // copy icon, how-to circle border
const WHITE_42: u32 = 0x6B6B6B; // off transport icon
const WHITE_45: u32 = 0x737373; // Block text, switch off knob
const WHITE_50: u32 = 0x808080; // offline phone name, connected IP, Forget, @handle, decline X
const WHITE_55: u32 = 0x8C8C8C; // how-to descriptions
const WHITE_60: u32 = 0x999999; // privacy body, nr/vol numeric values
const WHITE_65: u32 = 0xA6A6A6; // disconnect icon, unselected cam option
const WHITE_70: u32 = 0xB3B3B3; // help tile labels, "Wants to pair"
const WHITE_78: u32 = 0xC7C7C7; // bottom dock off icons
const WHITE_80: u32 = 0xCCCCCC; // refresh icon
const _WHITE_85: u32 = 0xD9D9D9; // pairing icon
const WHITE_90: u32 = 0xE6E6E6; // host PC name

const CREDITS: [(&str, &str); 5] = [
    ("Divesh Patil", "diveshpatil9104"),
    ("Yash Thorat", "yashthorat7"),
    ("Vedant Newadkar", "vedantnewadkar"),
    ("Vaibhav Sonawane", "Vaibhav-Sonawane105"),
    ("aipd506", "aipd506"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DragFader {
    Nr,
    Vol,
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
    pub meter: Arc<AtomicU32>,
    pub on_action: Box<dyn Fn(Action)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnimState {
    Closed,
    Opening,
    Open,
    Closing,
}

struct Panel {
    hwnd: HWND,
    ui: Ui,
    painter: Painter,
    tray: Option<Tray>,
    state: Arc<PanelState>,
    drawer: Drawer,
    cam_hud: bool,
    cam_hud_at: Option<Instant>,
    cam_menu_open: bool,
    mic_quick_open: bool,
    dock_open: bool,
    hover: Option<Target>,
    dragging: Option<DragFader>,
    scale: f32,
    visible: bool,
    hidden_at: Option<Instant>,
    shown_at: Option<Instant>,
    last_tray_click: Instant,
    applied_at: Instant,
    copied_at: Option<Instant>,
    refresh_at: Option<Instant>,
    last_connected_at: Option<Instant>,
    sync_flash_at: Option<Instant>,
    taskbar_created: u32,
    anim_frame: u64,
    anim_state: AnimState,
    anim_start: Instant,
    base_pos: (i32, i32),
    is_mouse_over: bool,
    unhover_since: Option<Instant>,
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
            cam_hud: false,
            cam_hud_at: None,
            cam_menu_open: false,
            mic_quick_open: false,
            dock_open: false,
            hover: None,
            dragging: None,
            scale: 1.0,
            visible: false,
            hidden_at: None,
            shown_at: None,
            last_tray_click: Instant::now() - Duration::from_secs(1),
            applied_at: Instant::now(),
            copied_at: None,
            refresh_at: None,
            last_connected_at: None,
            sync_flash_at: None,
            taskbar_created: RegisterWindowMessageW(w!("TaskbarCreated")),
            anim_frame: 0,
            anim_state: AnimState::Closed,
            anim_start: Instant::now(),
            base_pos: (0, 0),
            is_mouse_over: false,
            unhover_since: None,
        });
        let raw = Box::into_raw(panel);
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_LAYERED,
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
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA);
        // Windows 11 rounds the outer window corners and draws a hairline border.
        let corners = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&corners as *const DWM_WINDOW_CORNER_PREFERENCE).cast(),
            size_of_val(&corners) as u32,
        );
        let border = BENTO_BG.swap_bytes() >> 8;
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

fn gray(alpha: f32) -> u32 {
    let c = (alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
    (c << 16) | (c << 8) | c
}

fn dim_color(rgb: u32, factor: f32) -> u32 {
    let f = factor.clamp(0.0, 1.0);
    let r = (((rgb >> 16) & 0xFF) as f32 * f).round() as u32;
    let g = (((rgb >> 8) & 0xFF) as f32 * f).round() as u32;
    let b = ((rgb & 0xFF) as f32 * f).round() as u32;
    (r << 16) | (g << 8) | b
}

impl Panel {
    fn cam_active(&self) -> bool {
        matches!(self.state.camera, Feature::On | Feature::Restarting) && self.state.connected()
    }

    fn mic_active(&self) -> bool {
        matches!(self.state.mic, Feature::On | Feature::Restarting) && self.state.connected()
    }

    fn active_quick_audio(&self) -> bool {
        if !self.mic_active() {
            return false;
        }
        if self.cam_active() {
            self.dock_open
        } else {
            self.mic_quick_open
        }
    }

    fn current_screen(&self) -> Screen {
        self.state
            .active_screen(self.drawer, self.cam_menu_open, self.active_quick_audio())
    }

    fn hud_is_bright(&self) -> bool {
        self.cam_hud
            || self.cam_menu_open
            || (self.cam_active() && self.mic_active() && self.dock_open)
    }

    fn touch_cam_hud(&mut self) {
        self.cam_hud = true;
        self.cam_hud_at = Some(Instant::now());
    }

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
                self.anim_frame = self.anim_frame.wrapping_add(1);

                match self.anim_state {
                    AnimState::Opening => {
                        let elapsed = self.anim_start.elapsed().as_secs_f32();
                        let dur = 0.260;
                        let t = (elapsed / dur).clamp(0.0, 1.0);
                        let ease = 1.0 - (1.0 - t).powi(3);
                        let offset_y = ((1.0 - ease) * 32.0 * self.scale).round() as i32;
                        let alpha = (t.min(0.200 / 0.260) / (0.200 / 0.260) * 255.0).round() as u8;
                        unsafe {
                            let _ = SetLayeredWindowAttributes(
                                self.hwnd,
                                COLORREF(0),
                                alpha,
                                LWA_ALPHA,
                            );
                            let _ = SetWindowPos(
                                self.hwnd,
                                None,
                                self.base_pos.0,
                                self.base_pos.1 + offset_y,
                                0,
                                0,
                                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                            );
                        }
                        if t >= 1.0 {
                            self.anim_state = AnimState::Open;
                            unsafe {
                                let _ = SetLayeredWindowAttributes(
                                    self.hwnd,
                                    COLORREF(0),
                                    255,
                                    LWA_ALPHA,
                                );
                                let _ = SetWindowPos(
                                    self.hwnd,
                                    None,
                                    self.base_pos.0,
                                    self.base_pos.1,
                                    0,
                                    0,
                                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                                );
                            }
                        }
                    }
                    AnimState::Closing => {
                        let elapsed = self.anim_start.elapsed().as_secs_f32();
                        let dur = 0.200;
                        let t = (elapsed / dur).clamp(0.0, 1.0);
                        let ease = t.powi(3);
                        let offset_y = (ease * 32.0 * self.scale).round() as i32;
                        let alpha = ((1.0 - t) * 255.0).round() as u8;
                        unsafe {
                            let _ = SetLayeredWindowAttributes(
                                self.hwnd,
                                COLORREF(0),
                                alpha,
                                LWA_ALPHA,
                            );
                            let _ = SetWindowPos(
                                self.hwnd,
                                None,
                                self.base_pos.0,
                                self.base_pos.1 + offset_y,
                                0,
                                0,
                                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                            );
                        }
                        if t >= 1.0 {
                            self.finish_hide();
                            return Some(LRESULT(0));
                        }
                    }
                    AnimState::Open => {
                        // 1. Taskbar check: if taskbar is gone or hidden, close panel
                        unsafe {
                            let tray = FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null());
                            if tray.is_err()
                                || tray.as_ref().map_or(true, |h| {
                                    h.0.is_null() || !IsWindowVisible(*h).as_bool()
                                })
                            {
                                self.hide();
                                return Some(LRESULT(0));
                            }
                        }
                        // 2. Unhover check: if mouse has left the window for >= 2 seconds, close panel
                        if let Some(since) = self.unhover_since
                            && since.elapsed() >= Duration::from_secs(2)
                        {
                            self.hide();
                            return Some(LRESULT(0));
                        }
                    }
                    AnimState::Closed => {}
                }

                // Auto-dim camera HUD after 4s idle (matching CFG.hudIdleMs = 4000)
                if self.cam_hud
                    && !self.cam_menu_open
                    && !(self.mic_active() && self.dock_open)
                    && self.cam_hud_at.is_some_and(|t| t.elapsed() >= HUD_IDLE)
                {
                    self.cam_hud = false;
                    self.cam_hud_at = None;
                }
                if self.visible {
                    self.invalidate();
                }
            }
            WM_APP_OPEN => self.show(true),
            tray::CALLBACK => {
                let event = (lparam.0 & 0xFFFF) as u32;
                if [WM_LBUTTONUP, NIN_SELECT, NIN_KEYSELECT].contains(&event)
                    && self.last_tray_click.elapsed() >= Duration::from_millis(250)
                {
                    self.last_tray_click = Instant::now();
                    if self.visible {
                        self.hide();
                    } else if self.hidden_at.is_none_or(|t| t.elapsed() > REOPEN_GUARD) {
                        self.show(true);
                    }
                } else if [WM_RBUTTONUP, WM_CONTEXTMENU].contains(&event) {
                    if self.visible {
                        self.hide();
                    }
                    self.show_tray_context_menu();
                }
            }
            WM_ACTIVATE if (wparam.0 & 0xFFFF) as u32 == WA_INACTIVE && self.visible => {
                self.hover = None;
                self.dragging = None;
                if std::env::var("OWLMIC_NO_AUTO_HIDE").is_err()
                    && self
                        .shown_at
                        .is_none_or(|t| t.elapsed() > Duration::from_millis(250))
                {
                    self.hide();
                }
            }
            WM_KEYDOWN if wparam.0 as u16 == VK_ESCAPE.0 => self.hide(),
            WM_LBUTTONDOWN => {
                let x = lo(lparam.0) as f32 / self.scale;
                let y = hi(lparam.0) as f32 / self.scale;
                self.mouse_down(x, y);
            }
            WM_MOUSEMOVE => {
                if !self.is_mouse_over {
                    self.is_mouse_over = true;
                    self.unhover_since = None;
                    let mut tme = TRACKMOUSEEVENT {
                        cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: self.hwnd,
                        dwHoverTime: 0,
                    };
                    unsafe {
                        let _ = TrackMouseEvent(&mut tme);
                    }
                } else {
                    self.unhover_since = None;
                }
                let x = lo(lparam.0) as f32 / self.scale;
                let y = hi(lparam.0) as f32 / self.scale;
                let lbtn_down = (wparam.0 & 0x0001) != 0;
                self.mouse_move(x, y, lbtn_down);
            }
            WM_MOUSELEAVE => {
                self.is_mouse_over = false;
                self.unhover_since = Some(Instant::now());
                self.hover = None;
                self.invalidate();
            }
            WM_LBUTTONUP => {
                let x = lo(lparam.0) as f32 / self.scale;
                let y = hi(lparam.0) as f32 / self.scale;
                if self.dragging.take().is_some() {
                    unsafe {
                        let _ = ReleaseCapture();
                    }
                    self.invalidate();
                } else {
                    self.click(x, y);
                }
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
        let was_conn = self.state.connected();
        let now_conn = new.connected();
        let asks = |s: &PanelState| matches!(s.connection, Connection::Approval { .. });
        let open_pairing = asks(&new) && !asks(&self.state);

        if was_conn && !now_conn {
            self.last_connected_at = Some(Instant::now());
        }
        if !was_conn && now_conn {
            self.sync_flash_at = Some(Instant::now());
        }
        if !now_conn {
            self.cam_hud = false;
            self.cam_hud_at = None;
            self.cam_menu_open = false;
            self.mic_quick_open = false;
            self.dock_open = false;
        }
        if !matches!(new.camera, Feature::On | Feature::Restarting) {
            self.cam_hud = false;
            self.cam_hud_at = None;
            self.cam_menu_open = false;
            self.dock_open = false;
        }
        if !matches!(new.mic, Feature::On | Feature::Restarting) {
            self.mic_quick_open = false;
            self.dock_open = false;
        }

        self.state = new;
        let tip = self.state.tooltip();
        if let Some(t) = self.tray.as_mut() {
            t.set_tip(&tip);
        }
        if open_pairing {
            self.drawer = Drawer::Wifi;
            self.cam_menu_open = false;
            self.mic_quick_open = false;
            self.dock_open = false;
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
        self.shown_at = Some(Instant::now());
        let mut cursor = POINT::default();
        let has_cursor = unsafe { GetCursorPos(&mut cursor) }.is_ok();
        let mut anchor = self.tray.as_ref().and_then(Tray::rect);
        if let Some(r) = anchor {
            let invalid = r.right <= r.left || r.bottom <= r.top;
            let cursor_outside = has_cursor
                && (cursor.y < r.top - 24
                    || cursor.y > r.bottom + 24
                    || cursor.x < r.left - 24
                    || cursor.x > r.right + 24);
            if invalid || cursor_outside {
                anchor = None;
            }
        }
        let anchor = anchor.unwrap_or_else(|| {
            let p = if has_cursor {
                cursor
            } else {
                let mut p = POINT::default();
                unsafe {
                    let _ = GetCursorPos(&mut p);
                }
                p
            };
            RECT {
                left: p.x - 12,
                top: p.y - 12,
                right: p.x + 12,
                bottom: p.y + 12,
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
        self.base_pos = (x, y);
        self.anim_state = AnimState::Opening;
        self.anim_start = Instant::now();
        self.is_mouse_over = true;
        self.unhover_since = None;

        let flags = if activate {
            SWP_SHOWWINDOW
        } else {
            SWP_SHOWWINDOW | SWP_NOACTIVATE
        };
        let start_y = y + (32.0 * self.scale).round() as i32;
        unsafe {
            let _ = SetLayeredWindowAttributes(self.hwnd, COLORREF(0), 0, LWA_ALPHA);
            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                x,
                start_y,
                size.0,
                size.1,
                flags,
            );
            let _ = ShowWindow(self.hwnd, SW_SHOW);
            if activate {
                let _ = SetForegroundWindow(self.hwnd);
            }
            SetTimer(Some(self.hwnd), TIMER_PREVIEW, 16, None);
        }
        self.visible = true;
        self.ui.preview.set_wanted(true);
        self.invalidate();
    }

    fn hide(&mut self) {
        if self.anim_state == AnimState::Open || self.anim_state == AnimState::Opening {
            self.anim_state = AnimState::Closing;
            self.anim_start = Instant::now();
            unsafe {
                SetTimer(Some(self.hwnd), TIMER_PREVIEW, 16, None);
            }
        } else if self.anim_state == AnimState::Closing {
            // Already in closing animation
        } else {
            self.finish_hide();
        }
    }

    fn finish_hide(&mut self) {
        self.anim_state = AnimState::Closed;
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
            let _ = KillTimer(Some(self.hwnd), TIMER_PREVIEW);
        }
        self.visible = false;
        self.drawer = Drawer::None;
        self.cam_hud = false;
        self.cam_hud_at = None;
        self.cam_menu_open = false;
        self.mic_quick_open = false;
        self.dock_open = false;
        self.hover = None;
        self.dragging = None;
        self.is_mouse_over = false;
        self.unhover_since = None;
        self.hidden_at = Some(Instant::now());
        self.shown_at = None;
        self.ui.preview.set_wanted(false);
    }

    fn show_tray_context_menu(&mut self) {
        unsafe {
            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            let _ = SetForegroundWindow(self.hwnd);

            let Ok(menu) = CreatePopupMenu() else {
                return;
            };

            let open_label = HSTRING::from(format!(
                "{} {}",
                crate::messages::VALUE_OPEN,
                crate::names::PRODUCT
            ));
            let quit_label = HSTRING::from(crate::messages::PC_QUIT);

            const ID_OPEN: usize = 1;
            const ID_QUIT: usize = 2;

            let _ = AppendMenuW(
                menu,
                MF_STRING,
                ID_OPEN,
                PCWSTR::from_raw(open_label.as_ptr()),
            );
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                ID_QUIT,
                PCWSTR::from_raw(quit_label.as_ptr()),
            );

            let _ = SetMenuDefaultItem(menu, ID_OPEN as u32, 0);

            let cmd = TrackPopupMenu(
                menu,
                TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_RETURNCMD,
                pt.x,
                pt.y,
                None,
                self.hwnd,
                None,
            );

            let _ = DestroyMenu(menu);
            let _ = PostMessageW(Some(self.hwnd), WM_NULL, WPARAM(0), LPARAM(0));

            if cmd.0 == ID_OPEN as i32 {
                if !self.visible {
                    self.show(true);
                }
            } else if cmd.0 == ID_QUIT as i32 {
                (self.ui.on_action)(Action::Quit);
            }
        }
    }

    fn apply_fader_drag(&mut self, which: DragFader, x: f32) {
        let panel = l::active_audio_panel(self.cam_active());
        let ac = l::audio_controls(panel);
        match which {
            DragFader::Nr => {
                let ratio = ((x - ac.nr_fader.x) / ac.nr_fader.w).clamp(0.0, 1.0);
                if let Some(action) = self.state.click(Target::NrFader(ratio * 10.0)) {
                    (self.ui.on_action)(action);
                }
            }
            DragFader::Vol => {
                let ratio = ((x - ac.vol_fader.x) / ac.vol_fader.w).clamp(0.0, 1.0);
                if let Some(action) = self.state.click(Target::VolFader(ratio * 100.0)) {
                    (self.ui.on_action)(action);
                }
            }
        }
    }

    fn mouse_down(&mut self, x: f32, y: f32) {
        let screen = self.current_screen();
        if let Some(target) = l::hit(screen, x, y) {
            match target {
                Target::NrFader(_) => {
                    self.dragging = Some(DragFader::Nr);
                    unsafe {
                        SetCapture(self.hwnd);
                    }
                    self.apply_fader_drag(DragFader::Nr, x);
                    self.invalidate();
                }
                Target::VolFader(_) => {
                    self.dragging = Some(DragFader::Vol);
                    unsafe {
                        SetCapture(self.hwnd);
                    }
                    self.apply_fader_drag(DragFader::Vol, x);
                    self.invalidate();
                }
                _ => {}
            }
        }
    }

    fn mouse_move(&mut self, x: f32, y: f32, lbtn_down: bool) {
        if let Some(which) = self.dragging {
            if !lbtn_down {
                self.dragging = None;
                unsafe {
                    let _ = ReleaseCapture();
                }
            } else {
                self.apply_fader_drag(which, x);
            }
            self.invalidate();
            return;
        }
        let mut next_hover = if (0.0..l::WIDTH).contains(&x) && (0.0..l::HEIGHT).contains(&y) {
            l::hit(self.current_screen(), x, y)
        } else {
            None
        };
        if next_hover == Some(Target::RemovePaired) && self.state.secondary_phone().is_none() {
            next_hover = None;
        }
        if next_hover != self.hover {
            self.hover = next_hover;
            self.invalidate();
        }
    }

    fn is_hovered(&self, target: Target) -> bool {
        self.hover == Some(target)
    }

    fn click(&mut self, x: f32, y: f32) {
        let screen = self.current_screen();
        let Some(target) = l::hit(screen, x, y) else {
            if self.cam_menu_open {
                self.cam_menu_open = false;
                self.touch_cam_hud();
                self.invalidate();
            }
            return;
        };
        match target {
            Target::Wifi => {
                if self.drawer == Drawer::Wifi {
                    self.drawer = Drawer::None;
                } else {
                    self.drawer = Drawer::Wifi;
                    self.cam_menu_open = false;
                }
            }
            Target::Help => {
                if matches!(self.drawer, Drawer::Help(_)) {
                    self.drawer = Drawer::None;
                } else {
                    self.drawer = Drawer::Help(HelpScreen::Bento);
                    self.cam_menu_open = false;
                }
            }
            Target::Camera | Target::Mic | Target::Speaker => {
                self.drawer = Drawer::None;
                if self.state.connected()
                    && let Some(action) = self.state.click(target)
                {
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
            Target::HelpReleases => {
                open_url(
                    "[https://github.com/diveshpatil9104/owlmic/releases](https://github.com/diveshpatil9104/owlmic/releases)",
                );
            }
            Target::HelpCredit(idx) => {
                if let Some(&(_, user)) = CREDITS.get(idx) {
                    open_url(&format!("[https://github.com/](https://github.com/){user}"));
                }
            }
            Target::CamCanvas => {
                if self.cam_menu_open {
                    self.cam_menu_open = false;
                    self.touch_cam_hud();
                } else if self.cam_hud {
                    self.cam_hud = false;
                    self.cam_hud_at = None;
                } else {
                    self.touch_cam_hud();
                }
            }
            Target::CamMenu => {
                self.cam_menu_open = !self.cam_menu_open;
                self.touch_cam_hud();
                if self.cam_menu_open {
                    self.dock_open = false;
                }
            }
            Target::Quality(q) => {
                // Prototype behavior: clicking already-selected quality closes menu;
                // clicking a different quality keeps menu open.
                if self.state.camera_quality_label() == q {
                    self.cam_menu_open = false;
                } else if let Some(action) = self.state.click(target) {
                    (self.ui.on_action)(action);
                }
                self.touch_cam_hud();
            }
            Target::Fps(_) | Target::Ratio(_) => {
                // Prototype keeps dropdown open when changing FPS or Aspect Ratio.
                self.touch_cam_hud();
                if let Some(action) = self.state.click(target) {
                    (self.ui.on_action)(action);
                }
            }
            Target::Mirror | Target::Flip => {
                self.cam_menu_open = false;
                self.touch_cam_hud();
                if let Some(action) = self.state.click(target) {
                    (self.ui.on_action)(action);
                }
            }
            Target::WaveDock => {
                if self.cam_active() {
                    self.dock_open = !self.dock_open;
                    if self.dock_open {
                        self.cam_menu_open = false;
                        self.cam_hud = true;
                        self.cam_hud_at = None;
                    } else {
                        self.touch_cam_hud();
                    }
                } else {
                    self.mic_quick_open = !self.mic_quick_open;
                }
            }
            Target::CopyIp => {
                let ip_str = self.state.host_ip();
                copy_to_clipboard(self.hwnd, &ip_str);
                self.copied_at = Some(Instant::now());
            }
            Target::Refresh => {
                self.refresh_at = Some(Instant::now());
            }
            Target::RemovePaired => {
                if self.state.secondary_phone().is_some()
                    && let Some(action) = self.state.click(target)
                {
                    (self.ui.on_action)(action);
                }
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
            self.paint_frame();
            self.painter.end();
        }
        unsafe {
            let _ = EndPaint(self.hwnd, &ps);
        }
    }

    fn paint_frame(&mut self) {
        // #727272 base shows through the 1 px outer perimeter, 1 px gaps, and 12 px tile corners
        self.painter.clear(BENTO_BG);
        let screen = self.current_screen();
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
    }

    fn draw_playground_base(&mut self) {
        self.painter
            .fill_rounded(l::PLAYGROUND, l::TILE_RADIUS, BLACK);
    }

    /// Draws the 25-bar pure-white waveform matching scripts.js / styles.css exact dimensions
    /// and cosine envelope motion.
    fn draw_waveform_bars(&mut self, center_x: f32, center_y: f32, docked: bool) {
        let bar_w = if docked { 3.0 } else { 4.0 };
        let gap = if docked { 3.5 } else { 4.5 };
        let min_h = if docked { 3.0 } else { 4.0 };
        let max_h = if docked { 20.0 } else { 60.0 };
        let total_w = WAVE_BARS as f32 * bar_w + (WAVE_BARS - 1) as f32 * gap;
        let start_x = center_x - total_w / 2.0;

        let raw_level = f32::from_bits(self.ui.meter.load(Ordering::Relaxed));
        let loud = {
            const WAVE_THRESHOLD: f32 = 0.24;
            const WAVE_GAIN: f32 = 1.2;
            let v = ((raw_level - WAVE_THRESHOLD) / (1.0 - WAVE_THRESHOLD)).clamp(0.0, 1.0);
            (v * v * (1.4 - 0.4 * v) * WAVE_GAIN).min(1.0)
        };
        let wt = self.anim_frame as f32 * 0.055;
        let mid = (WAVE_BARS as f32 - 1.0) / 2.0;

        for i in 0..WAVE_BARS {
            let env = (((i as f32 - mid).abs() / mid) * std::f32::consts::PI * 0.46)
                .cos()
                .max(0.2);
            let mot = ((wt * 5.2 + i as f32 * 0.52).sin()
                + (wt * 9.1 + i as f32 * 1.1).sin() * 0.5
                + (wt * 14.0 + i as f32 * 0.28).cos() * 0.25)
                / 1.75;
            let fraction = (loud * 0.95 + (mot + 1.0) * 0.14 * (0.2 + loud)) * env;
            let norm_h = fraction.clamp(0.08, 1.0);
            let h = min_h + (max_h - min_h) * norm_h;
            let x = start_x + i as f32 * (bar_w + gap);
            let y = center_y - h / 2.0;
            self.painter
                .fill_rounded(Rect::new(x, y, bar_w, h), bar_w / 2.0, WHITE);
        }
    }

    fn draw_owl_switch(&mut self, rect: Rect, on: bool, dimmed: bool) {
        let p = &mut self.painter;
        let r = rect.h / 2.0;
        let (bg, border, knob, knob_x) = if on {
            (
                if dimmed { WHITE_36 } else { WHITE },
                if dimmed { WHITE_36 } else { WHITE },
                BLACK,
                rect.x + rect.w - 3.0 - 12.0,
            )
        } else {
            (
                BLACK,
                if dimmed { WHITE_14 } else { WHITE_28 },
                if dimmed { WHITE_20 } else { WHITE_45 },
                rect.x + 3.0,
            )
        };
        p.fill_rounded(rect, r, bg);
        p.stroke_rounded(rect, r, border, 1.0);
        p.circle(knob_x + 6.0, rect.y + rect.h / 2.0, 6.0, knob);
    }

    fn draw_hardware_fader(&mut self, rect: Rect, value: f32, live: Option<f32>, active: bool) {
        let p = &mut self.painter;
        let track_h = if active { 9.0 } else { 7.0 };
        let track_y = rect.y + (rect.h - track_h) / 2.0;
        let track = Rect::new(rect.x, track_y, rect.w, track_h);
        p.fill_rounded(track, track_h / 2.0, WHITE_14);

        if let Some(lvl) = live {
            let lw = (rect.w * lvl.clamp(0.0, 1.0)).max(0.0);
            if lw > 1.0 {
                p.fill_rounded(Rect::new(rect.x, track_y, lw, track_h), 3.0, WHITE_28);
            }
        }

        let fill_w = (rect.w * value.clamp(0.0, 1.0)).max(0.0);
        if fill_w > 1.0 {
            p.fill_rounded(Rect::new(rect.x, track_y, fill_w, track_h), 3.0, WHITE);
        }

        // White vertical cap with 1.5 px black halo (matching .fader-cap box-shadow)
        let cap_h = if active { 16.0 } else { 13.0 };
        let cap_x = (rect.x + fill_w).clamp(rect.x + 1.5, rect.right() - 1.5);
        let cap_y = rect.y + (rect.h - cap_h) / 2.0;
        p.fill_rounded(
            Rect::new(cap_x - 2.75, cap_y - 1.5, 5.5, cap_h + 3.0),
            2.5,
            BLACK,
        );
        p.fill_rounded(Rect::new(cap_x - 1.25, cap_y, 2.5, cap_h), 1.25, WHITE);
    }

    fn draw_audio_controls(&mut self, panel: Rect) {
        let s = self.state.clone();
        let ac = l::audio_controls(panel);
        let iso_on = s.is_on("mic.isolation", true);
        let norm_on = s.is_on("mic.autoLevel", true);
        let nr_val = s.mic_nr_value();
        let vol_val = s.mic_volume_percent();
        let norm_muted = vol_val <= 0.0;

        self.painter.text(
            "Voice isolation",
            ac.iso_label,
            Font::Body,
            WHITE,
            Align::Left,
            false,
        );
        self.draw_owl_switch(ac.iso_switch, iso_on, false);

        self.painter.text(
            "Normalization",
            ac.norm_label,
            Font::Body,
            if norm_muted { WHITE_36 } else { WHITE },
            Align::Left,
            false,
        );
        self.draw_owl_switch(ac.norm_switch, norm_on, norm_muted);

        self.painter.text(
            "Noise reduction",
            ac.nr_label,
            Font::Body,
            WHITE,
            Align::Left,
            false,
        );
        self.painter.text(
            &format!("{:.0}", nr_val),
            ac.nr_value,
            Font::Caption,
            WHITE_60,
            Align::Right,
            false,
        );
        self.draw_hardware_fader(
            ac.nr_fader,
            nr_val / 10.0,
            None,
            self.dragging == Some(DragFader::Nr),
        );

        self.painter.text(
            "Volume",
            ac.vol_label,
            Font::Body,
            WHITE,
            Align::Left,
            false,
        );
        let vol_db = s.mic_volume_db();
        let vol_str = if vol_db > 0 {
            format!("+{} dB", vol_db)
        } else {
            format!("{} dB", vol_db)
        };
        self.painter.text(
            &vol_str,
            ac.vol_value,
            Font::Caption,
            WHITE_60,
            Align::Right,
            false,
        );
        let vf = vol_val / 100.0;
        let raw_level = f32::from_bits(self.ui.meter.load(Ordering::Relaxed));
        self.draw_hardware_fader(
            ac.vol_fader,
            vf,
            Some(raw_level),
            self.dragging == Some(DragFader::Vol),
        );
    }

    fn draw_main(&mut self, cam_on: bool, mic_on: bool, cam_menu: bool, quick_audio: bool) {
        let s = self.state.clone();
        self.draw_playground_base();
        let c = l::PLAYGROUND;
        let (mid_x, mid_y) = (c.x + c.w / 2.0, c.y + c.h / 2.0);

        if cam_on {
            // Optical crop frame matching selected aspect ratio
            let ratio_lbl = s.camera_ratio_label();
            let (cam_rect, crop_mode) = l::cam_crop_rect(ratio_lbl);

            if self.ui.preview.seq() > 0 {
                let seq = self.ui.preview.seq();
                let preview = self.ui.preview.clone();
                self.painter
                    .picture(cam_rect, preview::WIDTH, preview::HEIGHT, seq, |copy| {
                        preview.read(|px| copy(px))
                    });
            }

            // 1 px crop seam border matching .cam-frame[data-crop]
            match crop_mode {
                CropMode::Pillar => {
                    self.painter.line(
                        cam_rect.x,
                        cam_rect.y,
                        cam_rect.x,
                        cam_rect.bottom(),
                        WHITE_22,
                        1.0,
                    );
                    self.painter.line(
                        cam_rect.right(),
                        cam_rect.y,
                        cam_rect.right(),
                        cam_rect.bottom(),
                        WHITE_22,
                        1.0,
                    );
                }
                CropMode::Letter => {
                    self.painter.line(
                        cam_rect.x,
                        cam_rect.y,
                        cam_rect.right(),
                        cam_rect.y,
                        WHITE_22,
                        1.0,
                    );
                    self.painter.line(
                        cam_rect.x,
                        cam_rect.bottom(),
                        cam_rect.right(),
                        cam_rect.bottom(),
                        WHITE_22,
                        1.0,
                    );
                }
                CropMode::None => {}
            }

            // Camera HUD (dims to 15% when quiet, matching .cam-hud.is-quiet)
            let hud_mul = if self.hud_is_bright() { 1.0 } else { 0.18 };

            if cam_menu {
                self.painter.fill_rounded(l::CAM_EXPAND_MENU, 7.0, BLACK);

                let cur_q = s.camera_quality_label();
                let cur_fps = s.camera_fps_label();
                let cur_ratio = s.camera_ratio_label();

                let row0 = ["FHD", "HD", "480p"];
                let row1 = ["60", "30", "24"];
                let row2 = ["Auto", "16:9", "4:3"];
                let row3 = ["1:1", "3:4", "9:16"];

                for (col, &val) in row0.iter().enumerate() {
                    let r = l::cam_option_rect(0, col);
                    let sel = cur_q == val;
                    let hov = self.is_hovered(Target::Quality(val));
                    if sel {
                        self.painter.fill(r, WHITE);
                    } else if hov {
                        self.painter.fill(r, ROW_HOVER);
                    }
                    self.painter.text(
                        val,
                        r,
                        Font::Caption,
                        if sel {
                            BLACK
                        } else if hov {
                            WHITE
                        } else {
                            WHITE_65
                        },
                        Align::Center,
                        false,
                    );
                }
                for (col, &val) in row1.iter().enumerate() {
                    let r = l::cam_option_rect(1, col);
                    let sel = cur_fps == val;
                    let hov = self.is_hovered(Target::Fps(val));
                    if sel {
                        self.painter.fill(r, WHITE);
                    } else if hov {
                        self.painter.fill(r, ROW_HOVER);
                    }
                    let lbl = format!("{val}fps");
                    self.painter.text(
                        &lbl,
                        r,
                        Font::Caption,
                        if sel {
                            BLACK
                        } else if hov {
                            WHITE
                        } else {
                            WHITE_65
                        },
                        Align::Center,
                        false,
                    );
                }
                for (col, &val) in row2.iter().enumerate() {
                    let r = l::cam_option_rect(2, col);
                    let sel = cur_ratio == val;
                    let hov = self.is_hovered(Target::Ratio(val));
                    if sel {
                        self.painter.fill(r, WHITE);
                    } else if hov {
                        self.painter.fill(r, ROW_HOVER);
                    }
                    self.painter.text(
                        val,
                        r,
                        Font::Caption,
                        if sel {
                            BLACK
                        } else if hov {
                            WHITE
                        } else {
                            WHITE_65
                        },
                        Align::Center,
                        false,
                    );
                }
                for (col, &val) in row3.iter().enumerate() {
                    let r = l::cam_option_rect(3, col);
                    let sel = cur_ratio == val;
                    let hov = self.is_hovered(Target::Ratio(val));
                    if sel {
                        self.painter.fill(r, WHITE);
                    } else if hov {
                        self.painter.fill(r, ROW_HOVER);
                    }
                    self.painter.text(
                        val,
                        r,
                        Font::Caption,
                        if sel {
                            BLACK
                        } else if hov {
                            WHITE
                        } else {
                            WHITE_65
                        },
                        Align::Center,
                        false,
                    );
                }

                // Internal 1 px grid dividers (white/10)
                for row in 1..4 {
                    let y = l::CAM_EXPAND_MENU.y + 1.0 + row as f32 * 25.0;
                    self.painter.line(
                        l::CAM_EXPAND_MENU.x + 1.0,
                        y,
                        l::CAM_EXPAND_MENU.right() - 1.0,
                        y,
                        WHITE_10,
                        1.0,
                    );
                }
                for &vx in &[l::CAM_EXPAND_MENU.x + 52.0, l::CAM_EXPAND_MENU.x + 104.0] {
                    self.painter.line(
                        vx,
                        l::CAM_EXPAND_MENU.y + 1.0,
                        vx,
                        l::CAM_EXPAND_MENU.bottom() - 1.0,
                        WHITE_10,
                        1.0,
                    );
                }
                self.painter
                    .stroke_rounded(l::CAM_EXPAND_MENU, 7.0, WHITE_36, 1.0);
            } else {
                let bg = if self.is_hovered(Target::CamMenu) && self.hud_is_bright() {
                    BTN_HOVER
                } else {
                    BLACK
                };
                self.painter.fill_rounded(l::CAM_MENU_BTN, 7.0, bg);
                self.painter.stroke_rounded(
                    l::CAM_MENU_BTN,
                    7.0,
                    dim_color(WHITE_22, hud_mul),
                    1.0,
                );
                self.painter.text(
                    s.camera_quality_label(),
                    l::CAM_MENU_BTN,
                    Font::Caption,
                    dim_color(WHITE, hud_mul),
                    Align::Center,
                    false,
                );
            }

            // Mirror button (white fill + black icon when active, matching .cam-btn.is-on)
            let mirror_on = s.is_on("camera.mirror", true);
            let (mir_bg, mir_bd, mir_fg) = if mirror_on {
                (dim_color(WHITE, hud_mul), dim_color(WHITE, hud_mul), BLACK)
            } else {
                (
                    BLACK,
                    dim_color(WHITE_20, hud_mul),
                    dim_color(WHITE, hud_mul),
                )
            };
            self.painter.fill_rounded(l::CAM_MIRROR_BTN, 7.0, mir_bg);
            self.painter
                .stroke_rounded(l::CAM_MIRROR_BTN, 7.0, mir_bd, 1.0);
            self.painter.icon(
                icons::MIRROR,
                l::CAM_MIRROR_BTN.x + l::CAM_MIRROR_BTN.w / 2.0,
                l::CAM_MIRROR_BTN.y + l::CAM_MIRROR_BTN.h / 2.0,
                13.0,
                mir_fg,
            );

            // Flip lens button
            self.painter.fill_rounded(l::CAM_FLIP_BTN, 7.0, BLACK);
            self.painter
                .stroke_rounded(l::CAM_FLIP_BTN, 7.0, dim_color(WHITE_20, hud_mul), 1.0);
            self.painter.icon(
                icons::FLIP,
                l::CAM_FLIP_BTN.x + l::CAM_FLIP_BTN.w / 2.0,
                l::CAM_FLIP_BTN.y + l::CAM_FLIP_BTN.h / 2.0,
                13.0,
                dim_color(WHITE, hud_mul),
            );

            // Camera + Mic floating waveform dock (collapses to 204x42, expands to 286x198)
            if mic_on {
                if quick_audio {
                    let dock = l::WAVE_DOCK_EXPANDED;
                    self.painter.fill_rounded(dock, 10.0, BLACK);
                    self.draw_audio_controls(l::WAVE_DOCK_EXPANDED_SETTINGS);
                    self.painter.line(
                        dock.x,
                        l::WAVE_DOCK_EXPANDED_HEADER.y,
                        dock.right(),
                        l::WAVE_DOCK_EXPANDED_HEADER.y,
                        WHITE_15,
                        1.0,
                    );
                    self.draw_waveform_bars(
                        l::WAVE_DOCK_EXPANDED_HEADER.x + l::WAVE_DOCK_EXPANDED_HEADER.w / 2.0,
                        l::WAVE_DOCK_EXPANDED_HEADER.y + l::WAVE_DOCK_EXPANDED_HEADER.h / 2.0,
                        true,
                    );
                    self.painter.stroke_rounded(dock, 10.0, WHITE_32, 1.0);
                } else {
                    let dock = l::WAVE_DOCK_DOCKED;
                    self.painter.fill_rounded(dock, dock.h / 2.0, BLACK);
                    self.draw_waveform_bars(dock.x + dock.w / 2.0, dock.y + dock.h / 2.0, true);
                    self.painter
                        .stroke_rounded(dock, dock.h / 2.0, WHITE_28, 1.0);
                }
            }
        } else if mic_on {
            // Mic-only: subtle dots grid + floating 25-bar waveform directly on black
            // (no card box, no border, no watermark, no text label)
            self.painter.dots_grid(c.inset(12.0), 18.0, 1.0, 0x161616);

            let wave_rect = if quick_audio {
                l::WAVE_DOCK_LIFTED
            } else {
                l::WAVE_DOCK_CENTERED
            };
            self.draw_waveform_bars(
                wave_rect.x + wave_rect.w / 2.0,
                wave_rect.y + wave_rect.h / 2.0,
                false,
            );

            // Bottom-anchored Quick Audio panel
            if quick_audio {
                let qp = l::AUDIO_QUICK_PANEL;
                self.painter.fill_rounded(qp, l::TILE_RADIUS, BLACK);
                self.painter
                    .fill(Rect::new(qp.x, qp.y, qp.w, l::TILE_RADIUS), BLACK);
                self.painter
                    .line(qp.x, qp.y, qp.right(), qp.y, WHITE_15, 1.0);
                self.draw_audio_controls(qp);
            }
        } else {
            // Standby (also when only Speaker is active — no extra visuals)
            self.painter.dots_grid(c.inset(12.0), 18.0, 1.0, 0x161616);
            let wm_alpha = if s.connected() { 0.07 } else { 0.07 * 0.45 };
            self.painter.watermark(mid_x, mid_y, 152.0, wm_alpha);
        }
    }

    fn draw_bottom_dock(&mut self) {
        let s = self.state.clone();
        let connected = s.connected();
        let pulsing = !connected || matches!(s.connection, Connection::Approval { .. });
        let pulse_t = ((self.anim_frame as f32 * 0.095).sin() + 1.0) * 0.5; // ~1.1s cycle

        // b0: Wi-Fi / Connection drawer toggle
        let b0 = l::bottom(0);
        let wifi_on = self.drawer == Drawer::Wifi;
        let wifi_hov = self.is_hovered(Target::Wifi);
        let b0_bg = if wifi_on {
            if wifi_hov { BTN_ON_HOVER } else { WHITE }
        } else if wifi_hov {
            BTN_HOVER
        } else {
            BLACK
        };
        self.painter.fill_rounded(b0, l::TILE_RADIUS, b0_bg);
        let ind = indicator(&s.connection);
        let b0_fg = if wifi_on {
            BLACK
        } else if pulsing {
            gray(0.35 + 0.65 * pulse_t)
        } else if wifi_hov {
            WHITE
        } else {
            WHITE_78
        };
        self.painter.icon(
            ind.icon,
            b0.x + b0.w / 2.0,
            b0.y + b0.h / 2.0,
            DOCK_ICON,
            b0_fg,
        );

        // b1: Camera, b2: Mic, b3: Speaker
        let media = [
            (
                1,
                Target::Camera,
                matches!(s.camera, Feature::On | Feature::Restarting) && connected,
                icons::CAMERA,
                icons::CAMERA_OFF,
            ),
            (
                2,
                Target::Mic,
                matches!(s.mic, Feature::On | Feature::Restarting) && connected,
                icons::MIC,
                icons::MIC_OFF,
            ),
            (
                3,
                Target::Speaker,
                matches!(s.speaker, Feature::On | Feature::Restarting) && connected,
                icons::SPEAKER,
                icons::SPEAKER_OFF,
            ),
        ];

        for (idx, target, on, ic_on, ic_off) in media {
            let rect = l::bottom(idx);
            let hov = connected && self.is_hovered(target);
            let bg = if on {
                if hov { BTN_ON_HOVER } else { WHITE }
            } else if hov {
                BTN_HOVER
            } else {
                BLACK
            };
            let fg = if on {
                BLACK
            } else if !connected {
                WHITE_28
            } else if hov {
                WHITE
            } else {
                WHITE_78
            };
            self.painter.fill_rounded(rect, l::TILE_RADIUS, bg);
            self.painter.icon(
                if on { ic_on } else { ic_off },
                rect.x + rect.w / 2.0,
                rect.y + rect.h / 2.0,
                DOCK_ICON,
                fg,
            );
        }

        // b4: Help drawer toggle
        let b4 = l::bottom(4);
        let help_on = matches!(self.drawer, Drawer::Help(_));
        let help_hov = self.is_hovered(Target::Help);
        let b4_bg = if help_on {
            if help_hov { BTN_ON_HOVER } else { WHITE }
        } else if help_hov {
            BTN_HOVER
        } else {
            BLACK
        };
        let b4_fg = if help_on {
            BLACK
        } else if help_hov {
            WHITE
        } else {
            WHITE_78
        };
        self.painter.fill_rounded(b4, l::TILE_RADIUS, b4_bg);
        self.painter.icon(
            icons::HELP,
            b4.x + b4.w / 2.0,
            b4.y + b4.h / 2.0,
            DOCK_ICON,
            b4_fg,
        );
    }

    fn format_last_connected(&self) -> Option<String> {
        let elapsed = self.last_connected_at?.elapsed().as_secs();
        if elapsed < 60 {
            Some("Just now".to_owned())
        } else if elapsed < 3600 {
            Some(format!("{}m ago", elapsed / 60))
        } else {
            Some(format!("{}h ago", elapsed / 3600))
        }
    }

    fn draw_drawer_wifi(&mut self, mode: WifiMode) {
        let s = self.state.clone();
        self.draw_playground_base();

        // 1. Header: Host PC name + IP + borderless Copy button
        let hdr = l::CONN_HEADER;
        let host = if s.hostname.is_empty() {
            "PC"
        } else {
            &s.hostname
        };
        self.painter.text(
            host,
            Rect::new(hdr.x + 14.0, hdr.y + 10.0, 220.0, 18.0),
            Font::Title,
            WHITE_90,
            Align::Left,
            false,
        );
        let ip_text = s.host_ip();
        self.painter.text(
            &ip_text,
            Rect::new(hdr.x + 14.0, hdr.y + 28.0, 220.0, 16.0),
            Font::Caption,
            WHITE_35,
            Align::Left,
            false,
        );

        let copy_btn = l::CONN_COPY_IP_BTN;
        let copy_hov = self.is_hovered(Target::CopyIp);
        let copied = self.copied_at.is_some_and(|t| t.elapsed() < COPY_FLASH);
        if copy_hov {
            self.painter.fill_rounded(copy_btn, 6.0, WHITE_10);
        }
        self.painter.icon(
            if copied { icons::CHECK } else { icons::COPY },
            copy_btn.x + copy_btn.w / 2.0,
            copy_btn.y + copy_btn.h / 2.0,
            14.0,
            if copied || copy_hov { WHITE } else { WHITE_40 },
        );
        self.painter.line(
            hdr.x,
            hdr.bottom(),
            hdr.right(),
            hdr.bottom(),
            WHITE_10,
            1.0,
        );

        // 2. Primary device row
        let prim = l::CONN_PRIMARY_ROW;
        let ind = indicator(&s.connection);
        let show_empty = s.phones.is_empty()
            && !s.connected()
            && !matches!(s.connection, Connection::Approval { .. });

        let pulse_t = ((self.anim_frame as f32 * 0.095).sin() + 1.0) * 0.5;
        let row_ic_color = match mode {
            WifiMode::Connected => WHITE,
            WifiMode::Pairing => gray(0.35 + 0.55 * pulse_t),
            _ => WHITE_25,
        };
        let row_icon = if show_empty { icons::UNPLUG } else { ind.icon };
        self.painter.icon(
            row_icon,
            prim.x + 14.0 + 9.0,
            prim.y + prim.h / 2.0,
            17.0,
            row_ic_color,
        );

        let (name, name_color, subtitle, sub_color) = if show_empty {
            (
                "No phone connected",
                WHITE_50,
                "Open Owlmic on your phone".to_owned(),
                WHITE_35,
            )
        } else {
            let name = s.primary_phone_name();
            let name_color = match mode {
                WifiMode::Connected | WifiMode::Pairing => WHITE,
                _ => WHITE_50,
            };
            let (subtitle, sub_color) = match mode {
                WifiMode::Pairing => ("Wants to pair".to_owned(), WHITE_70),
                WifiMode::Connected => (ind.label.to_owned(), WHITE_50),
                WifiMode::Blocked => ("Blocked".to_owned(), WHITE_35),
                WifiMode::Forget | WifiMode::Offline => (
                    self.format_last_connected()
                        .unwrap_or_else(|| "Offline".to_owned()),
                    WHITE_35,
                ),
            };
            (name, name_color, subtitle, sub_color)
        };
        self.painter.text(
            name,
            Rect::new(prim.x + 44.0, prim.y + 12.0, 165.0, 18.0),
            Font::Title,
            name_color,
            Align::Left,
            false,
        );

        self.painter.text(
            &subtitle,
            Rect::new(prim.x + 44.0, prim.y + 32.0, 165.0, 16.0),
            Font::Caption,
            sub_color,
            Align::Left,
            false,
        );

        match mode {
            WifiMode::Pairing => {
                let pair = l::CONN_PAIR_BTN;
                let pair_bg = if self.is_hovered(Target::Pair) {
                    WHITE_90
                } else {
                    WHITE
                };
                self.painter.fill_rounded(pair, 6.0, pair_bg);
                self.painter.icon(
                    icons::CHECK,
                    pair.x + pair.w / 2.0,
                    pair.y + pair.h / 2.0,
                    14.0,
                    BLACK,
                );
                let dec = l::CONN_DECLINE_BTN;
                let dec_hov = self.is_hovered(Target::Decline);
                self.painter.fill_rounded(dec, 6.0, WHITE_10);
                self.painter.icon(
                    icons::CLOSE,
                    dec.x + dec.w / 2.0,
                    dec.y + dec.h / 2.0,
                    14.0,
                    if dec_hov { WHITE } else { WHITE_50 },
                );
            }
            WifiMode::Connected => {
                let disc = l::CONN_DISCONNECT_BTN;
                let disc_hov = self.is_hovered(Target::Disconnect);
                self.painter.icon(
                    icons::UNPLUG,
                    disc.x + disc.w / 2.0,
                    disc.y + disc.h / 2.0,
                    15.0,
                    if disc_hov { WHITE } else { WHITE_65 },
                );
                let blk = l::CONN_BLOCK_BTN;
                self.painter.text(
                    "Block",
                    blk,
                    Font::Caption,
                    if self.is_hovered(Target::Block) {
                        WHITE
                    } else {
                        WHITE_45
                    },
                    Align::Right,
                    false,
                );
            }
            WifiMode::Blocked => {
                let unblk = l::CONN_UNBLOCK_BTN;
                self.painter.text(
                    "Unblock",
                    unblk,
                    Font::Body,
                    if self.is_hovered(Target::Unblock) {
                        WHITE_80
                    } else {
                        WHITE
                    },
                    Align::Right,
                    false,
                );
            }
            WifiMode::Forget => {
                let fgt = l::CONN_FORGET_BTN;
                self.painter.text(
                    "Forget",
                    fgt,
                    Font::Caption,
                    if self.is_hovered(Target::Forget) {
                        WHITE
                    } else {
                        WHITE_50
                    },
                    Align::Right,
                    false,
                );
            }
            WifiMode::Offline => {
                if !show_empty {
                    let blk = l::CONN_BLOCK_BTN;
                    self.painter.text(
                        "Block",
                        blk,
                        Font::Caption,
                        if self.is_hovered(Target::Block) {
                            WHITE
                        } else {
                            WHITE_45
                        },
                        Align::Right,
                        false,
                    );
                }
            }
        }
        self.painter.line(
            prim.x,
            prim.bottom(),
            prim.right(),
            prim.bottom(),
            WHITE_10,
            1.0,
        );

        // 1 px sync beam sweep along bottom edge of primary row during pairing/connecting
        let is_sweeping =
            mode == WifiMode::Pairing || matches!(s.connection, Connection::Connecting { .. });
        if let Some(t) = self.sync_flash_at {
            if t.elapsed() < SYNC_FLASH {
                let fade = 1.0 - (t.elapsed().as_secs_f32() / SYNC_FLASH.as_secs_f32());
                self.painter.line(
                    prim.x,
                    prim.bottom(),
                    prim.right(),
                    prim.bottom(),
                    gray(fade * 0.95),
                    1.0,
                );
            }
        } else if is_sweeping {
            self.draw_sync_beam(prim);
        }

        // 3. Secondary paired row (only when another paired phone exists; otherwise clean empty)
        if let Some(other) = s.secondary_phone() {
            let paired = l::CONN_PAIRED_ROW;
            self.painter.icon(
                icons::WIFI,
                paired.x + 14.0 + 9.0,
                paired.y + paired.h / 2.0,
                17.0,
                WHITE_25,
            );
            self.painter.text(
                &other.name,
                Rect::new(paired.x + 44.0, paired.y + 12.0, 175.0, 18.0),
                Font::Body,
                WHITE_50,
                Align::Left,
                false,
            );
            self.painter.text(
                if other.blocked { "Blocked" } else { "Paired" },
                Rect::new(paired.x + 44.0, paired.y + 32.0, 175.0, 16.0),
                Font::Caption,
                WHITE_35,
                Align::Left,
                false,
            );
            let rem = l::CONN_REMOVE_PAIRED_BTN;
            self.painter.text(
                "Remove",
                rem,
                Font::Caption,
                if self.is_hovered(Target::RemovePaired) {
                    WHITE_70
                } else {
                    WHITE_35
                },
                Align::Right,
                false,
            );
            self.painter.line(
                paired.x,
                paired.bottom(),
                paired.right(),
                paired.bottom(),
                WHITE_10,
                1.0,
            );
        }

        // 4. Transport footer: Refresh | full-height divider | 4 equal transport toggles
        let tf = l::CONN_FOOTER;
        self.painter.fill_rounded(tf, l::TILE_RADIUS, BLACK);
        self.painter
            .fill(Rect::new(tf.x, tf.y, tf.w, l::TILE_RADIUS), BLACK);
        self.painter
            .line(tf.x, tf.y, tf.right(), tf.y, WHITE_16, 1.0);
        self.painter.line(
            l::CONN_FOOTER_DIVIDER_X,
            tf.y,
            l::CONN_FOOTER_DIVIDER_X,
            tf.bottom(),
            WHITE_16,
            1.0,
        );

        let ref_btn = l::CONN_REFRESH_BTN;
        let is_spinning = self
            .refresh_at
            .is_some_and(|t| t.elapsed() < Duration::from_millis(420));
        let ref_hov = self.is_hovered(Target::Refresh);
        self.painter
            .fill_rounded(ref_btn, 7.0, if is_spinning { WHITE_10 } else { BLACK });
        self.painter.stroke_rounded(
            ref_btn,
            7.0,
            if ref_hov || is_spinning {
                WHITE_35
            } else {
                WHITE_20
            },
            1.0,
        );
        let rot_180 = self.refresh_at.is_some_and(|t| {
            let el = t.elapsed();
            el >= Duration::from_millis(210) && el < Duration::from_millis(420)
        });
        if rot_180 {
            self.painter.icon_rotated_180(
                icons::REFRESH,
                ref_btn.x + ref_btn.w / 2.0,
                ref_btn.y + ref_btn.h / 2.0,
                15.0,
                if is_spinning { WHITE } else { WHITE_80 },
            );
        } else {
            self.painter.icon(
                icons::REFRESH,
                ref_btn.x + ref_btn.w / 2.0,
                ref_btn.y + ref_btn.h / 2.0,
                15.0,
                if is_spinning { WHITE } else { WHITE_80 },
            );
        }

        let transports: [(usize, &'static str, crate::glyph::Icon); 4] = [
            (0, "link.usbDebugging", icons::USB),
            (1, "link.usbTethering", icons::CABLE),
            (2, "link.wifi", icons::WIFI),
            (3, "link.bluetooth", icons::BLUETOOTH),
        ];

        for (idx, key, ic) in transports {
            let btn = l::conn_transport_btn(idx);
            let on = s.is_on(key, true);
            let hov = self.is_hovered(Target::ToggleTransport(idx as u8 + 1));
            let bg = if on { WHITE_10 } else { BLACK };
            let bd = if hov {
                WHITE_28
            } else if on {
                WHITE_24
            } else {
                WHITE_14
            };
            let fg = if on { WHITE } else { WHITE_42 };
            self.painter.fill_rounded(btn, 7.0, bg);
            self.painter.stroke_rounded(btn, 7.0, bd, 1.0);
            self.painter
                .icon(ic, btn.x + btn.w / 2.0, btn.y + btn.h / 2.0, 15.0, fg);
        }
    }

    /// Dual soft 1 px horizontal glide along the bottom edge of the pairing row.
    fn draw_sync_beam(&mut self, row: Rect) {
        let cycle = (self.anim_frame % 126) as f32 / 126.0;
        let pulses = [
            (cycle / 0.58, 0.50, 0.95),
            ((cycle - 0.32) / 0.60, 0.35, 0.72),
        ];
        let y = row.bottom();
        for (p, width_frac, peak) in pulses {
            if !(0.0..=1.0).contains(&p) {
                continue;
            }
            let beam_w = row.w * width_frac;
            let center = row.x - beam_w * 0.5 + p * (row.w + beam_w);
            let segs = 12;
            let seg_w = beam_w / segs as f32;
            for i in 0..segs {
                let x0 = (center - beam_w / 2.0 + i as f32 * seg_w).max(row.x);
                let x1 = (x0 + seg_w).min(row.right());
                if x1 <= x0 {
                    continue;
                }
                let u = (i as f32 + 0.5) / segs as f32;
                let intensity = (u * std::f32::consts::PI).sin() * peak;
                self.painter.line(x0, y, x1, y, gray(intensity), 1.0);
            }
        }
    }

    fn draw_help_sub_header(&mut self, title: &str) {
        let hdr = l::HELP_HEADER;
        let back = l::HELP_BACK_BTN;
        if self.is_hovered(Target::HelpBack) {
            self.painter.fill_rounded(back, 7.0, WHITE_10);
        }
        self.painter.icon_rotated_180(
            icons::ARROW,
            back.x + back.w / 2.0,
            back.y + back.h / 2.0,
            16.0,
            WHITE,
        );
        self.painter.text(
            title,
            Rect::new(back.right() + 8.0, hdr.y, 200.0, hdr.h),
            Font::Title,
            WHITE,
            Align::Left,
            false,
        );
        self.painter.line(
            hdr.x,
            hdr.bottom(),
            hdr.right(),
            hdr.bottom(),
            WHITE_15,
            1.0,
        );
    }

    fn draw_drawer_help(&mut self, screen: HelpScreen) {
        let s = self.state.clone();
        self.draw_playground_base();

        match screen {
            HelpScreen::Bento => {
                let items: [(usize, Target, &'static str, crate::glyph::Icon); 6] = [
                    (0, Target::HelpHowTo, "How-to", icons::LIST),
                    (1, Target::HelpPrivacy, "Privacy", icons::SHIELD),
                    (2, Target::HelpRate, "Rate", icons::STAR),
                    (3, Target::HelpGitHub, "GitHub", icons::GITHUB),
                    (4, Target::HelpDonate, "Donate", icons::COFFEE),
                    (5, Target::HelpCredits, "Credits", icons::USERS),
                ];
                for (idx, target, label, ic) in items {
                    let tile = l::help_bento_tile(idx);
                    let hov = self.is_hovered(target);
                    self.painter.fill_rounded(tile, 10.0, BLACK);
                    self.painter.stroke_rounded(
                        tile,
                        10.0,
                        if hov { WHITE_30 } else { WHITE_15 },
                        1.0,
                    );
                    self.painter
                        .icon(ic, tile.x + tile.w / 2.0, tile.y + 30.0, 22.0, WHITE);
                    self.painter.text(
                        label,
                        Rect::new(tile.x, tile.y + 50.0, tile.w, 16.0),
                        Font::Caption,
                        WHITE_70,
                        Align::Center,
                        false,
                    );
                }
                self.painter.text(
                    &s.version,
                    Rect::new(
                        l::PLAYGROUND.x,
                        l::PLAYGROUND.bottom() - 22.0,
                        l::PLAYGROUND.w,
                        14.0,
                    ),
                    Font::Caption,
                    WHITE_35,
                    Align::Center,
                    false,
                );
            }
            HelpScreen::HowTo => {
                self.draw_help_sub_header("How-to");

                let steps: [(i32, f32, &'static str, &'static str); 3] = [
                    (
                        1,
                        56.0,
                        "Get the Android App",
                        "Install and open Owlmic on your phone",
                    ),
                    (
                        2,
                        120.0,
                        "Pair with this PC",
                        "Plug in USB or join the same Wi-Fi, or copy this PC's IP from Connections",
                    ),
                    (
                        3,
                        196.0,
                        "Pick Owlmic in Windows apps",
                        "Select Owlmic Camera or Mic in Discord, OBS, Meet, or Zoom",
                    ),
                ];

                // Vertical 1 px timeline spine behind step badges
                let spine_x = l::PLAYGROUND.x + 16.0 + 11.0;
                self.painter.line(
                    spine_x,
                    l::PLAYGROUND.y + steps[0].1 + 11.0,
                    spine_x,
                    l::PLAYGROUND.y + steps[2].1 + 11.0,
                    WHITE_20,
                    1.0,
                );

                for (idx, y_off, title, desc) in steps {
                    let y = l::PLAYGROUND.y + y_off;
                    let badge = Rect::new(l::PLAYGROUND.x + 16.0, y, 22.0, 22.0);
                    self.painter.fill_rounded(badge, 11.0, BLACK);
                    self.painter.stroke_rounded(badge, 11.0, WHITE_40, 1.0);
                    self.painter.text(
                        &idx.to_string(),
                        badge,
                        Font::Caption,
                        WHITE,
                        Align::Center,
                        false,
                    );

                    let title_x = badge.right() + 12.0;
                    let text_w = l::PLAYGROUND.right() - 16.0 - title_x;
                    let title_str = if idx == 1 {
                        "Get the Android App ↗"
                    } else {
                        title
                    };
                    self.painter.text(
                        title_str,
                        Rect::new(title_x, y, text_w, 18.0),
                        Font::Body,
                        WHITE,
                        Align::Left,
                        false,
                    );
                    if idx == 1 && self.is_hovered(Target::HelpReleases) {
                        self.painter.line(
                            title_x,
                            y + 17.0,
                            title_x + 132.0,
                            y + 17.0,
                            WHITE_70,
                            1.0,
                        );
                    }
                    self.painter.text(
                        desc,
                        Rect::new(title_x, y + 19.0, text_w, 46.0),
                        Font::Caption,
                        WHITE_55,
                        Align::Left,
                        true,
                    );
                }
            }
            HelpScreen::Privacy => {
                self.draw_help_sub_header("Privacy");
                self.painter.text(
                    "Owlmic receives camera, microphone, and speaker streams strictly over your local USB or Wi-Fi network. Zero media or telemetry leaves your local network.",
                    Rect::new(
                        l::PLAYGROUND.x + 16.0,
                        l::HELP_HEADER.bottom() + 16.0,
                        l::PLAYGROUND.w - 32.0,
                        180.0,
                    ),
                    Font::Body,
                    WHITE_60,
                    Align::Left,
                    true,
                );
            }
            HelpScreen::Credits => {
                self.draw_help_sub_header("Credits");
                for (idx, (name, user)) in CREDITS.iter().enumerate() {
                    let row = l::help_credit_row(idx);
                    if self.is_hovered(Target::HelpCredit(idx)) {
                        self.painter.fill(row, ROW_HOVER);
                    }
                    self.painter.text(
                        name,
                        Rect::new(row.x + 14.0, row.y, 140.0, row.h),
                        Font::Body,
                        WHITE,
                        Align::Left,
                        false,
                    );
                    self.painter.text(
                        &format!("@{user}"),
                        Rect::new(row.x + 140.0, row.y, row.w - 154.0, row.h),
                        Font::Caption,
                        WHITE_50,
                        Align::Right,
                        false,
                    );
                    self.painter.line(
                        row.x,
                        row.bottom(),
                        row.right(),
                        row.bottom(),
                        WHITE_10,
                        1.0,
                    );
                }

                let gh_row = l::help_credit_row(CREDITS.len());
                if self.is_hovered(Target::HelpGitHub) {
                    self.painter.fill(gh_row, ROW_HOVER);
                }
                self.painter.text(
                    "GitHub",
                    Rect::new(gh_row.x + 14.0, gh_row.y, 140.0, gh_row.h),
                    Font::Body,
                    WHITE,
                    Align::Left,
                    false,
                );
                self.painter.text(
                    "↗",
                    Rect::new(gh_row.right() - 30.0, gh_row.y, 16.0, gh_row.h),
                    Font::Caption,
                    WHITE_55,
                    Align::Right,
                    false,
                );
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

fn open_url(target: &str) {
    unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &HSTRING::from(target),
            None,
            None,
            SW_SHOWNORMAL,
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
        Link::Website => Some("[https://owlmic.app](https://owlmic.app)".to_owned()),
        Link::Donate => Some("[https://github.com/sponsors/diveshpatil9104](https://github.com/sponsors/diveshpatil9104)".to_owned()),
        Link::GitHub | Link::Rate => Some("[https://github.com/diveshpatil9104/owlmic](https://github.com/diveshpatil9104/owlmic)".to_owned()),
    };
    if let Some(t) = target {
        open_url(&t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::{LINK_USB_DEBUGGING, LINK_WIFI, PhoneEntry, Repair};
    use std::collections::BTreeMap;
    use windows::Win32::Graphics::Direct2D::Common::*;
    use windows::Win32::Graphics::Direct2D::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
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

            let preview = Arc::new(Preview::default());

            let mut settings = BTreeMap::new();
            settings.insert("mic.isolation".into(), "on".into());
            settings.insert("mic.noiseReduction".into(), "5".into());
            settings.insert("mic.volume".into(), "80".into());
            settings.insert("mic.autoLevel".into(), "on".into());
            settings.insert("camera.lens".into(), "front".into());
            settings.insert("camera.quality".into(), "1080p".into());
            settings.insert("camera.frameRate".into(), "30".into());
            settings.insert("camera.ratio".into(), "Auto".into());
            settings.insert("camera.mirror".into(), "on".into());
            settings.insert("speaker.quietPc".into(), "on".into());
            settings.insert("link.usbDebugging".into(), "on".into());
            settings.insert("link.usbTethering".into(), "on".into());
            settings.insert("link.wifi".into(), "on".into());
            settings.insert("link.bluetooth".into(), "on".into());
            settings.insert("general.startWithWindows".into(), "on".into());

            let phones = vec![
                PhoneEntry {
                    id: "p1".into(),
                    name: "Pixel 8".into(),
                    blocked: false,
                },
                PhoneEntry {
                    id: "p2".into(),
                    name: "Galaxy S24".into(),
                    blocked: false,
                },
            ];
            let addresses = vec!["192.168.1.15:7653".into()];

            // (filename, state, drawer, cam_hud, cam_menu, mic_quick, dock_open)
            let mut states: Vec<(&str, PanelState, Drawer, bool, bool, bool, bool)> = Vec::new();

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
                    phones: phones.clone(),
                    addresses: addresses.clone(),
                    hostname: "YASH-PC".into(),
                    version: "v0.1.0".into(),
                    ..Default::default()
                },
                Drawer::Wifi,
                false,
                false,
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
                true,
                false,
                false,
                false,
            ));

            states.push((
                "10_cam_and_mic_docked.bmp",
                PanelState {
                    connection: Connection::Connected {
                        phone: "Pixel 8".into(),
                        link: LINK_USB_DEBUGGING,
                        weak: false,
                    },
                    mic: Feature::On,
                    camera: Feature::On,
                    settings: settings.clone(),
                    paired: true,
                    version: "v0.1.0".into(),
                    ..Default::default()
                },
                Drawer::None,
                true,
                false,
                false,
                false,
            ));

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
                true,
                false,
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
                false,
                true,
                false,
            ));

            states.push((
                "21_cam_and_mic_dock_expanded.bmp",
                PanelState {
                    connection: Connection::Connected {
                        phone: "Pixel 8".into(),
                        link: LINK_USB_DEBUGGING,
                        weak: false,
                    },
                    camera: Feature::On,
                    mic: Feature::On,
                    settings: settings.clone(),
                    paired: true,
                    version: "v0.1.0".into(),
                    ..Default::default()
                },
                Drawer::None,
                true,
                false,
                false,
                true,
            ));

            let out_dir =
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../ui/pc");
            let _ = std::fs::create_dir_all(&out_dir);

            for (file_name, st, drawer, cam_hud, cam_menu, mic_quick, dock_open) in states {
                let arc_st = Arc::new(st);
                let mut panel = Panel {
                    hwnd: HWND::default(),
                    ui: Ui {
                        state: Publisher::new((*arc_st).clone(), || {}),
                        preview: preview.clone(),
                        meter: Arc::new(AtomicU32::new(0)),
                        on_action: Box::new(|_| {}),
                    },
                    painter: Painter::new().unwrap(),
                    tray: None,
                    state: arc_st,
                    drawer,
                    cam_hud,
                    cam_hud_at: None,
                    cam_menu_open: cam_menu,
                    mic_quick_open: mic_quick,
                    dock_open,
                    hover: None,
                    dragging: None,
                    scale: 1.0,
                    visible: true,
                    hidden_at: None,
                    shown_at: None,
                    last_tray_click: Instant::now() - Duration::from_secs(1),
                    applied_at: Instant::now(),
                    copied_at: None,
                    refresh_at: None,
                    last_connected_at: None,
                    sync_flash_at: None,
                    taskbar_created: 0,
                    anim_frame: 12,
                    anim_state: AnimState::Open,
                    anim_start: Instant::now(),
                    base_pos: (0, 0),
                    is_mouse_over: true,
                    unhover_since: None,
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
                    panel.paint_frame();
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
