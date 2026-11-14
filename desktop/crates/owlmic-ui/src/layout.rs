//! The panel's grid (SYSTEM_DESIGN sections 33 and 35) in pixels at 100% scale, and what a
//! click lands on. Every gap is the 1 px hairline the background shows through.

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
            self.w - 2.0 * by,
            self.h - 2.0 * by,
        )
    }
}

const GAP: f32 = 1.0;
pub const WIDTH: f32 = p::WIDTH_PX;
pub const HEIGHT: f32 = p::HEIGHT_PX;

pub const CAMERA: Rect = Rect::new(0.0, 0.0, p::CAMERA_WIDTH_PX, p::CAMERA_HEIGHT_PX);
pub const MIC: Rect = Rect::new(
    p::CAMERA_WIDTH_PX + GAP,
    0.0,
    p::FEATURE_TILE_PX,
    p::FEATURE_ROW_PX,
);
pub const SPEAKER: Rect = Rect::new(
    MIC.x + p::FEATURE_TILE_PX + GAP,
    0.0,
    p::FEATURE_TILE_PX,
    p::FEATURE_ROW_PX,
);
pub const PHONE: Rect = Rect::new(
    MIC.x,
    p::FEATURE_ROW_PX + GAP,
    WIDTH - MIC.x,
    p::PHONE_TILE_HEIGHT_PX,
);
const BOTTOM_Y: f32 = HEIGHT - p::BOTTOM_ROW_PX;

/// Fill or Fit, Mirror, Lens, Settings, the connection indicator.
pub fn bottom(i: usize) -> Rect {
    const WIDTHS: [f32; 5] = [111.0, 111.0, 111.0, 111.0, 112.0];
    let x: f32 = WIDTHS[..i].iter().map(|w| w + GAP).sum();
    Rect::new(x, BOTTOM_Y, WIDTHS[i], p::BOTTOM_ROW_PX)
}

/// The Repair tile over the top of the camera area.
pub const REPAIR: Rect = Rect::new(0.0, 0.0, CAMERA.w, 64.0);
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

/// The settings view: a back row, then a scrolling list.
pub const BACK: Rect = Rect::new(0.0, 0.0, WIDTH, 40.0);
pub const LIST: Rect = Rect::new(0.0, BACK.h + GAP, WIDTH, HEIGHT - BACK.h - GAP);
pub const ROW: f32 = 32.0;
/// Phone rows end in two buttons: Remove, then Block or Unblock.
pub const ROW_BUTTON: f32 = 88.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Camera,
    Mic,
    Speaker,
    Phone,
    Framing,
    Mirror,
    Lens,
    Settings,
    Indicator,
    Repair,
    Allow,
    Deny,
    Back,
    /// A settings row, and which of its buttons (0 for the row itself).
    Row(usize, u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Main { repair: bool },
    Gate,
    Settings,
}

/// What sits under (x, y) in panel pixels at 100%, with the settings list scrolled by `scroll`.
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
        Screen::Main { repair } => {
            if repair && repair_button().contains(x, y) {
                return Some(Target::Repair);
            }
            let tiles = [
                (CAMERA, Target::Camera),
                (MIC, Target::Mic),
                (SPEAKER, Target::Speaker),
                (PHONE, Target::Phone),
                (bottom(0), Target::Framing),
                (bottom(1), Target::Mirror),
                (bottom(2), Target::Lens),
                (bottom(3), Target::Settings),
                (bottom(4), Target::Indicator),
            ];
            tiles
                .into_iter()
                .find(|(r, _)| r.contains(x, y))
                .map(|(_, t)| t)
        }
    }
}

/// The settings list's full height, for scrolling.
pub fn list_height(rows: usize) -> f32 {
    rows as f32 * (ROW + GAP)
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
            place((1800, 1050, 1824, 1074), work, (560, 255), 12),
            (1348, 773)
        );
        assert_eq!(
            place((100, 1050, 124, 1074), work, (560, 255), 12),
            (12, 773)
        );
        let top = (0, 40, 1920, 1080);
        assert_eq!(place((1800, 8, 1824, 32), top, (560, 255), 12).1, 52);
    }

    #[test]
    fn the_grid_fills_560_by_255_with_1_px_gaps() {
        assert_eq!(SPEAKER.right(), WIDTH);
        assert_eq!(PHONE.bottom(), CAMERA.bottom());
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
        let main = Screen::Main { repair: false };
        assert_eq!(hit(main, 10.0, 10.0, 0.0, 0), Some(Target::Camera));
        assert_eq!(hit(main, 400.0, 50.0, 0.0, 0), Some(Target::Mic));
        assert_eq!(hit(main, 500.0, 50.0, 0.0, 0), Some(Target::Speaker));
        assert_eq!(hit(main, 352.5, 50.0, 0.0, 0), None, "the hairline");
        assert_eq!(hit(main, 555.0, 250.0, 0.0, 0), Some(Target::Indicator));
        let r = repair_button();
        assert_eq!(
            hit(Screen::Main { repair: true }, r.x + 1.0, r.y + 1.0, 0.0, 0),
            Some(Target::Repair)
        );
        assert_eq!(hit(Screen::Gate, 100.0, 220.0, 0.0, 0), Some(Target::Allow));
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
}
