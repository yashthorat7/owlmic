//! The PC's look: design tokens, texts and icons generated from design/ and icons/, the panel's
//! layout and view model, and on Windows the Direct2D panel and the tray icon.

include!(concat!(env!("OUT_DIR"), "/design.rs"));

pub mod glyph;
pub mod layout;
pub mod owl;
pub mod preview;
pub mod view;
#[cfg(windows)]
pub mod win;

/// A text style from design/tokens.json, in pixels at 100% scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    pub size_px: f32,
    pub line_px: f32,
    pub weight: u64,
}

/// "Mic", "Mic and Camera", "Mic, Camera and Speaker".
pub fn list(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [rest @ .., last] => format!("{} {} {last}", rest.join(", "), messages::UI_AND),
    }
}

/// Fills a message's `{name}` placeholders: `fill(messages::STATUS_WAITING, &[("pc", "DESKTOP-A")])`.
pub fn fill(template: &str, values: &[(&str, &str)]) -> String {
    let mut text = template.to_owned();
    for (name, value) in values {
        text = text.replace(&format!("{{{name}}}"), value);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_come_from_the_design_file() {
        assert_eq!(color::BG, 0x000000);
        assert_eq!(color::GREEN, 0x30D158);
        assert_eq!(color::ON_CONTENT, 0x000000);
        assert_eq!(text_style::TITLE.size_px, 14.0);
        assert_eq!(panel::WIDTH_PX, 560.0);
    }

    #[test]
    fn messages_and_names_come_from_the_design_file() {
        assert_eq!(names::MIC_DEVICE, "Owlmic Mic");
        assert_eq!(messages::STATUS_SEARCHING, "Looking for your PC");
    }

    #[test]
    fn lists_read_naturally() {
        assert_eq!(list(&[]), "");
        assert_eq!(list(&["Mic"]), "Mic");
        assert_eq!(list(&["Mic", "Camera"]), "Mic and Camera");
        assert_eq!(
            list(&["Mic", "Camera", "Speaker"]),
            "Mic, Camera and Speaker"
        );
    }

    #[test]
    fn fill_replaces_every_placeholder() {
        let text = fill(
            messages::STATUS_BUSY,
            &[("pc", "DESKTOP-A"), ("phone", "Pixel 8")],
        );
        assert_eq!(text, "DESKTOP-A is in use by Pixel 8");
    }
}
