//! What the panel and the tray show for a state (SYSTEM_DESIGN sections 33 to 37), and what a
//! click on each tile does. Texts come only from design/copy.json.

use crate::glyph::Icon;
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
    pub hostname: String,
}

/// What a click asks the app to do. Opening settings or a link, and scrolling, stay in the
/// panel.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Pause(Which),
    /// Moves a setting to its next value.
    Cycle(&'static str),
    /// Explicitly sets a setting to a given value.
    Set(&'static str, String),
    Allow,
    Deny,
    Repair,
    Refresh,
    RemovePhone(String),
    BlockPhone(String, bool),
    Disconnect,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Link {
    Licences,
    Website,
    Donate,
    GitHub,
    Rate,
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
        _ => icons::GLOBE,
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

/// The connection indicator (SYSTEM_DESIGN section 36). Monochrome matching prototype.
pub fn indicator(c: &Connection) -> Indicator {
    let globe = |label| Indicator {
        color: color::TEXT,
        icon: icons::GLOBE,
        label,
    };
    let ind = |link, label| Indicator {
        color: color::TEXT,
        icon: link_icon(link),
        label,
    };
    match c {
        Connection::NotConnected => Indicator {
            color: color::DISABLED,
            icon: icons::GLOBE,
            label: m::LINK_NOT_CONNECTED,
        },
        Connection::Connecting { .. } => globe(m::LINK_CONNECTING),
        Connection::Approval { .. } => globe(m::LINK_APPROVAL),
        Connection::Reconnecting { .. } => globe(m::STATUS_RECONNECTING),
        Connection::Connected {
            link, weak: true, ..
        } => ind(*link, m::LINK_WEAK),
        Connection::Connected { link, .. } => ind(
            *link,
            match *link {
                LINK_USB_DEBUGGING => m::LINK_USB_DEBUGGING,
                LINK_USB_TETHERING => m::LINK_USB_TETHERING,
                LINK_BLUETOOTH => m::LINK_BLUETOOTH,
                _ => m::LINK_WIFI,
            },
        ),
    }
}

impl PanelState {
    pub fn value(&self, id: &str) -> &str {
        self.settings.get(id).map_or("", String::as_str)
    }

    pub fn setting(&self, id: &str) -> &str {
        self.value(id)
    }

    /// Normalizes both `"on"`/`"off"` (store) and `"true"`/`"false"` boolean strings.
    pub fn is_on(&self, id: &str, default: bool) -> bool {
        match self.value(id) {
            "on" | "true" | "1" => true,
            "off" | "false" | "0" => false,
            _ => default,
        }
    }

    pub fn camera_ratio_label(&self) -> &'static str {
        match self.value("camera.ratio") {
            "16:9" => "16:9",
            "4:3" => "4:3",
            "2:1" => "2:1",
            "1:1" => "1:1",
            "1:2" => "1:2",
            "3:4" => "3:4",
            "9:16" => "9:16",
            _ => "Auto",
        }
    }

    pub fn camera_wide(&self, stream_size: (usize, usize), square_wide: bool) -> bool {
        match self.camera_ratio_label() {
            "16:9" | "4:3" | "2:1" => true,
            "9:16" | "3:4" | "1:2" => false,
            "1:1" => square_wide,
            _ => stream_size.0 > stream_size.1,
        }
    }

    pub fn mic_nr_value(&self) -> f32 {
        self.value("mic.noiseReduction")
            .parse::<f32>()
            .unwrap_or(5.0)
            .clamp(0.0, 10.0)
    }

    pub fn mic_volume_db(&self) -> i32 {
        let v = self.value("mic.volume");
        if let Ok(n) = v.parse::<f32>() {
            if (-12.0..=12.0).contains(&n) {
                return n.round() as i32;
            }
            return ((n / 100.0 * 24.0) - 12.0).round() as i32;
        }
        0
    }

    pub fn mic_volume_percent(&self) -> f32 {
        let v = self.value("mic.volume");
        if let Ok(n) = v.parse::<f32>() {
            if n > 12.0 {
                return n.clamp(0.0, 100.0);
            }
            return (((n.clamp(-12.0, 12.0) + 12.0) / 24.0) * 100.0).clamp(0.0, 100.0);
        }
        50.0
    }

    pub fn host_ip(&self) -> String {
        if let Some(addr) = self.addresses.first()
            && !addr.is_empty()
        {
            return addr.clone();
        }
        let host = if self.hostname.is_empty() {
            "127.0.0.1"
        } else {
            &self.hostname
        };
        format!("{host}:7653")
    }

    pub fn is_blocked(&self) -> bool {
        let id = self.active_phone_id().or_else(|| self.first_paired_id());
        id.and_then(|id| self.phones.iter().find(|p| p.id == id).map(|p| p.blocked))
            .unwrap_or(false)
    }

    pub fn active_screen(
        &self,
        drawer: crate::layout::Drawer,
        cam_menu: bool,
        quick_audio: bool,
        cam_wide: bool,
    ) -> Screen {
        match drawer {
            crate::layout::Drawer::Wifi => {
                let mode = if matches!(self.connection, Connection::Approval { .. }) {
                    crate::layout::WifiMode::Pairing
                } else if self.connected() {
                    crate::layout::WifiMode::Connected
                } else if self.is_blocked() {
                    crate::layout::WifiMode::Blocked
                } else if self.first_paired_id().is_some() {
                    crate::layout::WifiMode::Forget
                } else {
                    crate::layout::WifiMode::Offline
                };
                Screen::DrawerWifi(mode)
            }
            crate::layout::Drawer::Help(h) => Screen::DrawerHelp(h),
            crate::layout::Drawer::None => Screen::Main {
                cam_on: matches!(self.camera, Feature::On | Feature::Restarting)
                    && self.connected(),
                mic_on: matches!(self.mic, Feature::On | Feature::Restarting) && self.connected(),
                cam_menu,
                quick_audio,
                cam_wide,
            },
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

    pub fn primary_phone_name(&self) -> &str {
        self.phone_name()
            .or_else(|| self.phones.first().map(|p| p.name.as_str()))
            .unwrap_or("No phone")
    }

    /// Secondary paired phone (never duplicating the device already shown in the primary row).
    pub fn secondary_phone(&self) -> Option<&PhoneEntry> {
        let primary_id = self.active_phone_id().or_else(|| self.first_paired_id());
        let primary_name = self.primary_phone_name();
        self.phones
            .iter()
            .find(|p| primary_id.as_deref() != Some(p.id.as_str()) && p.name != primary_name)
    }

    pub fn connected(&self) -> bool {
        matches!(
            self.connection,
            Connection::Connected { .. } | Connection::Reconnecting { .. }
        )
    }

    pub fn bluetooth(&self) -> bool {
        matches!(
            self.connection,
            Connection::Connected {
                link: LINK_BLUETOOTH,
                ..
            } | Connection::Reconnecting {
                link: LINK_BLUETOOTH,
                ..
            }
        )
    }

    /// The tray tooltip (SYSTEM_DESIGN section 34).
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

    pub fn rows(&self) -> Vec<Row> {
        rows(self)
    }

    pub fn active_phone_id(&self) -> Option<String> {
        let name = self.phone_name()?;
        self.phones
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.id.clone())
            .or_else(|| self.phones.first().map(|p| p.id.clone()))
    }

    pub fn first_paired_id(&self) -> Option<String> {
        self.phones.first().map(|p| p.id.clone())
    }

    /// The link a Help tile opens directly.
    pub fn link_at(&self, target: Target) -> Option<Link> {
        match target {
            Target::HelpRate => Some(Link::Rate),
            Target::HelpGitHub => Some(Link::GitHub),
            Target::HelpDonate => Some(Link::Donate),
            _ => None,
        }
    }

    /// What a click does.
    pub fn click(&self, target: Target) -> Option<Action> {
        let feature = |which| self.connected().then_some(Action::Pause(which));
        let pairing = |id| self.paired.then_some(Action::Cycle(id));
        match target {
            Target::Camera => (!self.bluetooth())
                .then(|| feature(Which::Camera))
                .flatten(),
            Target::Mic => feature(Which::Mic),
            Target::Speaker => feature(Which::Speaker),
            Target::Ratio(r) => {
                let val = match r {
                    "Auto" => "full",
                    other => other,
                };
                Some(Action::Set("camera.ratio", val.to_string()))
            }
            Target::Mirror => pairing("camera.mirror"),
            Target::Flip => pairing("camera.lens"),
            Target::ToggleIso => Some(Action::Cycle("mic.isolation")),
            Target::ToggleNorm => Some(Action::Cycle("mic.autoLevel")),
            Target::NrFader(val) => Some(Action::Set(
                "mic.noiseReduction",
                (val.round() as i32).clamp(0, 10).to_string(),
            )),
            Target::VolFader(val) => {
                const VOL_STEPS: &[&str] = &["-12", "-9", "-6", "-3", "0", "3", "6", "9", "12"];
                let idx = ((val / 100.0 * 8.0).round() as usize).min(8);
                Some(Action::Set("mic.volume", VOL_STEPS[idx].to_string()))
            }
            Target::Pair => Some(Action::Allow),
            Target::Decline => Some(Action::Deny),
            Target::Disconnect => Some(Action::Disconnect),
            Target::Block => {
                let id = self.active_phone_id().or_else(|| self.first_paired_id())?;
                Some(Action::BlockPhone(id, true))
            }
            Target::Unblock => {
                let id = self.active_phone_id().or_else(|| self.first_paired_id())?;
                Some(Action::BlockPhone(id, false))
            }
            Target::RemovePaired => {
                let id = self
                    .secondary_phone()
                    .map(|p| p.id.clone())
                    .or_else(|| self.first_paired_id())?;
                Some(Action::RemovePhone(id))
            }
            Target::Forget => {
                let id = self.active_phone_id().or_else(|| self.first_paired_id())?;
                Some(Action::RemovePhone(id))
            }
            Target::ToggleTransport(link) => {
                let id = match link {
                    1 => "link.usbDebugging",
                    2 => "link.usbTethering",
                    3 => "link.wifi",
                    4 => "link.bluetooth",
                    _ => return None,
                };
                Some(Action::Cycle(id))
            }
            Target::Refresh => Some(Action::Refresh),
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

/// One line of the settings view.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub label: String,
    pub value: String,
    pub header: bool,
    pub enabled: bool,
    /// Phone rows: Remove, then Block or Unblock.
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

/// A setting's value as the panel shows it: noise reduction's 0 is "Off", the volume is in dB.
fn value_text(id: &str, value: &str) -> String {
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

/// The PC settings (SYSTEM_DESIGN section 33.3).
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
                ("mic.volume", "80"),
                ("mic.noiseReduction", "5"),
                ("mic.autoLevel", "on"),
                ("camera.mirror", "off"),
                ("camera.quality", "1080p"),
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
    fn setting_normalization_handles_on_off_and_labels() {
        let s = connected();
        assert!(s.is_on("mic.autoLevel", false));
        assert!(!s.is_on("camera.mirror", true));
        assert_eq!(s.camera_ratio_label(), "Auto");
        assert!(!s.camera_wide((1080, 1920), false));
        assert!(s.camera_wide((1920, 1080), false));
        assert_eq!(s.mic_nr_value(), 5.0);
        assert_eq!(s.mic_volume_percent(), 80.0);
    }

    #[test]
    fn the_indicator_is_monochrome_with_link_icon() {
        let off = indicator(&Connection::NotConnected);
        assert_eq!((off.color, off.icon), (color::DISABLED, icons::GLOBE));
        assert_eq!(
            indicator(&Connection::Connecting { link: LINK_WIFI }).icon,
            icons::GLOBE
        );
        assert_eq!(
            indicator(&Connection::Approval {
                phone: "P".into(),
                code: "123456".into(),
                link: LINK_USB_DEBUGGING,
            })
            .icon,
            icons::GLOBE
        );
        assert_eq!(
            indicator(&Connection::Reconnecting {
                phone: "P".into(),
                link: LINK_WIFI,
            })
            .icon,
            icons::GLOBE
        );
        let i = indicator(&connected().connection);
        assert_eq!((i.color, i.label), (color::TEXT, m::LINK_WIFI));
        assert_eq!(i.icon, icons::WIFI);
        let weak = indicator(&Connection::Connected {
            phone: "P".into(),
            link: LINK_USB_TETHERING,
            weak: true,
        });
        assert_eq!((weak.color, weak.icon), (color::TEXT, icons::CABLE));
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
    fn buttons_toggle_when_connected() {
        let mut s = connected();
        assert_eq!(s.click(Target::Mic), Some(Action::Pause(Which::Mic)));
        s.mic = Feature::On;
        assert_eq!(s.click(Target::Mic), Some(Action::Pause(Which::Mic)));
        assert_eq!(s.click(Target::Camera), Some(Action::Pause(Which::Camera)));
        s.camera = Feature::On;
        assert_eq!(s.click(Target::Camera), Some(Action::Pause(Which::Camera)));
        assert_eq!(
            s.click(Target::Speaker),
            Some(Action::Pause(Which::Speaker))
        );
        s.speaker = Feature::On;
        assert_eq!(
            s.click(Target::Speaker),
            Some(Action::Pause(Which::Speaker))
        );
        assert_eq!(s.click(Target::Flip), Some(Action::Cycle("camera.lens")));
        s.paired = false;
        assert_eq!(s.click(Target::Flip), None);

        let not_conn = PanelState::default();
        assert_eq!(not_conn.click(Target::Mic), None);
        assert_eq!(not_conn.click(Target::Camera), None);
        assert_eq!(not_conn.click(Target::Speaker), None);
    }

    #[test]
    fn connection_and_bento_targets_work() {
        let mut s = connected();
        s.phones.push(PhoneEntry {
            id: "01".into(),
            name: "Pixel 8".into(),
            blocked: false,
        });
        assert_eq!(
            s.click(Target::Block),
            Some(Action::BlockPhone("01".into(), true))
        );
        assert_eq!(
            s.click(Target::Unblock),
            Some(Action::BlockPhone("01".into(), false))
        );
        assert_eq!(
            s.click(Target::RemovePaired),
            Some(Action::RemovePhone("01".into()))
        );
        assert_eq!(s.click(Target::Disconnect), Some(Action::Disconnect));
        assert_eq!(
            s.click(Target::ToggleTransport(3)),
            Some(Action::Cycle("link.wifi"))
        );
        assert_eq!(s.link_at(Target::HelpGitHub), Some(Link::GitHub));

        let mut dup = connected();
        dup.phones = vec![
            PhoneEntry {
                id: "01".into(),
                name: "Pixel 8".into(),
                blocked: false,
            },
            PhoneEntry {
                id: "02".into(),
                name: "Pixel 8".into(),
                blocked: false,
            },
        ];
        assert!(dup.secondary_phone().is_none());
        dup.phones.push(PhoneEntry {
            id: "03".into(),
            name: "Galaxy S24".into(),
            blocked: false,
        });
        assert_eq!(dup.secondary_phone().map(|p| p.id.as_str()), Some("03"));
    }

    #[test]
    fn bluetooth_disables_camera_clicks_and_refresh_fires_action() {
        let mut s = connected();
        s.connection = Connection::Connected {
            phone: "Pixel 8".into(),
            link: LINK_BLUETOOTH,
            weak: false,
        };
        assert!(s.bluetooth());
        assert_eq!(s.click(Target::Camera), None);
        assert_eq!(s.click(Target::Mic), Some(Action::Pause(Which::Mic)));
        assert_eq!(s.click(Target::Refresh), Some(Action::Refresh));
    }
}
