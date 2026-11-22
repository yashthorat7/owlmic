//! Layout geometry of the 312 x 374 px PC Bento Flyout (matching shared/design/prototypes).
//! Every gap is the 1 px hairline the background shows through.

use crate::panel as p;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }

    pub const fn right(&self) -> f32 {
        self.x + self.w
    }

    pub const fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn inset(&self, by: f32) -> Rect {
        Rect::new(
            self.x + by,
            self.y + by,
            self.w - 2.0 * by,
            self.h - 2.0 * by,
        )
    }
}

pub const GAP: f32 = 1.0;
pub const WIDTH: f32 = p::WIDTH_PX;
pub const HEIGHT: f32 = p::HEIGHT_PX;

/// 310 x 310 square Bento Playground (312 x 312 outer bounds including 1 px padding).
pub const PLAYGROUND: Rect = Rect::new(0.0, 0.0, WIDTH, HEIGHT - p::BOTTOM_ROW_PX - GAP);
pub const CAMERA: Rect = PLAYGROUND;

const BOTTOM_Y: f32 = HEIGHT - p::BOTTOM_ROW_PX;
const WIDTHS: [f32; 5] = [61.0, 61.0, 61.0, 61.0, 64.0];
const XS: [f32; 5] = [0.0, 62.0, 124.0, 186.0, 248.0];

/// 5 Square Bento Buttons in a single line below (Wi-Fi, Cam, Mic, Speaker, Help).
pub const fn bottom(i: usize) -> Rect {
    Rect::new(XS[i], BOTTOM_Y, WIDTHS[i], p::BOTTOM_ROW_PX)
}

pub const BTN_WIFI: Rect = bottom(0);
pub const BTN_CAM: Rect = bottom(1);
pub const BTN_MIC: Rect = bottom(2);
pub const BTN_SPK: Rect = bottom(3);
pub const BTN_HELP: Rect = bottom(4);

pub const MIC: Rect = BTN_MIC;
pub const SPEAKER: Rect = BTN_SPK;
pub const PHONE: Rect = PLAYGROUND;

/// The Repair tile over the top of the playground area.
pub const REPAIR: Rect = Rect::new(0.0, 0.0, WIDTH, 64.0);
pub fn repair_button() -> Rect {
    Rect::new(
        REPAIR.right() - 12.0 - 80.0,
        REPAIR.y + (REPAIR.h - 32.0) / 2.0,
        80.0,
        32.0,
    )
}

/// The approve gate: a message tile over Allow and Deny.
pub const GATE: Rect = Rect::new(0.0, 0.0, WIDTH, BOTTOM_Y - GAP);
pub const ALLOW: Rect = Rect::new(0.0, BOTTOM_Y, (WIDTH - GAP) / 2.0 - 0.5, p::BOTTOM_ROW_PX);
pub const DENY: Rect = Rect::new(
    ALLOW.w + GAP,
    BOTTOM_Y,
    WIDTH - ALLOW.w - GAP,
    p::BOTTOM_ROW_PX,
);

// Camera HUD Controls
pub const CAM_MENU_BTN: Rect = Rect::new(10.0, 10.0, 44.0, 25.0);
pub const CAM_EXPAND_MENU: Rect = Rect::new(10.0, 10.0, 156.0, 103.0);
pub const CAM_MIRROR_BTN: Rect = Rect::new(246.0, 10.0, 25.0, 25.0);
pub const CAM_FLIP_BTN: Rect = Rect::new(277.0, 10.0, 25.0, 25.0);

pub fn cam_option_rect(row: usize, col: usize) -> Rect {
    let w = CAM_EXPAND_MENU.w / 3.0;
    let h = 25.0;
    let y = CAM_EXPAND_MENU.y + 28.0 + row as f32 * h;
    let x = CAM_EXPAND_MENU.x + col as f32 * w;
    Rect::new(x, y, w, h)
}

// Waveform Dock Insets
pub const WAVE_DOCK_CENTERED: Rect = Rect::new(
    (WIDTH - 248.0) / 2.0,
    (PLAYGROUND.h - 68.0) / 2.0,
    248.0,
    68.0,
);
pub const WAVE_DOCK_DOCKED: Rect = Rect::new(
    (WIDTH - 204.0) / 2.0,
    PLAYGROUND.bottom() - 12.0 - 22.0,
    204.0,
    22.0,
);

// Audio Quick Panel Controls
pub const AUDIO_QUICK_PANEL: Rect = Rect::new(0.0, PLAYGROUND.bottom() - 150.0, WIDTH, 150.0);
pub const AUDIO_SWITCH_ISO: Rect =
    Rect::new(WIDTH - 14.0 - 34.0, AUDIO_QUICK_PANEL.y + 12.0, 34.0, 18.0);
pub const AUDIO_SWITCH_NORM: Rect =
    Rect::new(WIDTH - 14.0 - 34.0, AUDIO_QUICK_PANEL.y + 38.0, 34.0, 18.0);
pub const AUDIO_SLIDER_NR: Rect = Rect::new(14.0, AUDIO_QUICK_PANEL.y + 72.0, WIDTH - 28.0, 18.0);
pub const AUDIO_SLIDER_VOL: Rect = Rect::new(14.0, AUDIO_QUICK_PANEL.y + 116.0, WIDTH - 28.0, 18.0);

// Connection Drawer Controls
pub const CONN_COPY_IP_BTN: Rect = Rect::new(WIDTH - 14.0 - 28.0, 12.0, 28.0, 28.0);
pub const CONN_PRIMARY_ROW: Rect = Rect::new(0.0, 50.0, WIDTH, 52.0);
pub const CONN_DISCONNECT_BTN: Rect = Rect::new(WIDTH - 14.0 - 45.0 - 28.0, 62.0, 28.0, 28.0);
pub const CONN_BLOCK_BTN: Rect = Rect::new(WIDTH - 14.0 - 40.0, 62.0, 40.0, 28.0);
pub const CONN_PAIR_BTN: Rect = Rect::new(WIDTH - 14.0 - 28.0 - 6.0 - 28.0, 62.0, 28.0, 28.0);
pub const CONN_DECLINE_BTN: Rect = Rect::new(WIDTH - 14.0 - 28.0, 62.0, 28.0, 28.0);
pub const CONN_UNBLOCK_BTN: Rect = Rect::new(WIDTH - 14.0 - 55.0, 62.0, 55.0, 28.0);
pub const CONN_PAIRED_ROW: Rect = Rect::new(0.0, 104.0, WIDTH, 50.0);
pub const CONN_REMOVE_PAIRED_BTN: Rect = Rect::new(WIDTH - 14.0 - 55.0, 115.0, 55.0, 28.0);
pub const CONN_FOOTER: Rect = Rect::new(0.0, PLAYGROUND.bottom() - 44.0, WIDTH, 44.0);
pub const CONN_REFRESH_BTN: Rect = Rect::new(8.0, CONN_FOOTER.y + 4.0, 36.0, 36.0);

pub fn conn_transport_btn(i: usize) -> Rect {
    let x = 52.0 + i as f32 * 62.0;
    Rect::new(x, CONN_FOOTER.y + 4.0, 56.0, 36.0)
}

// Help Drawer Bento Grid
pub fn help_bento_tile(i: usize) -> Rect {
    let col = i % 3;
    let row = i / 3;
    let tile_w = 80.0;
    let tile_h = 80.0;
    let gap = 8.0;
    let total_w = 3.0 * tile_w + 2.0 * gap;
    let start_x = (WIDTH - total_w) / 2.0;
    let start_y = 50.0;
    Rect::new(
        start_x + col as f32 * (tile_w + gap),
        start_y + row as f32 * (tile_h + gap),
        tile_w,
        tile_h,
    )
}

pub const HELP_BACK_BTN: Rect = Rect::new(10.0, 8.0, 28.0, 28.0);

// Legacy Settings View support
pub const BACK: Rect = Rect::new(0.0, 0.0, WIDTH, 40.0);
pub const LIST: Rect = Rect::new(0.0, BACK.h + GAP, WIDTH, HEIGHT - BACK.h - GAP);
pub const ROW: f32 = 32.0;
pub const ROW_BUTTON: f32 = 88.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HelpScreen {
    #[default]
    Bento,
    HowTo,
    Credits,
    Privacy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Drawer {
    #[default]
    None,
    Wifi,
    Help(HelpScreen),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target {
    // Bottom 5 bento navigation buttons
    Wifi,
    Camera,
    Mic,
    Speaker,
    Help,

    // Camera controls
    CamMenu,
    Quality(&'static str),
    Fps(&'static str),
    Ratio(&'static str),
    Mirror,
    Flip,

    // Audio controls
    WaveDock,
    ToggleIso,
    ToggleNorm,
    NrFader(f32),
    VolFader(f32),

    // Connection drawer
    CopyIp,
    Disconnect,
    Block,
    Unblock,
    Pair,
    Decline,
    RemovePaired,
    Refresh,
    ToggleTransport(u8),

    // Help drawer
    HelpHowTo,
    HelpPrivacy,
    HelpRate,
    HelpGitHub,
    HelpDonate,
    HelpCredits,
    HelpBack,

    // Gate & Repair
    Allow,
    Deny,
    Repair,

    // Legacy / fallback targets
    Phone,
    Framing,
    Lens,
    Settings,
    Indicator,
    Back,
    Row(usize, u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Main {
        repair: bool,
        cam_on: bool,
        mic_on: bool,
        cam_menu: bool,
        quick_audio: bool,
    },
    Gate,
    Settings,
    DrawerWifi,
    DrawerHelp(HelpScreen),
}

impl Screen {
    pub const fn main_screen(repair: bool) -> Self {
        Screen::Main {
            repair,
            cam_on: false,
            mic_on: false,
            cam_menu: false,
            quick_audio: false,
        }
    }
}

/// What sits under (x, y) in panel pixels at 100%.
pub fn hit(screen: Screen, x: f32, y: f32, scroll: f32, rows: usize) -> Option<Target> {
    match screen {
        Screen::Gate => [(ALLOW, Target::Allow), (DENY, Target::Deny)]
            .into_iter()
            .find(|(r, _)| r.contains(x, y))
            .map(|(_, t)| t),

        Screen::Settings => {
            if BACK.contains(x, y) {
                return Some(Target::Back);
            }
            if !LIST.contains(x, y) {
                return None;
            }
            let row = ((y - LIST.y + scroll) / (ROW + GAP)) as usize;
            if row >= rows {
                return None;
            }
            let from_right = WIDTH - x;
            let button = if from_right < ROW_BUTTON {
                2
            } else if from_right < 2.0 * ROW_BUTTON + GAP {
                1
            } else {
                0
            };
            Some(Target::Row(row, button))
        }

        _ => {
            // 1. Bottom 5 Square Bento Dock Buttons always take precedence for Main and Drawers
            if BTN_WIFI.contains(x, y) {
                return Some(Target::Wifi);
            }
            if BTN_CAM.contains(x, y) {
                return Some(Target::Camera);
            }
            if BTN_MIC.contains(x, y) {
                return Some(Target::Mic);
            }
            if BTN_SPK.contains(x, y) {
                return Some(Target::Speaker);
            }
            if BTN_HELP.contains(x, y) {
                return Some(Target::Help);
            }

            match screen {
                Screen::DrawerWifi => {
                    if CONN_COPY_IP_BTN.contains(x, y) {
                        return Some(Target::CopyIp);
                    }
                    if CONN_DISCONNECT_BTN.contains(x, y) {
                        return Some(Target::Disconnect);
                    }
                    if CONN_BLOCK_BTN.contains(x, y) {
                        return Some(Target::Block);
                    }
                    if CONN_PAIR_BTN.contains(x, y) {
                        return Some(Target::Pair);
                    }
                    if CONN_DECLINE_BTN.contains(x, y) {
                        return Some(Target::Decline);
                    }
                    if CONN_UNBLOCK_BTN.contains(x, y) {
                        return Some(Target::Unblock);
                    }
                    if CONN_REMOVE_PAIRED_BTN.contains(x, y) {
                        return Some(Target::RemovePaired);
                    }
                    if CONN_REFRESH_BTN.contains(x, y) {
                        return Some(Target::Refresh);
                    }
                    for i in 0..4 {
                        if conn_transport_btn(i).contains(x, y) {
                            return Some(Target::ToggleTransport(i as u8 + 1));
                        }
                    }
                    if PLAYGROUND.contains(x, y) {
                        return Some(Target::Wifi);
                    }
                    None
                }

                Screen::DrawerHelp(help_screen) => {
                    match help_screen {
                        HelpScreen::Bento => {
                            if help_bento_tile(0).contains(x, y) {
                                return Some(Target::HelpHowTo);
                            }
                            if help_bento_tile(1).contains(x, y) {
                                return Some(Target::HelpPrivacy);
                            }
                            if help_bento_tile(2).contains(x, y) {
                                return Some(Target::HelpRate);
                            }
                            if help_bento_tile(3).contains(x, y) {
                                return Some(Target::HelpGitHub);
                            }
                            if help_bento_tile(4).contains(x, y) {
                                return Some(Target::HelpDonate);
                            }
                            if help_bento_tile(5).contains(x, y) {
                                return Some(Target::HelpCredits);
                            }
                        }
                        _ => {
                            if HELP_BACK_BTN.contains(x, y) {
                                return Some(Target::HelpBack);
                            }
                        }
                    }
                    if PLAYGROUND.contains(x, y) {
                        return Some(Target::Help);
                    }
                    None
                }

                Screen::Main {
                    repair,
                    cam_on,
                    mic_on,
                    cam_menu,
                    quick_audio,
                } => {
                    if repair && repair_button().contains(x, y) {
                        return Some(Target::Repair);
                    }

                    if cam_on {
                        if cam_menu {
                            // Expanded Format Menu options:
                            let res_opts = ["1080p", "720p", "480p"];
                            let fps_opts = ["60", "30", "24"];
                            let ratio_opts = ["16:9", "4:3", "1:1"];

                            for (c, &opt) in res_opts.iter().enumerate() {
                                if cam_option_rect(0, c).contains(x, y) {
                                    return Some(Target::Quality(opt));
                                }
                            }
                            for (c, &opt) in fps_opts.iter().enumerate() {
                                if cam_option_rect(1, c).contains(x, y) {
                                    return Some(Target::Fps(opt));
                                }
                            }
                            for (c, &opt) in ratio_opts.iter().enumerate() {
                                if cam_option_rect(2, c).contains(x, y) {
                                    return Some(Target::Ratio(opt));
                                }
                            }
                            if CAM_EXPAND_MENU.contains(x, y) {
                                return Some(Target::CamMenu);
                            }
                        } else if CAM_MENU_BTN.contains(x, y) {
                            return Some(Target::CamMenu);
                        }

                        if CAM_MIRROR_BTN.contains(x, y) {
                            return Some(Target::Mirror);
                        }
                        if CAM_FLIP_BTN.contains(x, y) {
                            return Some(Target::Flip);
                        }
                        if mic_on && WAVE_DOCK_DOCKED.contains(x, y) {
                            return Some(Target::WaveDock);
                        }
                    } else if mic_on {
                        if quick_audio {
                            if AUDIO_SWITCH_ISO.contains(x, y) {
                                return Some(Target::ToggleIso);
                            }
                            if AUDIO_SWITCH_NORM.contains(x, y) {
                                return Some(Target::ToggleNorm);
                            }
                            if AUDIO_SLIDER_NR.contains(x, y) {
                                let ratio =
                                    ((x - AUDIO_SLIDER_NR.x) / AUDIO_SLIDER_NR.w).clamp(0.0, 1.0);
                                return Some(Target::NrFader(ratio * 10.0));
                            }
                            if AUDIO_SLIDER_VOL.contains(x, y) {
                                let ratio =
                                    ((x - AUDIO_SLIDER_VOL.x) / AUDIO_SLIDER_VOL.w).clamp(0.0, 1.0);
                                return Some(Target::VolFader(ratio * 100.0));
                            }
                        }
                        if WAVE_DOCK_CENTERED.contains(x, y) {
                            return Some(Target::WaveDock);
                        }
                    }

                    if PLAYGROUND.contains(x, y) {
                        return Some(Target::Camera);
                    }
                    None
                }
                _ => None,
            }
        }
    }
}

pub fn list_height(rows: usize) -> f32 {
    rows as f32 * (ROW + GAP)
}

pub fn place(
    anchor: (i32, i32, i32, i32),
    work: (i32, i32, i32, i32),
    size: (i32, i32),
    margin: i32,
) -> (i32, i32) {
    let centre = (anchor.0 + anchor.2) / 2;
    let x = (centre - size.0 / 2).clamp(
        work.0 + margin,
        (work.2 - size.0 - margin).max(work.0 + margin),
    );
    let y = if anchor.1 < work.1 {
        work.1 + margin
    } else {
        work.3 - size.1 - margin
    };
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_opens_above_the_tray_inside_the_screen() {
        let work = (0, 0, 1920, 1040);
        assert_eq!(
            place((1800, 1050, 1824, 1074), work, (312, 374), 12),
            (1596, 654)
        );
        assert_eq!(
            place((100, 1050, 124, 1074), work, (312, 374), 12),
            (12, 654)
        );
        let top = (0, 40, 1920, 1080);
        assert_eq!(place((1800, 8, 1824, 32), top, (312, 374), 12).1, 52);
    }

    #[test]
    fn the_grid_fills_312_by_374_with_1_px_gaps() {
        assert_eq!(bottom(4).right(), WIDTH);
        assert_eq!(bottom(0).y, CAMERA.bottom() + 1.0);
        assert_eq!(bottom(0).bottom(), HEIGHT);
        for i in 0..4 {
            assert_eq!(bottom(i + 1).x, bottom(i).right() + 1.0);
        }
        assert_eq!(DENY.right(), WIDTH);
        assert_eq!(DENY.x, ALLOW.right() + 1.0);
    }

    #[test]
    fn clicks_find_their_tile() {
        let main = Screen::main_screen(false);
        assert_eq!(hit(main, 10.0, 10.0, 0.0, 0), Some(Target::Camera));
        assert_eq!(hit(main, 10.0, 340.0, 0.0, 0), Some(Target::Wifi));
        assert_eq!(hit(main, 70.0, 340.0, 0.0, 0), Some(Target::Camera));
        assert_eq!(hit(main, 150.0, 340.0, 0.0, 0), Some(Target::Mic));
        assert_eq!(hit(main, 200.0, 340.0, 0.0, 0), Some(Target::Speaker));
        assert_eq!(hit(main, 280.0, 340.0, 0.0, 0), Some(Target::Help));
        assert_eq!(hit(main, 150.0, 312.5, 0.0, 0), None, "the hairline");

        let r = repair_button();
        assert_eq!(
            hit(Screen::main_screen(true), r.x + 1.0, r.y + 1.0, 0.0, 0),
            Some(Target::Repair)
        );
        assert_eq!(hit(Screen::Gate, 100.0, 340.0, 0.0, 0), Some(Target::Allow));
        assert_eq!(hit(Screen::Gate, 100.0, 100.0, 0.0, 0), None);
    }

    #[test]
    fn settings_rows_scroll_and_have_buttons_on_the_right() {
        assert_eq!(
            hit(Screen::Settings, 10.0, 10.0, 0.0, 5),
            Some(Target::Back)
        );
        assert_eq!(
            hit(Screen::Settings, 10.0, LIST.y + 1.0, 0.0, 5),
            Some(Target::Row(0, 0))
        );
        assert_eq!(
            hit(Screen::Settings, 10.0, LIST.y + 1.0, ROW + 1.0, 5),
            Some(Target::Row(1, 0))
        );
        assert_eq!(
            hit(Screen::Settings, WIDTH - 10.0, LIST.y + 1.0, 0.0, 5),
            Some(Target::Row(0, 2))
        );
        assert_eq!(
            hit(Screen::Settings, WIDTH - 100.0, LIST.y + 1.0, 0.0, 5),
            Some(Target::Row(0, 1))
        );
        assert_eq!(
            hit(Screen::Settings, 10.0, LIST.y + 1.0, 10.0 * ROW, 5),
            None
        );
    }

    #[test]
    fn drawer_and_cam_hud_hits() {
        assert_eq!(
            hit(
                Screen::DrawerWifi,
                CONN_COPY_IP_BTN.x + 2.0,
                CONN_COPY_IP_BTN.y + 2.0,
                0.0,
                0
            ),
            Some(Target::CopyIp)
        );
        assert_eq!(
            hit(Screen::DrawerHelp(HelpScreen::Bento), 40.0, 60.0, 0.0, 0),
            Some(Target::HelpHowTo)
        );
        assert_eq!(
            hit(
                Screen::DrawerHelp(HelpScreen::HowTo),
                HELP_BACK_BTN.x + 2.0,
                HELP_BACK_BTN.y + 2.0,
                0.0,
                0
            ),
            Some(Target::HelpBack)
        );

        let cam_hud_screen = Screen::Main {
            repair: false,
            cam_on: true,
            mic_on: true,
            cam_menu: false,
            quick_audio: false,
        };
        assert_eq!(
            hit(
                cam_hud_screen,
                CAM_MENU_BTN.x + 2.0,
                CAM_MENU_BTN.y + 2.0,
                0.0,
                0
            ),
            Some(Target::CamMenu)
        );
        assert_eq!(
            hit(
                cam_hud_screen,
                CAM_MIRROR_BTN.x + 2.0,
                CAM_MIRROR_BTN.y + 2.0,
                0.0,
                0
            ),
            Some(Target::Mirror)
        );
        assert_eq!(
            hit(
                cam_hud_screen,
                CAM_FLIP_BTN.x + 2.0,
                CAM_FLIP_BTN.y + 2.0,
                0.0,
                0
            ),
            Some(Target::Flip)
        );
        assert_eq!(
            hit(
                cam_hud_screen,
                WAVE_DOCK_DOCKED.x + 2.0,
                WAVE_DOCK_DOCKED.y + 2.0,
                0.0,
                0
            ),
            Some(Target::WaveDock)
        );
    }
}
