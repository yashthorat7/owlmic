//! The panel window: 312 x 312 (1:1) at 100% scale above the tray, never in the taskbar, closed by
//! Esc or by clicking elsewhere (SYSTEM_DESIGN section 33). Visual layout, palette, and
//! interaction behavior mirror the PC prototype 1:1.

use super::draw::{Align, Font, Painter};
use super::tray::{self, Tray};
use crate::icons;
use crate::layout::{self as l, CropMode, Drawer, HelpScreen, Rect, Screen, Target, WifiMode};
use crate::preview::Preview;
use crate::view::{Action, Connection, Feature, Link, PanelState, indicator, link_icon};
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
    cam_menu_wide: Option<bool>,
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
    anim: UiAnim,
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
        let init_state = ui.state.read();
        let anim = UiAnim::settled_for(&init_state, Drawer::None, false, false, false, false);
        let panel = Box::new(Panel {
            hwnd: HWND::default(),
            state: init_state,
            ui,
            painter: Painter::new()?,
            tray: None,
            drawer: Drawer::None,
            cam_hud: false,
            cam_hud_at: None,
            cam_menu_open: false,
            cam_menu_wide: None,
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
            anim,
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

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_rect(a: Rect, b: Rect, t: f32) -> Rect {
    Rect::new(
        lerp(a.x, b.x, t),
        lerp(a.y, b.y, t),
        lerp(a.w, b.w, t),
        lerp(a.h, b.h, t),
    )
}

fn lerp_color(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |shift: u32| -> u32 {
        let ca = ((a >> shift) & 0xFF) as f32;
        let cb = ((b >> shift) & 0xFF) as f32;
        lerp(ca, cb, t).round().clamp(0.0, 255.0) as u32
    };
    (mix(16) << 16) | (mix(8) << 8) | mix(0)
}

fn cubic_bezier(p1x: f32, p1y: f32, p2x: f32, p2y: f32, x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    if x <= 0.0 || x >= 1.0 {
        return x;
    }
    let cx = 3.0 * p1x;
    let bx = 3.0 * (p2x - p1x) - cx;
    let ax = 1.0 - cx - bx;
    let mut t = x;
    for _ in 0..4 {
        let xt = ((ax * t + bx) * t + cx) * t - x;
        let dx = (3.0 * ax * t + 2.0 * bx) * t + cx;
        if dx.abs() < 1e-5 {
            break;
        }
        t = (t - xt / dx).clamp(0.0, 1.0);
    }
    let cy = 3.0 * p1y;
    let by = 3.0 * (p2y - p1y) - cy;
    let ay = 1.0 - cy - by;
    (((ay * t + by) * t + cy) * t).clamp(0.0, 1.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Curve {
    EaseOut,
    EaseOutExpo,
}

#[derive(Debug, Clone, Copy)]
struct Tween {
    from: f32,
    target: f32,
    started: Instant,
    dur: Duration,
    curve: Curve,
}

impl Tween {
    fn settled(v: f32, dur_ms: u64, curve: Curve) -> Self {
        let dur = Duration::from_millis(dur_ms);
        Self {
            from: v,
            target: v,
            started: Instant::now() - dur - Duration::from_secs(1),
            dur,
            curve,
        }
    }

    fn set(&mut self, next: f32) {
        if (self.target - next).abs() > 0.001 {
            self.from = self.now();
            self.target = next;
            self.started = Instant::now();
        }
    }

    fn snap(&mut self, v: f32) {
        self.from = v;
        self.target = v;
        self.started = Instant::now() - self.dur - Duration::from_secs(1);
    }

    fn now(&self) -> f32 {
        let raw = if self.dur.is_zero() {
            1.0
        } else {
            (self.started.elapsed().as_secs_f32() / self.dur.as_secs_f32()).clamp(0.0, 1.0)
        };
        let e = match self.curve {
            Curve::EaseOut => cubic_bezier(0.22, 1.0, 0.36, 1.0, raw),
            Curve::EaseOutExpo => cubic_bezier(0.16, 1.0, 0.3, 1.0, raw),
        };
        lerp(self.from, self.target, e)
    }
}

struct UiAnim {
    epoch: Instant,
    drawer_t: Tween,
    last_drawer: Drawer,
    help_sub_t: Tween,
    last_help: HelpScreen,
    cam_layer_t: Tween,
    cam_hud_t: Tween,
    cam_menu_t: Tween,
    cam_orient_t: Tween,
    wave_dock_cam_t: Tween,
    dock_expand_t: Tween,
    quick_audio_t: Tween,
    iso_sw_t: Tween,
    norm_sw_t: Tween,
    nr_drag_t: Tween,
    vol_drag_t: Tween,
    vol_live: f32,
    btn_on: [Tween; 5],
    btn_hov: [Tween; 5],
    wave_bars_h: [f32; WAVE_BARS],
}

impl UiAnim {
    fn settled_for(
        state: &PanelState,
        drawer: Drawer,
        cam_hud: bool,
        cam_menu: bool,
        mic_quick: bool,
        dock_open: bool,
    ) -> Self {
        let cam_on = matches!(state.camera, Feature::On | Feature::Restarting) && state.connected();
        let mic_on = matches!(state.mic, Feature::On | Feature::Restarting) && state.connected();
        let spk_on =
            matches!(state.speaker, Feature::On | Feature::Restarting) && state.connected();
        let hud_bright = cam_hud || cam_menu || (cam_on && mic_on && dock_open);
        let on_flags = [
            drawer == Drawer::Wifi,
            cam_on,
            mic_on,
            spk_on,
            matches!(drawer, Drawer::Help(_)),
        ];
        let help_screen = match drawer {
            Drawer::Help(h) => h,
            _ => HelpScreen::Bento,
        };
        let is_help_sub = !matches!(help_screen, HelpScreen::Bento);
        Self {
            epoch: Instant::now(),
            drawer_t: Tween::settled(
                if drawer != Drawer::None { 1.0 } else { 0.0 },
                280,
                Curve::EaseOut,
            ),
            last_drawer: if drawer != Drawer::None {
                drawer
            } else {
                Drawer::Wifi
            },
            help_sub_t: Tween::settled(if is_help_sub { 1.0 } else { 0.0 }, 220, Curve::EaseOut),
            last_help: if is_help_sub {
                help_screen
            } else {
                HelpScreen::HowTo
            },
            cam_layer_t: Tween::settled(if cam_on { 1.0 } else { 0.0 }, 180, Curve::EaseOut),
            cam_hud_t: Tween::settled(if hud_bright { 1.0 } else { 0.15 }, 260, Curve::EaseOut),
            cam_menu_t: Tween::settled(if cam_menu { 1.0 } else { 0.0 }, 240, Curve::EaseOutExpo),
            cam_orient_t: Tween::settled(
                if matches!(state.camera_ratio_label(), "16:9" | "4:3" | "2:1") {
                    1.0
                } else {
                    0.0
                },
                220,
                Curve::EaseOutExpo,
            ),
            wave_dock_cam_t: Tween::settled(if cam_on { 1.0 } else { 0.0 }, 360, Curve::EaseOut),
            dock_expand_t: Tween::settled(
                if cam_on && mic_on && dock_open {
                    1.0
                } else {
                    0.0
                },
                340,
                Curve::EaseOut,
            ),
            quick_audio_t: Tween::settled(
                if !cam_on && mic_on && mic_quick {
                    1.0
                } else {
                    0.0
                },
                280,
                Curve::EaseOut,
            ),
            iso_sw_t: Tween::settled(
                if state.is_on("mic.isolation", true) {
                    1.0
                } else {
                    0.0
                },
                220,
                Curve::EaseOut,
            ),
            norm_sw_t: Tween::settled(
                if state.is_on("mic.autoLevel", true) {
                    1.0
                } else {
                    0.0
                },
                220,
                Curve::EaseOut,
            ),
            nr_drag_t: Tween::settled(0.0, 140, Curve::EaseOut),
            vol_drag_t: Tween::settled(0.0, 140, Curve::EaseOut),
            vol_live: 0.0,
            btn_on: on_flags
                .map(|on| Tween::settled(if on { 1.0 } else { 0.0 }, 140, Curve::EaseOut)),
            btn_hov: [Tween::settled(0.0, 140, Curve::EaseOut); 5],
            wave_bars_h: [0.0; WAVE_BARS],
        }
    }

    fn elapsed_secs(&self, anim_frame: u64) -> f32 {
        let live = self.epoch.elapsed().as_secs_f32();
        if live < 0.002 && anim_frame > 0 {
            anim_frame as f32 * 0.016
        } else {
            live
        }
    }
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

    fn cam_wide(&self) -> bool {
        match self.state.camera_ratio_label() {
            "16:9" | "4:3" | "2:1" => true,
            "9:16" | "3:4" | "1:2" => false,
            _ => {
                if let Some(w) = self.cam_menu_wide {
                    w
                } else {
                    let stream = self.ui.preview.stream_size();
                    let size = if stream.0 > 0 {
                        stream
                    } else {
                        self.ui.preview.size()
                    };
                    self.state.camera_wide(size, false)
                }
            }
        }
    }

    fn current_screen(&self) -> Screen {
        self.state.active_screen(
            self.drawer,
            self.cam_menu_open,
            self.active_quick_audio(),
            self.cam_wide(),
        )
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

    fn sync_tweens(&mut self) {
        let cam_on = self.cam_active();
        let mic_on = self.mic_active();
        let spk_on = matches!(self.state.speaker, Feature::On | Feature::Restarting)
            && self.state.connected();
        let connected = self.state.connected();
        if self.drawer != Drawer::None {
            if matches!(self.drawer, Drawer::Help(_)) && self.anim.drawer_t.now() < 0.05 {
                let is_sub = !matches!(self.drawer, Drawer::Help(HelpScreen::Bento));
                self.anim.help_sub_t.snap(if is_sub { 1.0 } else { 0.0 });
            }
            self.anim.last_drawer = self.drawer;
        }
        self.anim.drawer_t.set(if self.drawer != Drawer::None {
            1.0
        } else {
            0.0
        });
        if let Drawer::Help(h) = self.drawer {
            if !matches!(h, HelpScreen::Bento) {
                self.anim.last_help = h;
            }
            self.anim
                .help_sub_t
                .set(if !matches!(h, HelpScreen::Bento) {
                    1.0
                } else {
                    0.0
                });
        }
        self.anim.cam_layer_t.set(if cam_on { 1.0 } else { 0.0 });
        self.anim
            .cam_hud_t
            .set(if self.hud_is_bright() { 1.0 } else { 0.15 });
        self.anim.cam_menu_t.set(if cam_on && self.cam_menu_open {
            1.0
        } else {
            0.0
        });
        self.anim
            .cam_orient_t
            .set(if self.cam_wide() { 1.0 } else { 0.0 });
        self.anim
            .wave_dock_cam_t
            .set(if cam_on { 1.0 } else { 0.0 });
        self.anim
            .dock_expand_t
            .set(if cam_on && mic_on && self.dock_open {
                1.0
            } else {
                0.0
            });
        self.anim
            .quick_audio_t
            .set(if !cam_on && mic_on && self.mic_quick_open {
                1.0
            } else {
                0.0
            });
        self.anim
            .iso_sw_t
            .set(if self.state.is_on("mic.isolation", true) {
                1.0
            } else {
                0.0
            });
        self.anim
            .norm_sw_t
            .set(if self.state.is_on("mic.autoLevel", true) {
                1.0
            } else {
                0.0
            });
        self.anim
            .nr_drag_t
            .set(if self.dragging == Some(DragFader::Nr) {
                1.0
            } else {
                0.0
            });
        self.anim
            .vol_drag_t
            .set(if self.dragging == Some(DragFader::Vol) {
                1.0
            } else {
                0.0
            });

        let on_flags = [
            self.drawer == Drawer::Wifi,
            cam_on,
            mic_on,
            spk_on,
            matches!(self.drawer, Drawer::Help(_)),
        ];
        let hov_targets = [
            (Target::Wifi, true),
            (Target::Camera, connected),
            (Target::Mic, connected),
            (Target::Speaker, connected),
            (Target::Help, true),
        ];
        for i in 0..5 {
            self.anim.btn_on[i].set(if on_flags[i] { 1.0 } else { 0.0 });
            let (t, allow) = hov_targets[i];
            self.anim.btn_hov[i].set(if allow && self.is_hovered(t) {
                1.0
            } else {
                0.0
            });
        }
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

                // Auto-close ratio menu and dim camera HUD after 4s idle (matching CFG.hudIdleMs = 4000)
                if (self.cam_hud || self.cam_menu_open)
                    && !(self.mic_active() && self.dock_open)
                    && self.cam_hud_at.is_some_and(|t| t.elapsed() >= HUD_IDLE)
                {
                    self.cam_menu_open = false;
                    self.cam_hud = false;
                    self.cam_hud_at = None;
                    self.sync_tweens();
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
                self.sync_tweens();
                self.invalidate();
            }
            WM_LBUTTONUP => {
                let x = lo(lparam.0) as f32 / self.scale;
                let y = hi(lparam.0) as f32 / self.scale;
                if self.dragging.take().is_some() {
                    unsafe {
                        let _ = ReleaseCapture();
                    }
                    self.sync_tweens();
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
        let link_changed = match (&self.state.connection, &new.connection) {
            (Connection::Connected { link: a, .. }, Connection::Connected { link: b, .. }) => {
                a != b
            }
            _ => false,
        };
        if (!was_conn && now_conn) || link_changed {
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
        } else if !matches!(self.state.camera, Feature::On | Feature::Restarting) {
            self.touch_cam_hud();
        }
        if !matches!(new.mic, Feature::On | Feature::Restarting) {
            self.mic_quick_open = false;
            self.dock_open = false;
        }

        self.state = new;
        self.sync_tweens();
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
        self.sync_tweens();
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
        self.anim = UiAnim::settled_for(&self.state, Drawer::None, false, false, false, false);
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
                    self.sync_tweens();
                    self.invalidate();
                }
                Target::VolFader(_) => {
                    self.dragging = Some(DragFader::Vol);
                    unsafe {
                        SetCapture(self.hwnd);
                    }
                    self.apply_fader_drag(DragFader::Vol, x);
                    self.sync_tweens();
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
                self.sync_tweens();
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
            self.sync_tweens();
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
                self.sync_tweens();
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
                    self.mic_quick_open = false;
                    self.dock_open = false;
                }
            }
            Target::Help => {
                if matches!(self.drawer, Drawer::Help(_)) {
                    self.drawer = Drawer::None;
                } else {
                    self.drawer = Drawer::Help(HelpScreen::Bento);
                    self.cam_menu_open = false;
                    self.mic_quick_open = false;
                    self.dock_open = false;
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
                open_url("https://github.com/diveshpatil9104/owlmic/releases");
            }
            Target::HelpCredit(idx) => {
                if let Some(&(_, user)) = CREDITS.get(idx) {
                    open_url(&format!("https://github.com/{user}"));
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
                    self.cam_menu_wide = match self.state.camera_ratio_label() {
                        "16:9" | "4:3" | "2:1" => Some(true),
                        "9:16" | "3:4" | "1:2" => Some(false),
                        "Auto" => None,
                        _ => self.cam_menu_wide,
                    };
                }
            }
            Target::Ratio(r) => {
                self.cam_menu_wide = match r {
                    "16:9" | "4:3" | "2:1" => Some(true),
                    "9:16" | "3:4" | "1:2" => Some(false),
                    "Auto" => None,
                    _ => self.cam_menu_wide,
                };
                self.touch_cam_hud();
                if self.state.camera_ratio_label() == r {
                    self.cam_menu_open = false;
                } else if let Some(action) = self.state.click(target) {
                    (self.ui.on_action)(action);
                }
            }
            Target::CamOrient => {
                self.touch_cam_hud();
                let ratio = self.state.camera_ratio_label();
                let next = match ratio {
                    "16:9" => Some("9:16"),
                    "9:16" => Some("16:9"),
                    "4:3" => Some("3:4"),
                    "3:4" => Some("4:3"),
                    "2:1" => Some("1:2"),
                    "1:2" => Some("2:1"),
                    _ => None,
                };
                if let Some(next_ratio) = next {
                    self.cam_menu_wide = Some(matches!(next_ratio, "16:9" | "4:3" | "2:1"));
                    (self.ui.on_action)(Action::Set("camera.ratio", next_ratio.to_owned()));
                } else {
                    self.cam_menu_wide = Some(!self.cam_wide());
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
        self.sync_tweens();
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
        let dt = self.anim.drawer_t.now();
        let active_drawer = if self.drawer != Drawer::None {
            Some(self.drawer)
        } else if dt > 0.01 {
            Some(self.anim.last_drawer)
        } else {
            None
        };

        if dt < 0.999 || active_drawer.is_none() {
            let cam_on = self.cam_active();
            let mic_on = self.mic_active();
            self.draw_main(
                cam_on,
                mic_on,
                self.cam_menu_open,
                self.active_quick_audio(),
            );
        }

        if let Some(d) = active_drawer {
            let fade = (dt / 0.75).clamp(0.0, 1.0);
            let dy = 18.0 * (1.0 - dt);
            let drawer_clip = self
                .painter
                .push_rounded_clip(l::PLAYGROUND, l::TILE_RADIUS);
            if !drawer_clip {
                self.painter.clip(l::PLAYGROUND);
            }
            let pushed = self.painter.push_layer(l::PLAYGROUND, fade);
            if dy.abs() > 0.05 {
                self.painter.set_offset(0.0, dy);
            }
            match d {
                Drawer::Wifi => {
                    let mode = match self.state.active_screen(Drawer::Wifi, false, false, false) {
                        Screen::DrawerWifi(m) => m,
                        _ => WifiMode::Offline,
                    };
                    self.draw_drawer_wifi(mode);
                }
                Drawer::Help(h) => self.draw_drawer_help(h),
                Drawer::None => {}
            }
            if dy.abs() > 0.05 {
                self.painter.reset_transform();
            }
            self.painter.pop_layer(pushed);
            if drawer_clip {
                self.painter.pop_layer(true);
            } else {
                self.painter.unclip();
            }
        }
        self.draw_bottom_dock();
    }

    fn draw_playground_base(&mut self) {
        self.painter
            .fill_rounded(l::PLAYGROUND, l::TILE_RADIUS, BLACK);
    }

    /// Draws the 25-bar pure-white waveform matching scripts.js / styles.css exact dimensions
    /// and cosine envelope motion, morphing smoothly as `docked_t` changes 0..1.
    fn draw_waveform_bars(&mut self, center_x: f32, center_y: f32, docked_t: f32) {
        let bar_w = lerp(4.0, 3.0, docked_t);
        let gap = lerp(4.5, 3.5, docked_t);
        let min_h = lerp(4.0, 3.0, docked_t);
        let max_h = lerp(60.0, 18.0, docked_t);
        let total_w = WAVE_BARS as f32 * bar_w + (WAVE_BARS - 1) as f32 * gap;
        let start_x = center_x - total_w / 2.0;

        let raw_level = f32::from_bits(self.ui.meter.load(Ordering::Relaxed));
        let loud = wave_level(raw_level);
        let wt = self.anim.elapsed_secs(self.anim_frame) * 3.3;
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
            let target_h = min_h + (max_h - min_h) * norm_h;
            let prev_h = self.anim.wave_bars_h[i];
            let h = if prev_h <= 0.0 || !self.visible {
                target_h
            } else {
                lerp(prev_h, target_h, 0.42)
            };
            self.anim.wave_bars_h[i] = h;
            let x = start_x + i as f32 * (bar_w + gap);
            let y = center_y - h / 2.0;
            self.painter
                .fill_rounded(Rect::new(x, y, bar_w, h), bar_w / 2.0, WHITE);
        }
    }

    fn draw_owl_switch(&mut self, rect: Rect, on_t: f32, dimmed: bool) {
        let p = &mut self.painter;
        let t = on_t.clamp(0.0, 1.0);
        let r = rect.h / 2.0;
        let on_fill = if dimmed { WHITE_36 } else { WHITE };
        let off_border = if dimmed { WHITE_14 } else { WHITE_28 };
        let off_knob = if dimmed { WHITE_20 } else { WHITE_45 };
        let bg = lerp_color(BLACK, on_fill, t);
        let border = lerp_color(off_border, on_fill, t);
        let knob = lerp_color(off_knob, BLACK, t);
        let knob_x = lerp(rect.x + 3.0, rect.x + rect.w - 3.0 - 12.0, t);
        p.fill_rounded(rect, r, bg);
        p.stroke_rounded(rect, r, border, 1.0);
        p.circle(knob_x + 6.0, rect.y + rect.h / 2.0, 6.0, knob);
    }

    fn draw_hardware_fader(&mut self, rect: Rect, value: f32, live: Option<f32>, expand_t: f32) {
        let p = &mut self.painter;
        let track_h = lerp(7.0, 9.0, expand_t);
        let track_y = rect.y + (rect.h - track_h) / 2.0;
        let track = Rect::new(rect.x, track_y, rect.w, track_h);
        p.fill_rounded(track, track_h / 2.0, WHITE_14);

        if let Some(lvl) = live {
            let lw = (rect.w * lvl.clamp(0.0, 1.0)).max(0.0);
            if lw > 1.0 {
                p.fill_rounded(
                    Rect::new(rect.x, track_y, lw, track_h),
                    track_h / 2.0,
                    WHITE_28,
                );
            }
        }

        let fill_w = (rect.w * value.clamp(0.0, 1.0)).max(0.0);
        if fill_w > 1.0 {
            p.fill_rounded(
                Rect::new(rect.x, track_y, fill_w, track_h),
                track_h / 2.0,
                WHITE,
            );
        }

        // White vertical cap with 1.5 px black halo (matching .fader-cap box-shadow)
        let cap_h = lerp(13.0, 16.0, expand_t);
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
        let nr_val = s.mic_nr_value();
        let vol_val = s.mic_volume_percent();
        let norm_muted = vol_val <= 0.0;
        let iso_t = self.anim.iso_sw_t.now();
        let norm_t = self.anim.norm_sw_t.now();
        let nr_exp = self.anim.nr_drag_t.now();
        let vol_exp = self.anim.vol_drag_t.now();

        self.painter.text(
            crate::messages::SETTING_MIC_ISOLATION,
            ac.iso_label,
            Font::Body,
            WHITE,
            Align::Left,
            false,
        );
        self.draw_owl_switch(ac.iso_switch, iso_t, false);

        self.painter.text(
            crate::messages::SETTING_MIC_NORMALIZATION,
            ac.norm_label,
            Font::Body,
            if norm_muted { WHITE_36 } else { WHITE },
            Align::Left,
            false,
        );
        self.draw_owl_switch(ac.norm_switch, norm_t, norm_muted);

        self.painter.text(
            crate::messages::SETTING_MIC_NOISE_REDUCTION,
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
        self.draw_hardware_fader(ac.nr_fader, nr_val / 10.0, None, nr_exp);

        self.painter.text(
            crate::messages::SETTING_MIC_VOLUME,
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
        let raw_level = if norm_muted {
            0.0
        } else {
            wave_level(f32::from_bits(self.ui.meter.load(Ordering::Relaxed)))
        };
        self.anim.vol_live = lerp(self.anim.vol_live, raw_level, 0.35);
        self.draw_hardware_fader(ac.vol_fader, vf, Some(self.anim.vol_live), vol_exp);
    }

    fn draw_main(&mut self, cam_on: bool, mic_on: bool, _cam_menu: bool, _quick_audio: bool) {
        let s = self.state.clone();
        self.draw_playground_base();
        let c = l::PLAYGROUND;
        let (mid_x, mid_y) = (c.x + c.w / 2.0, c.y + c.h / 2.0);
        let cam_a = self.anim.cam_layer_t.now();
        let hud_mul = self.anim.cam_hud_t.now();
        let menu_t = self.anim.cam_menu_t.now();
        let cam_dock_t = self.anim.wave_dock_cam_t.now();
        let dock_exp_t = self.anim.dock_expand_t.now();
        let quick_t = self.anim.quick_audio_t.now();

        if cam_a < 0.999 {
            self.painter.dots_grid(c.inset(12.0), 18.0, 1.0, 0x161616);
            if !mic_on {
                let wm_alpha = if s.connected() { 0.07 } else { 0.07 * 0.45 };
                self.painter.watermark(mid_x, mid_y, 152.0, wm_alpha);
            }
        }

        if cam_on || cam_a > 0.01 {
            let cam_clip = self
                .painter
                .push_rounded_clip(l::PLAYGROUND, l::TILE_RADIUS);
            if !cam_clip {
                self.painter.clip(l::PLAYGROUND);
            }
            let pushed = self.painter.push_layer(l::PLAYGROUND, cam_a);
            if cam_a < 0.999 {
                self.draw_playground_base();
            }

            let ratio_lbl = s.camera_ratio_label();
            let (pw, ph) = self.ui.preview.size();
            let (cam_rect, crop_mode) = l::cam_crop_rect_for_stream(ratio_lbl, pw, ph);

            if self.ui.preview.seq() > 0 && pw > 0 && ph > 0 {
                let seq = self.ui.preview.seq();
                let preview = self.ui.preview.clone();
                let ambient = (crop_mode != CropMode::None).then_some(l::PLAYGROUND);
                self.painter
                    .picture(cam_rect, ambient, pw, ph, seq, |copy| {
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

            // Camera HUD controls (group-dimmed cleanly via layer opacity so backgrounds stay translucent)
            let hud_layer = self.painter.push_layer(l::PLAYGROUND, hud_mul);

            let menu_rect = lerp_rect(l::CAM_MENU_BTN, l::CAM_EXPAND_MENU, menu_t);
            let menu_radius = lerp(7.0, 10.0, menu_t);
            let menu_bg =
                if menu_t < 0.05 && self.is_hovered(Target::CamMenu) && self.hud_is_bright() {
                    BTN_HOVER
                } else {
                    BLACK
                };
            let menu_clip = self.painter.push_rounded_clip(menu_rect, menu_radius);
            if !menu_clip {
                self.painter.clip(menu_rect);
            }
            self.painter.fill_rounded_alpha(
                menu_rect,
                menu_radius,
                menu_bg,
                lerp(0.72, 0.94, menu_t),
            );

            let cur_ratio = s.camera_ratio_label();
            if menu_t > 0.05 {
                let grid_alpha = ((menu_t - 0.16) / 0.84).clamp(0.0, 1.0);
                if grid_alpha > 0.01 {
                    let grid_layer = self.painter.push_layer(l::CAM_EXPAND_MENU, grid_alpha);
                    let cam_wide = self.cam_wide();
                    let ratio_cells = if cam_wide {
                        [
                            (0, 0, "Auto"),
                            (1, 0, "16:9"),
                            (1, 1, "4:3"),
                            (2, 0, "2:1"),
                            (2, 1, "1:1"),
                        ]
                    } else {
                        [
                            (0, 0, "Auto"),
                            (1, 0, "9:16"),
                            (1, 1, "3:4"),
                            (2, 0, "1:2"),
                            (2, 1, "1:1"),
                        ]
                    };
                    for (row, col, val) in ratio_cells {
                        let r = l::cam_option_rect(row, col);
                        let target = Target::Ratio(val);
                        let sel = cur_ratio == val;
                        let hov = menu_t > 0.9 && self.is_hovered(target);
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

                    // Row 0 Col 1: Rotating phone symbol
                    let hov_o = menu_t > 0.9 && self.is_hovered(Target::CamOrient);
                    if hov_o {
                        self.painter.fill(l::CAM_ORIENT_BTN, ROW_HOVER);
                    }
                    let orient_fg = if hov_o { WHITE } else { WHITE_65 };
                    let cx = l::CAM_ORIENT_BTN.x + l::CAM_ORIENT_BTN.w * 0.5;
                    let cy = l::CAM_ORIENT_BTN.y + l::CAM_ORIENT_BTN.h * 0.5;
                    self.painter.icon_rotated(
                        icons::PHONE,
                        cx,
                        cy,
                        13.0,
                        orient_fg,
                        self.anim.cam_orient_t.now() * 90.0,
                    );

                    let col_x = l::CAM_EXPAND_MENU.x + l::CAM_EXPAND_MENU.w * 0.5;
                    self.painter.line(
                        col_x,
                        l::CAM_EXPAND_MENU.y + 1.0,
                        col_x,
                        l::CAM_EXPAND_MENU.bottom() - 1.0,
                        WHITE_10,
                        1.0,
                    );
                    for row in 1..=2 {
                        let div_y =
                            l::CAM_EXPAND_MENU.y + row as f32 * (l::CAM_EXPAND_MENU.h / 3.0);
                        self.painter.line(
                            l::CAM_EXPAND_MENU.x + 1.0,
                            div_y,
                            l::CAM_EXPAND_MENU.right() - 1.0,
                            div_y,
                            WHITE_10,
                            1.0,
                        );
                    }
                    self.painter.pop_layer(grid_layer);
                }
            }

            let closed_alpha = (1.0 - menu_t * 2.0).clamp(0.0, 1.0);
            if closed_alpha > 0.01 {
                let closed_layer = self.painter.push_layer(l::CAM_MENU_BTN, closed_alpha);
                self.painter.text(
                    cur_ratio,
                    l::CAM_MENU_BTN,
                    Font::Caption,
                    WHITE,
                    Align::Center,
                    false,
                );
                self.painter.pop_layer(closed_layer);
            }
            if menu_clip {
                self.painter.pop_layer(true);
            } else {
                self.painter.unclip();
            }
            self.painter.stroke_rounded(
                menu_rect,
                menu_radius,
                lerp_color(WHITE_22, WHITE_36, menu_t),
                1.0,
            );

            // Mirror button (white fill + black icon when active, translucent black when off)
            let mirror_on = s.is_on("camera.mirror", true);
            if mirror_on {
                self.painter.fill_rounded(l::CAM_MIRROR_BTN, 7.0, WHITE);
                self.painter
                    .stroke_rounded(l::CAM_MIRROR_BTN, 7.0, WHITE, 1.0);
            } else {
                let bg = if self.is_hovered(Target::Mirror) && self.hud_is_bright() {
                    BTN_HOVER
                } else {
                    BLACK
                };
                self.painter
                    .fill_rounded_alpha(l::CAM_MIRROR_BTN, 7.0, bg, 0.72);
                self.painter
                    .stroke_rounded(l::CAM_MIRROR_BTN, 7.0, WHITE_20, 1.0);
            }
            self.painter.icon(
                icons::MIRROR,
                l::CAM_MIRROR_BTN.x + l::CAM_MIRROR_BTN.w / 2.0,
                l::CAM_MIRROR_BTN.y + l::CAM_MIRROR_BTN.h / 2.0,
                13.0,
                if mirror_on { BLACK } else { WHITE },
            );

            // Flip lens button
            let flip_bg = if self.is_hovered(Target::Flip) && self.hud_is_bright() {
                BTN_HOVER
            } else {
                BLACK
            };
            self.painter
                .fill_rounded_alpha(l::CAM_FLIP_BTN, 7.0, flip_bg, 0.72);
            self.painter
                .stroke_rounded(l::CAM_FLIP_BTN, 7.0, WHITE_20, 1.0);
            self.painter.icon(
                icons::FLIP,
                l::CAM_FLIP_BTN.x + l::CAM_FLIP_BTN.w / 2.0,
                l::CAM_FLIP_BTN.y + l::CAM_FLIP_BTN.h / 2.0,
                13.0,
                WHITE,
            );

            self.painter.pop_layer(hud_layer);

            self.painter.pop_layer(pushed);
            if cam_clip {
                self.painter.pop_layer(true);
            } else {
                self.painter.unclip();
            }
        }

        if mic_on {
            // Mic-only Quick Audio bottom slide panel
            if quick_t > 0.01 && cam_dock_t < 0.99 {
                let qp = l::AUDIO_QUICK_PANEL;
                let fade = (quick_t / 0.75).clamp(0.0, 1.0) * (1.0 - cam_dock_t);
                let dy = qp.h * (1.0 - quick_t);
                self.painter.clip(l::PLAYGROUND);
                let pushed = self.painter.push_layer(l::PLAYGROUND, fade);
                if dy.abs() > 0.05 {
                    self.painter.set_offset(0.0, dy);
                }
                self.painter.fill_rounded(qp, l::TILE_RADIUS, BLACK);
                self.painter
                    .fill(Rect::new(qp.x, qp.y, qp.w, l::TILE_RADIUS), BLACK);
                self.painter
                    .line(qp.x, qp.y, qp.right(), qp.y, WHITE_15, 1.0);
                self.draw_audio_controls(qp);
                if dy.abs() > 0.05 {
                    self.painter.reset_transform();
                }
                self.painter.pop_layer(pushed);
                self.painter.unclip();
            }

            // Unified morphing Waveform Dock
            let centered_rect = lerp_rect(l::WAVE_DOCK_CENTERED, l::WAVE_DOCK_LIFTED, quick_t);
            let docked_rect = lerp_rect(l::WAVE_DOCK_DOCKED, l::WAVE_DOCK_EXPANDED, dock_exp_t);
            let dock = lerp_rect(centered_rect, docked_rect, cam_dock_t);
            let wave_cy = lerp(
                centered_rect.y + centered_rect.h / 2.0,
                l::WAVE_DOCK_DOCKED.y + l::WAVE_DOCK_DOCKED.h / 2.0,
                cam_dock_t,
            );

            if cam_dock_t > 0.01 {
                self.painter.clip(dock);
                self.painter
                    .fill_rounded_alpha(dock, 10.0, BLACK, cam_dock_t);

                if dock_exp_t > 0.05 {
                    let ctrl_alpha = ((dock_exp_t - 0.15) / 0.85).clamp(0.0, 1.0) * cam_dock_t;
                    if ctrl_alpha > 0.01 {
                        let settings_r =
                            Rect::new(dock.x, dock.y, dock.w, l::WAVE_DOCK_EXPANDED_SETTINGS.h);
                        let pushed = self.painter.push_layer(dock, ctrl_alpha);
                        self.draw_audio_controls(settings_r);
                        let hdr_y = dock.bottom() - l::WAVE_DOCK_EXPANDED_HEADER.h;
                        self.painter
                            .line(dock.x, hdr_y, dock.right(), hdr_y, WHITE_15, 1.0);
                        self.painter.pop_layer(pushed);
                    }
                }

                self.draw_waveform_bars(dock.x + dock.w / 2.0, wave_cy, cam_dock_t);
                let bd = lerp_color(WHITE_28, WHITE_32, dock_exp_t);
                self.painter
                    .stroke_rounded_alpha(dock, 10.0, bd, cam_dock_t, 1.0);
                self.painter.unclip();
            } else {
                self.draw_waveform_bars(dock.x + dock.w / 2.0, wave_cy, 0.0);
            }
        }
    }

    fn draw_bottom_tile(
        &mut self,
        idx: usize,
        icon: crate::glyph::Icon,
        on_t: f32,
        hov_t: f32,
        off_fg: u32,
    ) {
        let rect = l::bottom(idx);
        let bg = lerp_color(
            lerp_color(BLACK, BTN_HOVER, hov_t),
            lerp_color(WHITE, BTN_ON_HOVER, hov_t),
            on_t,
        );
        let fg = lerp_color(lerp_color(off_fg, WHITE, hov_t), BLACK, on_t);
        self.painter.fill_rounded(rect, l::TILE_RADIUS, bg);
        self.painter.icon(
            icon,
            rect.x + rect.w / 2.0,
            rect.y + rect.h / 2.0,
            DOCK_ICON,
            fg,
        );
    }

    fn draw_bottom_dock(&mut self) {
        let s = self.state.clone();
        let connected = s.connected();
        let is_fully_connected = matches!(s.connection, Connection::Connected { .. });
        let secs = self.anim.elapsed_secs(self.anim_frame);
        let pulse_t = 0.5 + 0.5 * (secs * std::f32::consts::TAU / 1.1).sin();

        // b0: Connection drawer toggle (globe + 1.1s pulseFade until fully connected)
        let ind = indicator(&s.connection);
        let b0_off_fg = if !is_fully_connected {
            gray(0.35 + 0.65 * pulse_t)
        } else {
            WHITE_78
        };
        let b0_on = self.anim.btn_on[0].now();
        let b0_hov = if is_fully_connected || self.drawer == Drawer::Wifi {
            self.anim.btn_hov[0].now()
        } else {
            0.0
        };
        self.draw_bottom_tile(0, ind.icon, b0_on, b0_hov, b0_off_fg);

        // b1: Camera, b2: Mic, b3: Speaker
        let media = [
            (
                1,
                matches!(s.camera, Feature::On | Feature::Restarting) && connected,
                icons::CAMERA,
                icons::CAMERA_OFF,
            ),
            (
                2,
                matches!(s.mic, Feature::On | Feature::Restarting) && connected,
                icons::MIC,
                icons::MIC_OFF,
            ),
            (
                3,
                matches!(s.speaker, Feature::On | Feature::Restarting) && connected,
                icons::SPEAKER,
                icons::SPEAKER_OFF,
            ),
        ];

        for (idx, on, ic_on, ic_off) in media {
            let off_fg = if !connected { WHITE_28 } else { WHITE_78 };
            let on_t = self.anim.btn_on[idx].now();
            let hov_t = self.anim.btn_hov[idx].now();
            self.draw_bottom_tile(idx, if on { ic_on } else { ic_off }, on_t, hov_t, off_fg);
        }

        // b4: Help drawer toggle
        let b4_on = self.anim.btn_on[4].now();
        let b4_hov = self.anim.btn_hov[4].now();
        self.draw_bottom_tile(4, icons::HELP, b4_on, b4_hov, WHITE_78);
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
        let is_pairing_or_connecting = mode == WifiMode::Pairing
            || matches!(
                s.connection,
                Connection::Connecting { .. } | Connection::Reconnecting { .. }
            );
        let show_empty = s.phones.is_empty()
            && !s.connected()
            && !matches!(
                s.connection,
                Connection::Approval { .. } | Connection::Connecting { .. }
            );

        let secs = self.anim.elapsed_secs(self.anim_frame);
        let pulse_t = 0.5 + 0.5 * (secs * std::f32::consts::TAU / 1.1).sin();
        let row_ic_color = if mode == WifiMode::Connected {
            WHITE
        } else if is_pairing_or_connecting {
            gray(0.35 + 0.55 * pulse_t)
        } else {
            WHITE_25
        };
        let row_icon = match &s.connection {
            Connection::Connected { link, .. }
            | Connection::Approval { link, .. }
            | Connection::Connecting { link }
            | Connection::Reconnecting { link, .. } => link_icon(*link),
            Connection::NotConnected => icons::GLOBE,
        };
        self.painter.icon(
            row_icon,
            prim.x + 14.0 + 9.0,
            prim.y + prim.h / 2.0,
            17.0,
            row_ic_color,
        );

        let (name, name_color, subtitle, sub_color) = if show_empty {
            (
                crate::messages::PC_NO_PHONE,
                WHITE_50,
                crate::messages::PC_OPEN_PHONE.to_owned(),
                WHITE_35,
            )
        } else {
            let name = s.primary_phone_name();
            let name_color = match mode {
                WifiMode::Connected | WifiMode::Pairing => WHITE,
                _ => WHITE_50,
            };
            let (subtitle, sub_color) = match &s.connection {
                Connection::Connecting { .. } => {
                    (crate::messages::LINK_CONNECTING.to_owned(), WHITE_70)
                }
                Connection::Reconnecting { .. } => {
                    (crate::messages::STATUS_RECONNECTING.to_owned(), WHITE_70)
                }
                _ => match mode {
                    WifiMode::Pairing => {
                        (crate::messages::STATUS_WANTS_TO_PAIR.to_owned(), WHITE_70)
                    }
                    WifiMode::Connected => (ind.label.to_owned(), WHITE_50),
                    WifiMode::Blocked => (crate::messages::PC_BLOCKED.to_owned(), WHITE_35),
                    WifiMode::Forget | WifiMode::Offline => (
                        self.format_last_connected()
                            .unwrap_or_else(|| crate::messages::STATUS_OFFLINE.to_owned()),
                        WHITE_35,
                    ),
                },
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
        let is_sweeping = is_pairing_or_connecting;
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
            } else {
                self.sync_flash_at = None;
            }
        }
        if self.sync_flash_at.is_none() && is_sweeping {
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
        let spin_angle = self.refresh_at.map_or(0.0, |t| {
            let p = (t.elapsed().as_secs_f32() / 0.420).clamp(0.0, 1.0);
            cubic_bezier(0.22, 1.0, 0.36, 1.0, p) * 360.0
        });
        self.painter.icon_rotated(
            icons::REFRESH,
            ref_btn.x + ref_btn.w / 2.0,
            ref_btn.y + ref_btn.h / 2.0,
            15.0,
            if is_spinning { WHITE } else { WHITE_80 },
            spin_angle,
        );

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
        let cycle = (self.anim.elapsed_secs(self.anim_frame) / 2.1).fract();
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
        self.draw_playground_base();
        let sub_t = self.anim.help_sub_t.now();
        let sub_screen = if !matches!(screen, HelpScreen::Bento) {
            screen
        } else {
            self.anim.last_help
        };

        if sub_t < 0.99 {
            let pushed = self.painter.push_layer(l::PLAYGROUND, 1.0 - sub_t);
            self.draw_help_page(HelpScreen::Bento);
            self.painter.pop_layer(pushed);
        }
        if sub_t > 0.01 {
            let pushed = self.painter.push_layer(l::PLAYGROUND, sub_t);
            self.draw_help_page(sub_screen);
            self.painter.pop_layer(pushed);
        }
    }

    fn draw_help_page(&mut self, screen: HelpScreen) {
        let s = self.state.clone();
        match screen {
            HelpScreen::Bento => {
                let items: [(usize, Target, &'static str, crate::glyph::Icon); 6] = [
                    (
                        0,
                        Target::HelpHowTo,
                        crate::messages::UI_HOW_TO,
                        icons::LIST,
                    ),
                    (
                        1,
                        Target::HelpPrivacy,
                        crate::messages::UI_PRIVACY,
                        icons::SHIELD,
                    ),
                    (2, Target::HelpRate, crate::messages::UI_RATE, icons::STAR),
                    (
                        3,
                        Target::HelpGitHub,
                        crate::messages::UI_GITHUB,
                        icons::GITHUB,
                    ),
                    (
                        4,
                        Target::HelpDonate,
                        crate::messages::UI_DONATE,
                        icons::COFFEE,
                    ),
                    (
                        5,
                        Target::HelpCredits,
                        crate::messages::UI_CREDITS,
                        icons::USERS,
                    ),
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
                self.draw_help_sub_header(crate::messages::UI_HOW_TO);

                let steps: [(i32, f32, &'static str, &'static str); 3] = [
                    (
                        1,
                        54.0,
                        crate::messages::PC_STEP_DOWNLOAD,
                        crate::messages::PC_STEP_DOWNLOAD_DETAIL,
                    ),
                    (
                        2,
                        112.0,
                        crate::messages::PC_STEP_CONNECT,
                        crate::messages::PC_STEP_CONNECT_DETAIL,
                    ),
                    (
                        3,
                        174.0,
                        crate::messages::PC_STEP_STREAM,
                        crate::messages::PC_STEP_STREAM_DETAIL,
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
                    self.painter.text(
                        title,
                        Rect::new(title_x, y, text_w, 18.0),
                        Font::Body,
                        WHITE,
                        Align::Left,
                        false,
                    );
                    if idx == 1 {
                        self.painter.icon(
                            icons::EXTERNAL,
                            title_x + 132.0,
                            y + 9.0,
                            12.0,
                            WHITE_70,
                        );
                        if self.is_hovered(Target::HelpReleases) {
                            self.painter.line(
                                title_x,
                                y + 17.0,
                                title_x + 122.0,
                                y + 17.0,
                                WHITE_70,
                                1.0,
                            );
                        }
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
                self.draw_help_sub_header(crate::messages::UI_PRIVACY);
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
                self.draw_help_sub_header(crate::messages::UI_CREDITS);
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
                    crate::messages::UI_GITHUB,
                    Rect::new(gh_row.x + 14.0, gh_row.y, 140.0, gh_row.h),
                    Font::Body,
                    WHITE,
                    Align::Left,
                    false,
                );
                self.painter.icon(
                    icons::EXTERNAL,
                    gh_row.right() - 22.0,
                    gh_row.y + gh_row.h / 2.0,
                    12.0,
                    WHITE_55,
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

fn wave_level(raw_level: f32) -> f32 {
    const WAVE_THRESHOLD: f32 = 0.08;
    const WAVE_EXPONENT: f32 = 0.7;
    const WAVE_GAIN: f32 = 1.5;
    let v = ((raw_level - WAVE_THRESHOLD) / (1.0 - WAVE_THRESHOLD)).clamp(0.0, 1.0);
    (v.powf(WAVE_EXPONENT) * WAVE_GAIN).min(1.0)
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
                let anim =
                    UiAnim::settled_for(&arc_st, drawer, cam_hud, cam_menu, mic_quick, dock_open);
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
                    cam_menu_wide: None,
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
                    anim,
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

    #[test]
    fn wave_level_matches_android_sensitivity_curve() {
        assert_eq!(wave_level(0.08), 0.0);
        assert!((wave_level(0.16) - 0.27).abs() < 0.02);
        assert!((wave_level(0.25) - 0.46).abs() < 0.02);
        assert!((wave_level(0.50) - 0.86).abs() < 0.02);
        assert_eq!(wave_level(0.65), 1.0);
        assert_eq!(wave_level(1.0), 1.0);
    }

    #[test]
    fn animation_tweens_settle_and_progress_smoothly() {
        let mut tw = Tween::settled(0.0, 240, Curve::EaseOutExpo);
        assert!((tw.now() - 0.0).abs() < 1e-4);
        tw.set(1.0);
        std::thread::sleep(Duration::from_millis(25));
        let mid = tw.now();
        assert!(mid > 0.0 && mid <= 1.0);
        tw.snap(0.25);
        assert!((tw.now() - 0.25).abs() < 1e-4);

        let st = PanelState {
            connection: Connection::Connected {
                phone: "Pixel 8".into(),
                link: 3,
                weak: false,
            },
            camera: Feature::On,
            mic: Feature::On,
            paired: true,
            ..Default::default()
        };
        let anim = UiAnim::settled_for(&st, Drawer::Wifi, false, true, false, true);
        assert!((anim.drawer_t.now() - 1.0).abs() < 1e-4);
        assert!((anim.cam_layer_t.now() - 1.0).abs() < 1e-4);
        assert!((anim.cam_menu_t.now() - 1.0).abs() < 1e-4);
        assert!((anim.dock_expand_t.now() - 1.0).abs() < 1e-4);
        assert!((anim.cam_hud_t.now() - 1.0).abs() < 1e-4);
    }
}
