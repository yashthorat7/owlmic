//! What the panel and the tray show for a state (SYSTEM_DESIGN sections 33 to 37), and what a
//! click on each tile does. Texts come only from design/copy.json.

use crate::glyph::Icon;
pub use crate::layout::Drawer;
use crate::layout::{Screen, Target};
use crate::{color, fill, icons, messages as m};
use std::collections::BTreeMap;

pub const LINK_USB_DEBUGGING: u8 = 1;
pub const LINK_USB_TETHERING: u8 = 2;
pub const LINK_WIFI: u8 = 3;
pub const LINK_BLUETOOTH: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Feature {
    #[default]
    Off,
    On,
    Paused,
    /// On, but stalled for a while; the recovery ladder is working on it.
    Restarting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Which {
    Mic,
    Camera,
    Speaker,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Connection {
    #[default]
    NotConnected,
    /// A phone is shaking hands over `link`.
    Connecting {
        link: u8,
    },
    Approval {
        phone: String,
        code: String,
        link: u8,
    },
    Connected {
        phone: String,
        link: u8,
        weak: bool,
    },
    Reconnecting {
        phone: String,
        link: u8,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PhoneEntry {
    pub id: String,
    pub name: String,
    pub blocked: bool,
}

/// Which parts need a repair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Repair {
    pub mic: bool,
    pub cam: bool,
    pub net: bool,
}

impl Repair {
    pub fn needed(&self) -> bool {
        self.mic || self.cam || self.net
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PanelState {
    pub connection: Connection,
    pub mic: Feature,
    pub camera: Feature,
    pub speaker: Feature,
    /// Every setting's current value, by id.
    pub settings: BTreeMap<String, String>,
    /// Pairing settings can change only with a phone connected.
    pub paired: bool,
    pub phones: Vec<PhoneEntry>,
    pub addresses: Vec<String>,
    pub repair: Repair,
    pub repairing: bool,
    pub version: String,
}

/// What a click asks the app to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Pause(Which),
    /// Moves a setting to its next value.
    Cycle(&'static str),
    /// Sets a setting directly to a specific value.
    Set(&'static str, String),
    Allow,
    Deny,
    Repair,
    RemovePhone(String),
    BlockPhone(String, bool),
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Link {
    Licences,
    Website,
    Privacy,
    Donate,
}

pub struct Indicator {
    pub color: u32,
    pub icon: Icon,
    pub label: &'static str,
}

pub fn link_icon(link: u8) -> Icon {
    match link {
        LINK_USB_DEBUGGING => icons::USB,
        LINK_USB_TETHERING => icons::CABLE,
        LINK_WIFI => icons::WIFI,
        LINK_BLUETOOTH => icons::BLUETOOTH,
        _ => icons::UNPLUG,
    }
}

/// The link's short name, as the settings call it.
pub fn link_name(link: u8) -> &'static str {
    match link {
        LINK_USB_DEBUGGING => m::SETTING_LINK_USB_DEBUGGING,
        LINK_USB_TETHERING => m::SETTING_LINK_USB_TETHERING,
        LINK_BLUETOOTH => m::SETTING_LINK_BLUETOOTH,
        _ => m::SETTING_LINK_WIFI,
    }
}

/// The connection indicator. Green is always the link's icon.
pub fn indicator(c: &Connection) -> Indicator {
    let yellow = |link, label| Indicator {
        color: color::YELLOW,
        icon: link_icon(link),
        label,
    };
    match c {
        Connection::NotConnected => Indicator {
            color: color::RED,
            icon: icons::UNPLUG,
            label: m::LINK_NOT_CONNECTED,
        },
        Connection::Connecting { link } => yellow(*link, m::LINK_CONNECTING),
        Connection::Approval { link, .. } => yellow(*link, m::LINK_APPROVAL),
        Connection::Reconnecting { link, .. } => yellow(*link, m::STATUS_RECONNECTING),
        Connection::Connected {
            link, weak: true, ..
        } => yellow(*link, m::LINK_WEAK),
        Connection::Connected { link, .. } => Indicator {
            color: color::GREEN,
            icon: link_icon(*link),
            label: match *link {
                LINK_USB_DEBUGGING => m::LINK_USB_DEBUGGING,
                LINK_USB_TETHERING => m::LINK_USB_TETHERING,
                LINK_BLUETOOTH => m::LINK_BLUETOOTH,
                _ => m::LINK_WIFI,
            },
        },
    }
}

impl PanelState {
    pub fn value(&self, id: &str) -> &str {
        self.settings.get(id).map_or("", String::as_str)
    }

    pub fn screen(&self, drawer: Option<Drawer>) -> Screen {
        if matches!(self.connection, Connection::Approval { .. }) {
            Screen::Gate
        } else {
            Screen::Main {
                repair: self.repair.needed(),
                drawer,
                camera_on: self.camera == Feature::On,
                mic_on: self.mic == Feature::On,
            }
        }
    }

    pub fn phone_name(&self) -> Option<&str> {
        match &self.connection {
            Connection::Approval { phone, .. }
            | Connection::Connected { phone, .. }
            | Connection::Reconnecting { phone, .. } => Some(phone),
            _ => None,
        }
    }

    pub fn connected(&self) -> bool {
        matches!(
            self.connection,
            Connection::Connected { .. } | Connection::Reconnecting { .. }
        )
    }

    /// The tray tooltip.
    pub fn tooltip(&self) -> String {
        if self.repair.needed() {
            return m::TRAY_REPAIR.to_owned();
        }
        match &self.connection {
            Connection::Approval { phone, .. } => fill(m::TRAY_APPROVE, &[("phone", phone)]),
            Connection::Connected { phone, link, .. }
            | Connection::Reconnecting { phone, link } => {
                let on: Vec<&str> = [
                    (self.mic, m::FEATURE_MIC),
                    (self.camera, m::FEATURE_CAMERA),
                    (self.speaker, m::FEATURE_SPEAKER),
                ]
                .into_iter()
                .filter(|(f, _)| matches!(f, Feature::On | Feature::Restarting))
                .map(|(_, name)| name)
                .collect();
                if on.is_empty() {
                    fill(
                        m::TRAY_CONNECTED,
                        &[("phone", phone), ("link", link_name(*link))],
                    )
                } else {
                    fill(m::TRAY_IN_USE, &[("features", &crate::list(&on))])
                }
            }
            _ => m::TRAY_NO_PHONE.to_owned(),
        }
    }

    /// The phone tile's second line.
    pub fn phone_state(&self) -> &'static str {
        match self.connection {
            Connection::Connected { .. } => m::STATUS_CONNECTED,
            Connection::Reconnecting { .. } => m::STATUS_RECONNECTING,
            Connection::Approval { .. } => m::LINK_APPROVAL,
            Connection::Connecting { .. } => m::LINK_CONNECTING,
            Connection::NotConnected => m::PC_OPEN_PHONE,
        }
    }

    /// The first repair message that applies.
    pub fn repair_text(&self) -> &'static str {
        if self.repair.mic {
            m::PC_REPAIR_MIC
        } else if self.repair.cam {
            m::PC_REPAIR_CAM
        } else {
            m::PC_REPAIR_NET
        }
    }

    /// Noise reduction level (0..10).
    pub fn noise_reduction(&self) -> u8 {
        self.value("mic.noiseReduction").parse().unwrap_or(5)
    }

    /// Mic volume in dB (-12..+12).
    pub fn volume(&self) -> i32 {
        self.value("mic.volume").parse().unwrap_or(0)
    }

    /// Voice isolation on/off.
    pub fn voice_isolation(&self) -> bool {
        self.value("mic.isolation") != "off"
    }

    /// Auto level on/off.
    pub fn auto_level(&self) -> bool {
        self.value("mic.autoLevel") != "off"
    }

    pub fn rows(&self) -> Vec<Row> {
        rows(self)
    }

    /// The link an external target opens.
    pub fn link_at(&self, target: Target) -> Option<Link> {
        match target {
            Target::HelpStepDownload => Some(Link::Website),
            Target::HelpPrivacy => Some(Link::Privacy),
            Target::HelpDonate => Some(Link::Donate),
            _ => None,
        }
    }

    /// What a click does.
    pub fn click(&self, target: Target) -> Option<Action> {
        let feature = |which, _state: Feature| self.connected().then_some(Action::Pause(which));
        let pairing = |id| self.paired.then_some(Action::Cycle(id));
        match target {
            Target::Camera => feature(Which::Camera, self.camera),
            Target::Mic => feature(Which::Mic, self.mic),
            Target::Speaker => feature(Which::Speaker, self.speaker),
            Target::Mirror => pairing("camera.mirror"),
            Target::Flip => pairing("camera.lens"),
            Target::Repair => (!self.repairing).then_some(Action::Repair),
            Target::Allow => Some(Action::Allow),
            Target::Deny => Some(Action::Deny),
            Target::ConnType(i) => {
                let id = match i {
                    0 => "link.usbDebugging",
                    1 => "link.usbTethering",
                    2 => "link.wifi",
                    3 => "link.bluetooth",
                    _ => return None,
                };
                Some(Action::Cycle(id))
            }
            Target::PhoneRow(i, button) => {
                let phone = self.phones.get(i)?;
                match button {
                    1 => Some(Action::RemovePhone(phone.id.clone())),
                    2 => Some(Action::BlockPhone(phone.id.clone(), !phone.blocked)),
                    _ => None,
                }
            }
            Target::NoiseSlider(frac) => {
                if !self.paired {
                    return None;
                }
                let step = (frac * 10.0).round() as u8;
                Some(Action::Set("mic.noiseReduction", step.to_string()))
            }
            Target::VolumeSlider(frac) => {
                if !self.paired {
                    return None;
                }
                const STEPS: [&str; 9] = ["-12", "-9", "-6", "-3", "0", "3", "6", "9", "12"];
                let idx = ((frac * 8.0).round() as usize).min(8);
                Some(Action::Set("mic.volume", STEPS[idx].to_string()))
            }
            Target::ToggleIsolation => pairing("mic.isolation"),
            Target::ToggleAutoLevel => pairing("mic.autoLevel"),
            _ => None,
        }
    }
}

pub fn feature_text(f: Feature) -> &'static str {
    match f {
        Feature::Off => m::FEATURE_OFF,
        Feature::On => m::FEATURE_ON,
        Feature::Paused => m::FEATURE_PAUSED,
        Feature::Restarting => m::FEATURE_RESTARTING,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RowAction {
    None,
    Cycle(&'static str),
    Phone(usize),
    Repair,
    Open(Link),
    Quit,
}

/// One line of settings for lists and testing.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub label: String,
    pub value: String,
    pub header: bool,
    pub enabled: bool,
    pub buttons: Option<[&'static str; 2]>,
    pub action: RowAction,
}

impl Row {
    fn header(label: &'static str) -> Self {
        Row {
            label: label.into(),
            value: String::new(),
            header: true,
            enabled: true,
            buttons: None,
            action: RowAction::None,
        }
    }

    fn plain(label: impl Into<String>, value: impl Into<String>, action: RowAction) -> Self {
        Row {
            label: label.into(),
            value: value.into(),
            header: false,
            enabled: true,
            buttons: None,
            action,
        }
    }
}

/// A setting's value as displayed: noise reduction 0 is "Off", volume in dB.
pub fn value_text(id: &str, value: &str) -> String {
    match (id, value) {
        ("mic.noiseReduction", "0") => m::VALUE_OFF.into(),
        ("mic.volume", v) => {
            let db = if v.starts_with('-') || v == "0" {
                v.to_string()
            } else {
                format!("+{v}")
            };
            fill(m::VALUE_DB, &[("db", &db)])
        }
        _ => m::text(&format!("value.{value}")).unwrap_or(value).into(),
    }
}

fn rows(s: &PanelState) -> Vec<Row> {
    let choice = |id: &'static str| {
        let shown = value_text(id, s.value(id));
        let mut row = Row::plain(
            m::text(&format!("setting.{id}")).unwrap_or(id),
            shown,
            RowAction::Cycle(id),
        );
        row.enabled = s.paired || id.starts_with("general.");
        row
    };
    let mut out = vec![
        Row::header(m::SECTION_MIC),
        choice("mic.noiseReduction"),
        choice("mic.volume"),
        choice("mic.isolation"),
        choice("mic.autoLevel"),
    ];
    out.extend([
        Row::header(m::SECTION_CAMERA),
        choice("camera.quality"),
        choice("camera.frameRate"),
    ]);
    out.extend([Row::header(m::SECTION_SPEAKER), choice("speaker.quietPc")]);
    out.push(Row::header(m::SECTION_CONNECTION));
    out.extend(
        [
            "link.usbDebugging",
            "link.usbTethering",
            "link.wifi",
            "link.bluetooth",
        ]
        .map(choice),
    );
    out.push(Row::plain(
        m::PC_ADDRESSES,
        s.addresses.join(", "),
        RowAction::None,
    ));
    out.push(Row::header(m::SECTION_PHONES));
    if s.phones.is_empty() {
        out.push(Row::plain(m::PC_NO_PHONES, "", RowAction::None));
    }
    for (i, p) in s.phones.iter().enumerate() {
        let mut row = Row::plain(
            p.name.clone(),
            if p.blocked { m::PC_BLOCKED } else { "" },
            RowAction::Phone(i),
        );
        row.buttons = Some([
            m::UI_REMOVE,
            if p.blocked {
                m::UI_UNBLOCK
            } else {
                m::UI_BLOCK
            },
        ]);
        out.push(row);
    }
    out.extend([
        Row::header(m::SECTION_GENERAL),
        choice("general.startWithWindows"),
    ]);
    let mut repair = Row::plain(m::PC_REPAIR_ALL, "", RowAction::Repair);
    repair.enabled = !s.repairing;
    out.extend([repair, Row::plain(m::PC_QUIT, "", RowAction::Quit)]);
    out.extend([
        Row::header(m::SECTION_ABOUT),
        Row::plain(
            fill(m::UI_VERSION, &[("version", &s.version)]),
            "",
            RowAction::None,
        ),
        Row::plain(m::UI_LICENCES, "", RowAction::Open(Link::Licences)),
        Row::plain(m::UI_WEBSITE, "", RowAction::Open(Link::Website)),
        Row::plain(m::UI_DONATE, "", RowAction::Open(Link::Donate)),
    ]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connected() -> PanelState {
        PanelState {
            connection: Connection::Connected {
                phone: "Pixel 8".into(),
                link: LINK_WIFI,
                weak: false,
            },
            paired: true,
            settings: [
                ("camera.framing", "fill"),
                ("camera.mirror", "off"),
                ("camera.lens", "front"),
                ("mic.volume", "6"),
                ("mic.noiseReduction", "0"),
                ("mic.isolation", "on"),
                ("mic.autoLevel", "on"),
                ("general.startWithWindows", "on"),
            ]
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect(),
            version: "1.0.0".into(),
            ..PanelState::default()
        }
    }

    #[test]
    fn the_indicator_is_red_yellow_or_the_green_link_icon() {
        assert_eq!(indicator(&Connection::NotConnected).color, color::RED);
        let i = indicator(&connected().connection);
        assert_eq!((i.color, i.label), (color::GREEN, m::LINK_WIFI));
        assert_eq!(i.icon, icons::WIFI);
        let weak = indicator(&Connection::Connected {
            phone: "P".into(),
            link: LINK_USB_TETHERING,
            weak: true,
        });
        assert_eq!((weak.color, weak.icon), (color::YELLOW, icons::CABLE));
    }

    #[test]
    fn the_tooltip_says_the_most_important_thing() {
        let mut s = connected();
        assert_eq!(s.tooltip(), "Owlmic · Pixel 8 over Wi-Fi");
        s.mic = Feature::On;
        s.camera = Feature::On;
        assert_eq!(s.tooltip(), "Owlmic · Mic and Camera on");
        s.repair.cam = true;
        assert_eq!(s.tooltip(), m::TRAY_REPAIR);
        assert_eq!(PanelState::default().tooltip(), m::TRAY_NO_PHONE);
    }

    #[test]
    fn tiles_toggle_features_when_connected() {
        let mut s = connected();
        assert_eq!(s.click(Target::Mic), Some(Action::Pause(Which::Mic)));
        s.mic = Feature::On;
        assert_eq!(s.click(Target::Mic), Some(Action::Pause(Which::Mic)));
        assert_eq!(s.click(Target::Camera), Some(Action::Pause(Which::Camera)));
        s.camera = Feature::On;
        assert_eq!(s.click(Target::Camera), Some(Action::Pause(Which::Camera)));
        s.camera = Feature::Paused;
        assert_eq!(s.click(Target::Camera), Some(Action::Pause(Which::Camera)));
        assert_eq!(s.click(Target::Flip), Some(Action::Cycle("camera.lens")));
        s.paired = false;
        assert_eq!(s.click(Target::Mirror), None);
    }

    #[test]
    fn settings_rows_show_value_texts_and_phone_buttons() {
        let mut s = connected();
        s.phones.push(PhoneEntry {
            id: "01".into(),
            name: "Pixel 8".into(),
            blocked: false,
        });
        let rows = s.rows();
        let value_of = |label: &str| &rows.iter().find(|r| r.label == label).unwrap().value;
        assert_eq!(value_of(m::SETTING_MIC_VOLUME), "+6 dB");
        assert_eq!(value_of(m::SETTING_MIC_NOISE_REDUCTION), m::VALUE_OFF);
        assert_eq!(value_of(m::SETTING_MIC_AUTO_LEVEL), m::VALUE_ON);
        assert_eq!(value_text("mic.volume", "-9"), "-9 dB");
        assert_eq!(value_text("mic.volume", "0"), "0 dB");
        assert_eq!(value_text("mic.noiseReduction", "7"), "7");

        assert_eq!(
            s.click(Target::PhoneRow(0, 2)),
            Some(Action::BlockPhone("01".into(), true))
        );
        assert_eq!(
            s.click(Target::PhoneRow(0, 1)),
            Some(Action::RemovePhone("01".into()))
        );

        assert_eq!(s.link_at(Target::HelpStepDownload), Some(Link::Website));
        assert_eq!(s.link_at(Target::HelpPrivacy), Some(Link::Privacy));
        assert_eq!(s.link_at(Target::HelpDonate), Some(Link::Donate));

        s.paired = false;
        assert_eq!(s.click(Target::NoiseSlider(0.5)), None);
        assert_eq!(s.click(Target::VolumeSlider(0.5)), None);
    }
}
