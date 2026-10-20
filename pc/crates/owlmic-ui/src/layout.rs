//! The panel's compact Bento 6-Square layout (312 x 374 px) in physical pixels at 100% scale,
//! conforming to landing/proto-templates and android design language.
//!
//! - Top: 1 Big Playground tile (310 x 310 px) with 12 px rounded corners.
//! - Bottom: 5 Square Bento Buttons in a single line (61 x 61 px) with 12 px rounded corners.
//! - Hairline seams: 1 px gap (#7E7E7E) throughout.

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

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn inset(&self, by: f32) -> Rect {
        Rect::new(
            self.x + by,
            self.y + by,
            (self.w - 2.0 * by).max(0.0),
            (self.h - 2.0 * by).max(0.0),
        )
    }
}

pub const WIDTH: f32 = p::WIDTH_PX;
pub const HEIGHT: f32 = p::HEIGHT_PX;
pub const GAP: f32 = 1.0;
pub const PAD: f32 = 1.0;

/// The top 1:1 square playground box (310 x 310 px).
pub const PLAYGROUND: Rect = Rect::new(PAD, PAD, 310.0, 310.0);

/// The bottom bento row (310 x 61 px).
pub const BOTTOM_BAR: Rect = Rect::new(PAD, 312.0, 310.0, 61.0);

/// The 5 square buttons across the bottom row.
pub const BTN_HELP: Rect = Rect::new(1.0, 312.0, 61.0, 61.0);
pub const BTN_CAM: Rect = Rect::new(63.0, 312.0, 61.0, 61.0);
pub const BTN_MIC: Rect = Rect::new(125.0, 312.0, 61.0, 61.0);
pub const BTN_SPK: Rect = Rect::new(187.0, 312.0, 61.0, 61.0);
pub const BTN_WIFI: Rect = Rect::new(249.0, 312.0, 62.0, 61.0);

pub fn btn_bottom(i: usize) -> Rect {
    match i {
        0 => BTN_HELP,
        1 => BTN_CAM,
        2 => BTN_MIC,
        3 => BTN_SPK,
        _ => BTN_WIFI,
    }
}

// Floating Camera Controls over Playground (Top-Right)
pub const BTN_WATCH: Rect = Rect::new(197.0, 11.0, 32.0, 32.0);
pub const BTN_MIRROR: Rect = Rect::new(233.0, 11.0, 32.0, 32.0);
pub const BTN_FLIP: Rect = Rect::new(269.0, 11.0, 32.0, 32.0);

// Floating Mic Settings Button over Playground (Bottom-Right)
pub const BTN_MIC_SETTINGS: Rect = Rect::new(269.0, 269.0, 32.0, 32.0);

// Waveform Docks
pub const WAVEFORM_CENTER: Rect = Rect::new(46.0, 105.0, 220.0, 70.0);
pub const WAVEFORM_DOCKED: Rect = Rect::new(81.0, 269.0, 150.0, 32.0);

// Repair Banner over Playground
pub const REPAIR_BANNER: Rect = Rect::new(9.0, 9.0, 294.0, 40.0);
pub const REPAIR_BTN: Rect = Rect::new(229.0, 16.0, 64.0, 26.0);

// Drawer Navigation & Header (Shared across all drawers)
pub const DRAWER_HEADER: Rect = Rect::new(1.0, 1.0, 310.0, 42.0);
pub const DRAWER_BACK: Rect = Rect::new(11.0, 8.0, 28.0, 28.0);
pub const DRAWER_BODY: Rect = Rect::new(1.0, 43.0, 310.0, 268.0);

// Connection Drawer (Footer with 5 link buttons)
pub const CONN_FOOTER: Rect = Rect::new(1.0, 267.0, 310.0, 44.0);
pub fn conn_type_btn(i: usize) -> Rect {
    let x = 9.0 + (i.min(4) as f32) * 60.0;
    Rect::new(x, 273.0, 54.0, 32.0)
}
pub const PHONE_ROW_H: f32 = 48.0;
pub const PHONE_BTN_W: f32 = 54.0;

// Mic Drawer (Sliders & Switches)
pub const SLIDER_NOISE_TRACK: Rect = Rect::new(15.0, 74.0, 282.0, 24.0);
pub const SLIDER_VOLUME_TRACK: Rect = Rect::new(15.0, 132.0, 282.0, 24.0);
pub const TOGGLE_ISOLATION_ROW: Rect = Rect::new(1.0, 175.0, 310.0, 48.0);
pub const TOGGLE_ISOLATION_SWITCH: Rect = Rect::new(255.0, 187.0, 42.0, 24.0);
pub const TOGGLE_AUTOLEVEL_ROW: Rect = Rect::new(1.0, 227.0, 310.0, 48.0);
pub const TOGGLE_AUTOLEVEL_SWITCH: Rect = Rect::new(255.0, 239.0, 42.0, 24.0);

// Help Drawer Rows
pub const HELP_STEP_1: Rect = Rect::new(1.0, 43.0, 310.0, 48.0);
pub const HELP_STEP_2: Rect = Rect::new(1.0, 91.0, 310.0, 48.0);
pub const HELP_STEP_3: Rect = Rect::new(1.0, 139.0, 310.0, 48.0);
pub const HELP_PRIVACY: Rect = Rect::new(1.0, 187.0, 310.0, 40.0);
pub const HELP_DONATE: Rect = Rect::new(1.0, 227.0, 310.0, 40.0);
pub const HELP_VERSION: Rect = Rect::new(1.0, 267.0, 310.0, 40.0);

// Approval Gate Screen
pub const GATE_BODY: Rect = Rect::new(1.0, 1.0, 310.0, 316.0);
pub const GATE_DENY: Rect = Rect::new(1.0, 318.0, 154.0, 55.0);
pub const GATE_ALLOW: Rect = Rect::new(156.0, 318.0, 155.0, 55.0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drawer {
    Connection,
    Help,
    Mic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Main {
        repair: bool,
        drawer: Option<Drawer>,
        camera_on: bool,
        mic_on: bool,
    },
    Gate,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target {
    // Bottom Bento Buttons
    Help,
    Camera,
    Mic,
    Speaker,
    Wifi,

    // Floating Camera Controls
    Watch,
    Mirror,
    Flip,

    // Floating Mic Controls
    MicSettings,
    Waveform,

    // Repair Banner
    Repair,

    // Drawer Header
    DrawerBack,

    // Connection Drawer
    PhoneRow(usize, u8),
    ConnType(u8),

    // Mic Drawer
    NoiseSlider(f32),
    VolumeSlider(f32),
    ToggleIsolation,
    ToggleAutoLevel,

    // Help Drawer
    HelpStepDownload,
    HelpStepConnect,
    HelpStepStream,
    HelpPrivacy,
    HelpDonate,

    // Approval Gate
    Allow,
    Deny,
}

/// What sits under (x, y) in panel coordinates at 100% scale.
pub fn hit(screen: Screen, x: f32, y: f32, phone_count: usize) -> Option<Target> {
    match screen {
        Screen::Gate => {
            if GATE_ALLOW.contains(x, y) {
                Some(Target::Allow)
            } else if GATE_DENY.contains(x, y) {
                Some(Target::Deny)
            } else {
                None
            }
        }
        Screen::Main {
            repair,
            drawer,
            camera_on,
            mic_on,
        } => {
            // 1. Bottom Bento Buttons are always active and interactive
            if BTN_HELP.contains(x, y) {
                return Some(Target::Help);
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
            if BTN_WIFI.contains(x, y) {
                return Some(Target::Wifi);
            }

            // 2. If a drawer is open inside Playground
            if let Some(d) = drawer {
                if DRAWER_BACK.contains(x, y) || DRAWER_HEADER.contains(x, y) {
                    return Some(Target::DrawerBack);
                }

                match d {
                    Drawer::Connection => {
                        // Check footer connection types (USB adb, Tethering, Wi-Fi, BT, Direct IP)
                        for i in 0..5 {
                            if conn_type_btn(i).contains(x, y) {
                                return Some(Target::ConnType(i as u8));
                            }
                        }
                        // Check phone list rows
                        let list_y = DRAWER_BODY.y;
                        if y >= list_y && y < CONN_FOOTER.y {
                            let idx = ((y - list_y) / PHONE_ROW_H) as usize;
                            if idx < phone_count.max(1) {
                                let from_right = PLAYGROUND.right() - x;
                                let btn = if from_right < PHONE_BTN_W {
                                    2 // Block/Unblock
                                } else if from_right < 2.0 * PHONE_BTN_W {
                                    1 // Remove
                                } else {
                                    0 // Row itself
                                };
                                return Some(Target::PhoneRow(idx, btn));
                            }
                        }
                    }
                    Drawer::Help => {
                        if HELP_STEP_1.contains(x, y) {
                            return Some(Target::HelpStepDownload);
                        }
                        if HELP_STEP_2.contains(x, y) {
                            return Some(Target::HelpStepConnect);
                        }
                        if HELP_STEP_3.contains(x, y) {
                            return Some(Target::HelpStepStream);
                        }
                        if HELP_PRIVACY.contains(x, y) {
                            return Some(Target::HelpPrivacy);
                        }
                        if HELP_DONATE.contains(x, y) {
                            return Some(Target::HelpDonate);
                        }
                    }
                    Drawer::Mic => {
                        if SLIDER_NOISE_TRACK.inset(-6.0).contains(x, y) {
                            let frac =
                                ((x - SLIDER_NOISE_TRACK.x) / SLIDER_NOISE_TRACK.w).clamp(0.0, 1.0);
                            return Some(Target::NoiseSlider(frac));
                        }
                        if SLIDER_VOLUME_TRACK.inset(-6.0).contains(x, y) {
                            let frac = ((x - SLIDER_VOLUME_TRACK.x) / SLIDER_VOLUME_TRACK.w)
                                .clamp(0.0, 1.0);
                            return Some(Target::VolumeSlider(frac));
                        }
                        if TOGGLE_ISOLATION_ROW.contains(x, y) {
                            return Some(Target::ToggleIsolation);
                        }
                        if TOGGLE_AUTOLEVEL_ROW.contains(x, y) {
                            return Some(Target::ToggleAutoLevel);
                        }
                    }
                }
                return None;
            }

            // 3. Normal Playground (no drawer open)
            if repair && REPAIR_BTN.inset(-4.0).contains(x, y) {
                return Some(Target::Repair);
            }

            if camera_on {
                if BTN_WATCH.contains(x, y) {
                    return Some(Target::Watch);
                }
                if BTN_MIRROR.contains(x, y) {
                    return Some(Target::Mirror);
                }
                if BTN_FLIP.contains(x, y) {
                    return Some(Target::Flip);
                }
                if mic_on && WAVEFORM_DOCKED.contains(x, y) {
                    return Some(Target::Waveform);
                }
            } else if mic_on && WAVEFORM_CENTER.contains(x, y) {
                return Some(Target::Waveform);
            }

            if mic_on && BTN_MIC_SETTINGS.contains(x, y) {
                return Some(Target::MicSettings);
            }

            None
        }
    }
}

/// Where the panel opens, in screen pixels: centred over the tray icon `anchor`, kept inside the
/// work area `work`, just above the taskbar (or below it when the taskbar is at the top).
/// Rectangles are (left, top, right, bottom).
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
    let top_half = anchor.1 < work.1 + (work.3 - work.1) / 2;
    let y = if top_half {
        anchor.3 + margin
    } else {
        anchor.1 - size.1 - margin
    };
    let y_clamped = y.clamp(
        work.1 + margin,
        (work.3 - size.1 - margin).max(work.1 + margin),
    );
    (x, y_clamped)
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
        assert_eq!(PLAYGROUND.x, 1.0);
        assert_eq!(PLAYGROUND.y, 1.0);
        assert_eq!(PLAYGROUND.w, 310.0);
        assert_eq!(PLAYGROUND.h, 310.0);
        assert_eq!(BOTTOM_BAR.y, PLAYGROUND.bottom() + 1.0);
        assert_eq!(BOTTOM_BAR.bottom(), HEIGHT - 1.0);
        assert_eq!(BTN_HELP.x, 1.0);
        assert_eq!(BTN_CAM.x, BTN_HELP.right() + 1.0);
        assert_eq!(BTN_MIC.x, BTN_CAM.right() + 1.0);
        assert_eq!(BTN_SPK.x, BTN_MIC.right() + 1.0);
        assert_eq!(BTN_WIFI.x, BTN_SPK.right() + 1.0);
        assert_eq!(BTN_WIFI.right(), WIDTH - 1.0);
        assert_eq!(GATE_DENY.right() + 1.0, GATE_ALLOW.x);
    }

    #[test]
    fn clicks_find_their_tile() {
        let main = Screen::Main {
            repair: false,
            drawer: None,
            camera_on: false,
            mic_on: false,
        };
        assert_eq!(hit(main, 30.0, 330.0, 0), Some(Target::Help));
        assert_eq!(hit(main, 90.0, 330.0, 0), Some(Target::Camera));
        assert_eq!(hit(main, 150.0, 330.0, 0), Some(Target::Mic));
        assert_eq!(hit(main, 210.0, 330.0, 0), Some(Target::Speaker));
        assert_eq!(hit(main, 280.0, 330.0, 0), Some(Target::Wifi));

        let with_repair = Screen::Main {
            repair: true,
            drawer: None,
            camera_on: false,
            mic_on: false,
        };
        assert_eq!(hit(with_repair, 240.0, 24.0, 0), Some(Target::Repair));

        let with_cam = Screen::Main {
            repair: false,
            drawer: None,
            camera_on: true,
            mic_on: true,
        };
        assert_eq!(hit(with_cam, 280.0, 20.0, 0), Some(Target::Flip));
        assert_eq!(hit(with_cam, 245.0, 20.0, 0), Some(Target::Mirror));
        assert_eq!(hit(with_cam, 210.0, 20.0, 0), Some(Target::Watch));
        assert_eq!(hit(with_cam, 280.0, 280.0, 0), Some(Target::MicSettings));
        assert_eq!(hit(with_cam, 150.0, 280.0, 0), Some(Target::Waveform));

        let with_mic_drawer = Screen::Main {
            repair: false,
            drawer: Some(Drawer::Mic),
            camera_on: false,
            mic_on: true,
        };
        assert_eq!(
            hit(with_mic_drawer, 20.0, 20.0, 0),
            Some(Target::DrawerBack)
        );
        assert!(matches!(
            hit(with_mic_drawer, 100.0, 80.0, 0),
            Some(Target::NoiseSlider(_))
        ));
        assert!(matches!(
            hit(with_mic_drawer, 100.0, 140.0, 0),
            Some(Target::VolumeSlider(_))
        ));
        assert_eq!(
            hit(with_mic_drawer, 260.0, 190.0, 0),
            Some(Target::ToggleIsolation)
        );
        assert_eq!(
            hit(with_mic_drawer, 260.0, 245.0, 0),
            Some(Target::ToggleAutoLevel)
        );

        assert_eq!(hit(Screen::Gate, 100.0, 340.0, 0), Some(Target::Deny));
        assert_eq!(hit(Screen::Gate, 200.0, 340.0, 0), Some(Target::Allow));
    }
}
