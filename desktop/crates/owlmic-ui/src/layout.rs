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

// Camera HUD Controls
pub const CAM_MENU_BTN: Rect = Rect::new(10.0, 10.0, 44.0, 25.0);
pub const CAM_EXPAND_MENU: Rect = Rect::new(10.0, 10.0, 156.0, 103.0);
pub const CAM_MIRROR_BTN: Rect = Rect::new(246.0, 10.0, 25.0, 25.0);
pub const CAM_FLIP_BTN: Rect = Rect::new(277.0, 10.0, 25.0, 25.0);

pub fn cam_option_rect(row: usize, col: usize) -> Rect {
    let w = CAM_EXPAND_MENU.w / 3.0;
    let h = 25.0;
    let y = CAM_EXPAND_MENU.y + 1.0 + row as f32 * h;
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
    PLAYGROUND.bottom() - 12.0 - 36.0,
    204.0,
    36.0,
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
pub const CONN_COPY_IP_BTN: Rect = Rect::new(WIDTH - 14.0 - 28.0, 13.0, 28.0, 28.0);
pub const CONN_PRIMARY_ROW: Rect = Rect::new(0.0, 54.0, WIDTH, 60.0);
pub const CONN_DISCONNECT_BTN: Rect =
    Rect::new(WIDTH - 14.0 - 40.0 - 10.0 - 28.0, 70.0, 28.0, 28.0);
pub const CONN_BLOCK_BTN: Rect = Rect::new(WIDTH - 14.0 - 40.0, 70.0, 40.0, 28.0);
pub const CONN_PAIR_BTN: Rect = Rect::new(WIDTH - 14.0 - 28.0 - 10.0 - 28.0, 70.0, 28.0, 28.0);
pub const CONN_DECLINE_BTN: Rect = Rect::new(WIDTH - 14.0 - 28.0, 70.0, 28.0, 28.0);
pub const CONN_UNBLOCK_BTN: Rect = Rect::new(WIDTH - 14.0 - 55.0, 70.0, 55.0, 28.0);
pub const CONN_FORGET_BTN: Rect = Rect::new(WIDTH - 14.0 - 45.0, 70.0, 45.0, 28.0);
pub const CONN_PAIRED_ROW: Rect = Rect::new(0.0, 114.0, WIDTH, 60.0);
pub const CONN_REMOVE_PAIRED_BTN: Rect = Rect::new(WIDTH - 14.0 - 50.0, 130.0, 50.0, 28.0);
pub const CONN_FOOTER: Rect = Rect::new(0.0, PLAYGROUND.bottom() - 52.0, WIDTH, 52.0);
pub const CONN_REFRESH_BTN: Rect = Rect::new(8.0, CONN_FOOTER.y + 8.0, 53.0, 36.0);

pub fn conn_transport_btn(i: usize) -> Rect {
    let x = 67.0 + i as f32 * 59.0;
    Rect::new(x, CONN_FOOTER.y + 8.0, 53.0, 36.0)
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

pub const TILE_RADIUS: f32 = 12.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CropMode {
    None,
    Pillar,
    Letter,
}

pub fn cam_crop_rect(ratio: &str) -> (Rect, CropMode) {
    let target_ar = match ratio {
        "16:9" => 16.0 / 9.0,
        "4:3" => 4.0 / 3.0,
        "1:1" => 1.0,
        "3:4" => 3.0 / 4.0,
        "9:16" => 9.0 / 16.0,
        _ => 0.0,
    };
    let lw = PLAYGROUND.w;
    let lh = PLAYGROUND.h;
    if target_ar <= 0.0 || (lw / lh - target_ar).abs() < 0.01 {
        return (PLAYGROUND, CropMode::None);
    }
    if lw / lh > target_ar {
        let w = (lh * target_ar).round();
        let x = PLAYGROUND.x + ((lw - w) / 2.0).round();
        (Rect::new(x, PLAYGROUND.y, w, lh), CropMode::Pillar)
    } else {
        let h = (lw / target_ar).round();
        let y = PLAYGROUND.y + ((lh - h) / 2.0).round();
        (Rect::new(PLAYGROUND.x, y, lw, h), CropMode::Letter)
    }
}

pub const WAVE_DOCK_LIFTED: Rect = Rect::new((WIDTH - 248.0) / 2.0, 38.0, 248.0, 68.0);
pub const WAVE_DOCK_EXPANDED: Rect = Rect::new(13.0, 101.0, 286.0, 198.0);
pub const WAVE_DOCK_EXPANDED_SETTINGS: Rect = Rect::new(13.0, 101.0, 286.0, 164.0);
pub const WAVE_DOCK_EXPANDED_HEADER: Rect = Rect::new(13.0, 265.0, 286.0, 34.0);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioControls {
    pub iso_label: Rect,
    pub iso_switch: Rect,
    pub norm_label: Rect,
    pub norm_switch: Rect,
    pub nr_label: Rect,
    pub nr_value: Rect,
    pub nr_fader: Rect,
    pub vol_label: Rect,
    pub vol_value: Rect,
    pub vol_fader: Rect,
}

pub fn audio_controls(panel: Rect) -> AudioControls {
    AudioControls {
        iso_label: Rect::new(panel.x + 14.0, panel.y + 12.0, panel.w - 28.0 - 40.0, 18.0),
        iso_switch: Rect::new(panel.right() - 14.0 - 34.0, panel.y + 12.0, 34.0, 18.0),
        norm_label: Rect::new(panel.x + 14.0, panel.y + 38.0, panel.w - 28.0 - 40.0, 18.0),
        norm_switch: Rect::new(panel.right() - 14.0 - 34.0, panel.y + 38.0, 34.0, 18.0),
        nr_label: Rect::new(panel.x + 14.0, panel.y + 64.0, panel.w - 28.0 - 40.0, 16.0),
        nr_value: Rect::new(panel.right() - 14.0 - 40.0, panel.y + 64.0, 40.0, 16.0),
        nr_fader: Rect::new(panel.x + 14.0, panel.y + 82.0, panel.w - 28.0, 20.0),
        vol_label: Rect::new(panel.x + 14.0, panel.y + 108.0, panel.w - 28.0 - 40.0, 16.0),
        vol_value: Rect::new(panel.right() - 14.0 - 40.0, panel.y + 108.0, 40.0, 16.0),
        vol_fader: Rect::new(panel.x + 14.0, panel.y + 126.0, panel.w - 28.0, 20.0),
    }
}

pub fn active_audio_panel(cam_active: bool) -> Rect {
    if cam_active {
        WAVE_DOCK_EXPANDED_SETTINGS
    } else {
        AUDIO_QUICK_PANEL
    }
}

pub const CONN_HEADER: Rect = Rect::new(0.0, 0.0, WIDTH, 54.0);
pub const CONN_FOOTER_DIVIDER_X: f32 = 64.0;
pub const HELP_HEADER: Rect = Rect::new(0.0, 0.0, WIDTH, 44.0);

pub fn help_credit_row(idx: usize) -> Rect {
    let row_h = 38.0;
    let y = HELP_HEADER.bottom() + 6.0 + idx as f32 * row_h;
    Rect::new(0.0, y, WIDTH, row_h)
}

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
    CamCanvas,
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
    Forget,
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
    HelpReleases,
    HelpCredit(usize),
    HelpBack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WifiMode {
    #[default]
    Offline,
    Pairing,
    Connected,
    Blocked,
    Forget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Main {
        cam_on: bool,
        mic_on: bool,
        cam_menu: bool,
        quick_audio: bool,
    },
    DrawerWifi(WifiMode),
    DrawerHelp(HelpScreen),
}

impl Screen {
    pub const fn main_screen() -> Self {
        Screen::Main {
            cam_on: false,
            mic_on: false,
            cam_menu: false,
            quick_audio: false,
        }
    }
}

/// What sits under (x, y) in panel pixels at 100%.
pub fn hit(screen: Screen, x: f32, y: f32) -> Option<Target> {
    // 1. Bottom 5 Square Bento Dock Buttons always take precedence
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
        Screen::DrawerWifi(mode) => {
            if CONN_COPY_IP_BTN.contains(x, y) {
                return Some(Target::CopyIp);
            }
            match mode {
                WifiMode::Pairing => {
                    if CONN_PAIR_BTN.contains(x, y) {
                        return Some(Target::Pair);
                    }
                    if CONN_DECLINE_BTN.contains(x, y) {
                        return Some(Target::Decline);
                    }
                }
                WifiMode::Connected => {
                    if CONN_DISCONNECT_BTN.contains(x, y) {
                        return Some(Target::Disconnect);
                    }
                    if CONN_BLOCK_BTN.contains(x, y) {
                        return Some(Target::Block);
                    }
                }
                WifiMode::Blocked => {
                    if CONN_UNBLOCK_BTN.contains(x, y) {
                        return Some(Target::Unblock);
                    }
                }
                WifiMode::Forget => {
                    if CONN_FORGET_BTN.contains(x, y) {
                        return Some(Target::Forget);
                    }
                }
                WifiMode::Offline => {}
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
            None
        }
        Screen::DrawerHelp(help_screen) => {
            match help_screen {
                HelpScreen::Bento => {
                    for i in 0..6 {
                        if help_bento_tile(i).contains(x, y) {
                            return Some(match i {
                                0 => Target::HelpHowTo,
                                1 => Target::HelpPrivacy,
                                2 => Target::HelpRate,
                                3 => Target::HelpGitHub,
                                4 => Target::HelpDonate,
                                _ => Target::HelpCredits,
                            });
                        }
                    }
                }
                HelpScreen::Credits => {
                    if HELP_BACK_BTN.contains(x, y) {
                        return Some(Target::HelpBack);
                    }
                    for i in 0..5 {
                        if help_credit_row(i).contains(x, y) {
                            return Some(Target::HelpCredit(i));
                        }
                    }
                    if help_credit_row(5).contains(x, y) {
                        return Some(Target::HelpGitHub);
                    }
                }
                HelpScreen::HowTo => {
                    if HELP_BACK_BTN.contains(x, y) {
                        return Some(Target::HelpBack);
                    }
                    let rel_rect = Rect::new(50.0, 56.0, 200.0, 22.0);
                    if rel_rect.contains(x, y) {
                        return Some(Target::HelpReleases);
                    }
                }
                _ => {
                    if HELP_BACK_BTN.contains(x, y) {
                        return Some(Target::HelpBack);
                    }
                }
            }
            None
        }
        Screen::Main {
            cam_on,
            mic_on,
            cam_menu,
            quick_audio,
        } => {
            if cam_on {
                if cam_menu {
                    let res_opts = ["FHD", "HD", "480p"];
                    let fps_opts = ["60", "30", "24"];
                    let row2 = ["Auto", "16:9", "4:3"];
                    let row3 = ["1:1", "3:4", "9:16"];

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
                    for (c, &opt) in row2.iter().enumerate() {
                        if cam_option_rect(2, c).contains(x, y) {
                            return Some(Target::Ratio(opt));
                        }
                    }
                    for (c, &opt) in row3.iter().enumerate() {
                        if cam_option_rect(3, c).contains(x, y) {
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
                if mic_on && quick_audio {
                    let ac = audio_controls(WAVE_DOCK_EXPANDED_SETTINGS);
                    if ac.iso_switch.contains(x, y) || ac.iso_label.contains(x, y) {
                        return Some(Target::ToggleIso);
                    }
                    if ac.norm_switch.contains(x, y) || ac.norm_label.contains(x, y) {
                        return Some(Target::ToggleNorm);
                    }
                    if ac.nr_fader.contains(x, y) {
                        let ratio = ((x - ac.nr_fader.x) / ac.nr_fader.w).clamp(0.0, 1.0);
                        return Some(Target::NrFader(ratio * 10.0));
                    }
                    if ac.vol_fader.contains(x, y) {
                        let ratio = ((x - ac.vol_fader.x) / ac.vol_fader.w).clamp(0.0, 1.0);
                        return Some(Target::VolFader(ratio * 100.0));
                    }
                    if WAVE_DOCK_EXPANDED_HEADER.contains(x, y) {
                        return Some(Target::WaveDock);
                    }
                    if WAVE_DOCK_EXPANDED.contains(x, y) {
                        return None;
                    }
                } else if mic_on && WAVE_DOCK_DOCKED.contains(x, y) {
                    return Some(Target::WaveDock);
                }
                if PLAYGROUND.contains(x, y) {
                    return Some(Target::CamCanvas);
                }
            } else if mic_on {
                if quick_audio {
                    let ac = audio_controls(AUDIO_QUICK_PANEL);
                    if ac.iso_switch.contains(x, y) || ac.iso_label.contains(x, y) {
                        return Some(Target::ToggleIso);
                    }
                    if ac.norm_switch.contains(x, y) || ac.norm_label.contains(x, y) {
                        return Some(Target::ToggleNorm);
                    }
                    if ac.nr_fader.contains(x, y) {
                        let ratio = ((x - ac.nr_fader.x) / ac.nr_fader.w).clamp(0.0, 1.0);
                        return Some(Target::NrFader(ratio * 10.0));
                    }
                    if ac.vol_fader.contains(x, y) {
                        let ratio = ((x - ac.vol_fader.x) / ac.vol_fader.w).clamp(0.0, 1.0);
                        return Some(Target::VolFader(ratio * 100.0));
                    }
                    if AUDIO_QUICK_PANEL.contains(x, y) {
                        return None;
                    }
                }
                if WAVE_DOCK_CENTERED.contains(x, y) || (!quick_audio && PLAYGROUND.contains(x, y))
                {
                    return Some(Target::WaveDock);
                }
            }
            None
        }
    }
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
    let y = if anchor.1 < work.1 || (anchor.3 - work.1) < (work.3 - anchor.1) {
        (anchor.3 + margin).clamp(
            work.1 + margin,
            (work.3 - size.1 - margin).max(work.1 + margin),
        )
    } else {
        (anchor.1 - size.1 - margin).clamp(
            work.1 + margin,
            (work.3 - size.1 - margin).max(work.1 + margin),
        )
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
        // Above Windows 11 tray overflow capsule (icon top at 940 px)
        assert_eq!(
            place((1800, 940, 1824, 964), work, (312, 374), 12),
            (1596, 554)
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
    }

    #[test]
    fn clicks_find_their_tile() {
        let main = Screen::main_screen();
        assert_eq!(hit(main, 10.0, 340.0), Some(Target::Wifi));
        assert_eq!(hit(main, 70.0, 340.0), Some(Target::Camera));
        assert_eq!(hit(main, 150.0, 340.0), Some(Target::Mic));
        assert_eq!(hit(main, 200.0, 340.0), Some(Target::Speaker));
        assert_eq!(hit(main, 280.0, 340.0), Some(Target::Help));
        assert_eq!(hit(main, 150.0, 312.5), None, "the hairline");
    }

    #[test]
    fn drawer_and_cam_hud_hits() {
        assert_eq!(
            hit(
                Screen::DrawerWifi(WifiMode::Offline),
                CONN_COPY_IP_BTN.x + 2.0,
                CONN_COPY_IP_BTN.y + 2.0,
            ),
            Some(Target::CopyIp)
        );
        assert_eq!(
            hit(
                Screen::DrawerWifi(WifiMode::Pairing),
                CONN_PAIR_BTN.x + 2.0,
                CONN_PAIR_BTN.y + 2.0,
            ),
            Some(Target::Pair)
        );
        assert_eq!(
            hit(
                Screen::DrawerWifi(WifiMode::Pairing),
                CONN_DECLINE_BTN.x + 2.0,
                CONN_DECLINE_BTN.y + 2.0,
            ),
            Some(Target::Decline)
        );
        assert_eq!(
            hit(
                Screen::DrawerWifi(WifiMode::Connected),
                CONN_DISCONNECT_BTN.x + 2.0,
                CONN_DISCONNECT_BTN.y + 2.0,
            ),
            Some(Target::Disconnect)
        );
        assert_eq!(
            hit(
                Screen::DrawerWifi(WifiMode::Connected),
                CONN_BLOCK_BTN.x + 2.0,
                CONN_BLOCK_BTN.y + 2.0,
            ),
            Some(Target::Block)
        );
        assert_eq!(
            hit(
                Screen::DrawerWifi(WifiMode::Blocked),
                CONN_UNBLOCK_BTN.x + 2.0,
                CONN_UNBLOCK_BTN.y + 2.0,
            ),
            Some(Target::Unblock)
        );
        assert_eq!(
            hit(
                Screen::DrawerWifi(WifiMode::Forget),
                CONN_FORGET_BTN.x + 2.0,
                CONN_FORGET_BTN.y + 2.0,
            ),
            Some(Target::Forget)
        );
        assert_eq!(
            hit(Screen::DrawerHelp(HelpScreen::Bento), 40.0, 60.0),
            Some(Target::HelpHowTo)
        );
        assert_eq!(
            hit(
                Screen::DrawerHelp(HelpScreen::HowTo),
                HELP_BACK_BTN.x + 2.0,
                HELP_BACK_BTN.y + 2.0,
            ),
            Some(Target::HelpBack)
        );

        let cam_hud_screen = Screen::Main {
            cam_on: true,
            mic_on: true,
            cam_menu: false,
            quick_audio: false,
        };
        assert_eq!(
            hit(cam_hud_screen, CAM_MENU_BTN.x + 2.0, CAM_MENU_BTN.y + 2.0,),
            Some(Target::CamMenu)
        );
        assert_eq!(
            hit(
                cam_hud_screen,
                CAM_MIRROR_BTN.x + 2.0,
                CAM_MIRROR_BTN.y + 2.0,
            ),
            Some(Target::Mirror)
        );
        assert_eq!(
            hit(cam_hud_screen, CAM_FLIP_BTN.x + 2.0, CAM_FLIP_BTN.y + 2.0,),
            Some(Target::Flip)
        );
        assert_eq!(
            hit(
                cam_hud_screen,
                WAVE_DOCK_DOCKED.x + 2.0,
                WAVE_DOCK_DOCKED.y + 2.0,
            ),
            Some(Target::WaveDock)
        );
    }
}
