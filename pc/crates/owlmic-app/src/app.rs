//! The App Hub (SYSTEM_DESIGN section 11.1): routes every hub's events to the hubs that act on
//! them, and publishes one snapshot for the panel and the tray. It holds no media.

use owlmic_devices::DeviceHealth;
use owlmic_hub::{Health, Hub, Outbox, Publisher};
use owlmic_link::{LinkEvent, LinkMsg, LinkView};
use owlmic_media::hub::{Feature as MediaFeature, MediaEvent, MediaMsg};
use owlmic_media::video::Framing;
use owlmic_proto::messages::{FeatureState, Message, Settings, State};
use owlmic_session::hub::{PhaseView, SessionEvent, SessionMsg, SessionView};
use owlmic_session::{hex, id_from_hex};
use owlmic_settings::hub::{SettingsEvent, SettingsMsg, SettingsView};
use owlmic_settings::store::Store;
use owlmic_ui::view::{Action, Connection, Feature, PanelState, PhoneEntry, Repair, Which};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A unit restarting for this long counts as broken, like one that failed.
const LONG_DEGRADED: Duration = Duration::from_secs(10);

pub enum AppMsg {
    Link(LinkEvent),
    Session(SessionEvent),
    Media(MediaEvent),
    Settings(SettingsEvent),
    Health(DeviceHealth),
    Repairing(bool),
    Ui(Action),
    /// This PC's addresses, for typing into the phone.
    Addresses(Vec<String>),
    /// A supervised hub or I/O thread's health, as it changes.
    Unit {
        name: &'static str,
        health: Health,
    },
    /// The Device Hub was built again after a crash: it starts with nothing on.
    DevicesStarted,
}

impl AppMsg {
    /// What a full mailbox may drop: views and reports a newer one replaces, and keyframe
    /// requests the receiver repeats.
    pub fn droppable(&self) -> bool {
        matches!(
            self,
            AppMsg::Link(
                LinkEvent::View(_)
                    | LinkEvent::Stalled { .. }
                    | LinkEvent::FromPhone(Message::Report(_))
            ) | AppMsg::Media(MediaEvent::View(_) | MediaEvent::ToPhone(Message::KeyframeRequest))
                | AppMsg::Settings(SettingsEvent::View(_))
                | AppMsg::Session(SessionEvent::Changed(_))
                | AppMsg::Addresses(_)
        )
    }
}

/// The part of the Repair tile a broken unit shows as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    Mic,
    Net,
}

/// Bluetooth and USB debugging are optional links, so they never ask for a repair.
fn repair_part(unit: &str) -> Option<Part> {
    match unit {
        "devices" | "media" => Some(Part::Mic),
        "session" | "settings" | "link" | "listener" | "answerer" | "udp" => Some(Part::Net),
        _ => None,
    }
}

/// What the App Hub asks of the Device Hub, kept free of Windows types.
#[derive(Debug, Clone, PartialEq)]
pub enum DeviceCommand {
    Mic(bool),
    Speaker(bool),
    Camera(bool),
    QuietPc(bool),
    Shape { framing: Framing, mirror: bool },
    Repair,
}

pub struct Wiring {
    pub link: Outbox<LinkMsg>,
    pub session: Outbox<SessionMsg>,
    pub media: Outbox<MediaMsg>,
    pub settings: Outbox<SettingsMsg>,
    pub devices: Box<dyn Fn(DeviceCommand) + Send>,
    pub autostart: Box<dyn Fn(bool) + Send>,
    pub quit: Box<dyn Fn() + Send>,
}

#[derive(Default)]
struct AppState {
    session: SessionView,
    link: LinkView,
    /// The link of the last phone that proved itself, for the indicator while it waits.
    proving_link: u8,
    features: State,
    /// Streams the phone should send that have stalled long enough to tell the user.
    restarting: [bool; 4],
    settings: SettingsView,
    health: DeviceHealth,
    repairing: bool,
    addresses: Vec<String>,
    /// The link that carried the session last, for the indicator while it reconnects.
    last_link: Option<u8>,
    /// Supervised units that aren't well, and since when.
    units: BTreeMap<&'static str, (Health, Instant)>,
    /// What the Device Hub was last told, to tell it again after it restarts.
    devices: Devices,
}

#[derive(Default)]
struct Devices {
    mic: Option<bool>,
    camera: Option<bool>,
    speaker: Option<bool>,
    quiet: Option<bool>,
    shape: Option<(Framing, bool)>,
}

pub struct AppHub {
    w: Wiring,
    store: Arc<Store>,
    ui: Publisher<PanelState>,
    state: AppState,
    shown: PanelState,
    /// When a unit that is still restarting will have been at it long enough to show.
    recheck_at: Option<Instant>,
}

/// A setting's next value, for a tile or row that cycles through them.
fn next_value(id: &str, current: &str) -> Option<String> {
    let values = owlmic_settings::find(id)?.values;
    let i = values
        .iter()
        .position(|v| *v == current)
        .map_or(0, |i| (i + 1) % values.len());
    Some(values[i].to_owned())
}

impl AppHub {
    pub fn new(w: Wiring, store: Arc<Store>, ui: Publisher<PanelState>) -> Self {
        let state = AppState {
            proving_link: owlmic_link::LINK_WIFI,
            ..AppState::default()
        };
        Self {
            w,
            store,
            ui,
            state,
            shown: PanelState::default(),
            recheck_at: None,
        }
    }

    fn on_link(&mut self, e: LinkEvent) {
        match e {
            LinkEvent::Hello {
                conn,
                phone,
                resume,
            } => self.w.session.send(SessionMsg::Hello {
                conn,
                phone,
                resume,
            }),
            LinkEvent::Proven {
                conn,
                phone,
                resume,
                code,
                link,
            } => {
                self.state.proving_link = link;
                self.w.session.send(SessionMsg::Proven {
                    conn,
                    phone,
                    resume,
                    code,
                    link,
                });
            }
            LinkEvent::ConnClosed { conn } => self.w.session.send(SessionMsg::ConnClosed { conn }),
            LinkEvent::LinkUp { session_id, link } => {
                self.w.session.send(SessionMsg::LinkUp { session_id, link });
                self.w.media.send(MediaMsg::Link(link));
            }
            LinkEvent::LinkLost { session_id } => {
                self.w.session.send(SessionMsg::LinkLost { session_id })
            }
            LinkEvent::Bye { session_id } => self.w.session.send(SessionMsg::Bye { session_id }),
            LinkEvent::FromPhone(Message::Settings(s)) => {
                self.w.settings.send(SettingsMsg::FromPhone(s.changes))
            }
            LinkEvent::FromPhone(
                m @ (Message::State(_)
                | Message::StreamStart(_)
                | Message::StreamStop(_)
                | Message::RestartStream(_)
                | Message::Report(_)),
            ) => {
                self.w.media.send(MediaMsg::FromPhone(m));
            }
            LinkEvent::FromPhone(_) => {}
            LinkEvent::Stalled { stream, tell_user } => {
                if let Some(r) = self.state.restarting.get_mut(stream as usize) {
                    *r = tell_user;
                }
            }
            LinkEvent::Recovered { stream } => {
                if let Some(r) = self.state.restarting.get_mut(stream as usize) {
                    *r = false;
                }
            }
            LinkEvent::View(v) => {
                self.state.last_link = v.link.or(self.state.last_link);
                self.state.link = v;
            }
        }
    }

    fn on_session(&mut self, e: SessionEvent) {
        match e {
            SessionEvent::Decided { conn, decision } => {
                self.w.link.send(LinkMsg::Decided { conn, decision })
            }
            SessionEvent::Welcome {
                conn,
                session_id,
                phone,
            } => {
                let key = hex(&phone.id);
                let settings = self
                    .store
                    .read(|d| d.pairings.get(&key).cloned())
                    .unwrap_or_default();
                self.w.link.send(LinkMsg::Welcome {
                    conn,
                    session_id,
                    settings,
                });
                self.w.settings.send(SettingsMsg::Session(Some(key)));
            }
            SessionEvent::Approval { conn } => self.w.link.send(LinkMsg::Pending { conn }),
            SessionEvent::Rejected {
                conn,
                reason,
                owner,
            } => self.w.link.send(LinkMsg::Reject {
                conn,
                reason,
                owner,
            }),
            SessionEvent::Ended { session_id } => {
                self.w.link.send(LinkMsg::SessionEnded { session_id });
                self.w.media.send(MediaMsg::Ended);
                self.w.settings.send(SettingsMsg::Session(None));
                self.state.restarting = [false; 4];
                self.state.last_link = None;
            }
            SessionEvent::Changed(view) => self.state.session = view,
        }
    }

    fn on_media(&mut self, e: MediaEvent) {
        let d = &mut self.state.devices;
        let command = match e {
            MediaEvent::Mic(on) => DeviceCommand::Mic(*d.mic.insert(on)),
            MediaEvent::Camera(on) => DeviceCommand::Camera(*d.camera.insert(on)),
            MediaEvent::Speaker(on) => DeviceCommand::Speaker(*d.speaker.insert(on)),
            MediaEvent::Shape { framing, mirror } => {
                d.shape = Some((framing, mirror));
                DeviceCommand::Shape { framing, mirror }
            }
            MediaEvent::QuietPc(on) => DeviceCommand::QuietPc(*d.quiet.insert(on)),
            MediaEvent::ToPhone(m) => {
                self.w.link.send(LinkMsg::Send(m));
                return;
            }
            MediaEvent::View(s) => {
                self.state.features = s;
                return;
            }
        };
        (self.w.devices)(command);
    }

    /// A rebuilt Device Hub has nothing on: it hears again what it was last told.
    fn devices_started(&self) {
        let d = &self.state.devices;
        let commands = [
            d.quiet.map(DeviceCommand::QuietPc),
            d.shape
                .map(|(framing, mirror)| DeviceCommand::Shape { framing, mirror }),
            d.mic.map(DeviceCommand::Mic),
            d.camera.map(DeviceCommand::Camera),
            d.speaker.map(DeviceCommand::Speaker),
        ];
        for c in commands.into_iter().flatten() {
            (self.w.devices)(c);
        }
    }

    fn on_unit(&mut self, name: &'static str, health: Health) {
        if health == Health::Ok {
            self.state.units.remove(name);
        } else {
            let since = self
                .state
                .units
                .get(name)
                .map_or_else(Instant::now, |(_, t)| *t);
            self.state.units.insert(name, (health, since));
        }
    }

    /// Whether a unit behind this part of the Repair tile is broken now.
    fn unit_broken(&self, part: Part, now: Instant) -> bool {
        self.state.units.iter().any(|(name, (h, since))| {
            repair_part(name) == Some(part)
                && match h {
                    Health::Failed(_) => true,
                    Health::Degraded(_) => now.duration_since(*since) >= LONG_DEGRADED,
                    Health::Ok => false,
                }
        })
    }

    fn on_settings(&mut self, e: SettingsEvent) {
        match e {
            SettingsEvent::Apply(values) => {
                for (id, value) in values {
                    if id == "general.startWithWindows" {
                        (self.w.autostart)(value == "on");
                    } else {
                        self.w.media.send(MediaMsg::Setting { id, value });
                    }
                }
            }
            SettingsEvent::ToPhone(changes) => self
                .w
                .link
                .send(LinkMsg::Send(Message::Settings(Settings { changes }))),
            SettingsEvent::View(v) => self.state.settings = v,
        }
    }

    fn on_ui(&mut self, a: Action) {
        match a {
            Action::Pause(which) => {
                let (feature, now) = match which {
                    Which::Mic => (MediaFeature::Mic, self.state.features.mic),
                    Which::Camera => (MediaFeature::Camera, self.state.features.camera),
                    Which::Speaker => (MediaFeature::Speaker, self.state.features.speaker),
                };
                self.w.media.send(MediaMsg::Pause {
                    feature,
                    paused: now == FeatureState::On,
                });
            }
            Action::Cycle(id) => {
                let current = self
                    .state
                    .settings
                    .values
                    .get(id)
                    .cloned()
                    .unwrap_or_default();
                if let Some(value) = next_value(id, &current) {
                    self.w.settings.send(SettingsMsg::Set {
                        id: id.to_owned(),
                        value,
                    });
                }
            }
            Action::Allow => self
                .w
                .session
                .send(SessionMsg::UserDecision { allow: true }),
            Action::Deny => self
                .w
                .session
                .send(SessionMsg::UserDecision { allow: false }),
            Action::Repair => (self.w.devices)(DeviceCommand::Repair),
            Action::RemovePhone(id) => {
                if let Some(phone_id) = id_from_hex(&id) {
                    self.w.session.send(SessionMsg::RemovePhone { phone_id });
                }
            }
            Action::BlockPhone(id, blocked) => {
                if let Some(phone_id) = id_from_hex(&id) {
                    self.w
                        .session
                        .send(SessionMsg::SetBlocked { phone_id, blocked });
                }
            }
            Action::Quit => (self.w.quit)(),
        }
    }

    fn panel(&self, now: Instant) -> PanelState {
        let s = &self.state;
        let connection = match &s.session.phase {
            PhaseView::Idle => match s.link.connecting {
                Some(link) => Connection::Connecting { link },
                None => Connection::NotConnected,
            },
            PhaseView::Approval { phone, code } => Connection::Approval {
                phone: phone.clone(),
                code: code.clone(),
                link: s.proving_link,
            },
            PhaseView::Active { phone, link } => Connection::Connected {
                phone: phone.clone(),
                link: s.link.link.unwrap_or(*link),
                weak: s.link.weak,
            },
            PhaseView::Held { phone } => Connection::Reconnecting {
                phone: phone.clone(),
                link: s.link.link.or(s.last_link).unwrap_or(s.proving_link),
            },
        };
        let feature = |f: FeatureState, stream: usize| match f {
            FeatureState::On if s.restarting[stream] => Feature::Restarting,
            FeatureState::On => Feature::On,
            FeatureState::Paused => Feature::Paused,
            FeatureState::Off => Feature::Off,
        };
        PanelState {
            connection,
            mic: feature(s.features.mic, 1),
            camera: feature(s.features.camera, 2),
            speaker: feature(s.features.speaker, 3),
            settings: s.settings.values.clone(),
            paired: s.settings.paired,
            phones: s
                .session
                .phones
                .iter()
                .map(|p| PhoneEntry {
                    id: hex(&p.id),
                    name: p.name.clone(),
                    blocked: p.blocked,
                })
                .collect(),
            addresses: s.addresses.clone(),
            repair: Repair {
                mic: !s.health.mic || self.unit_broken(Part::Mic, now),
                cam: !s.health.cam,
                net: !s.health.net || self.unit_broken(Part::Net, now),
            },
            repairing: s.repairing,
            version: env!("CARGO_PKG_VERSION").to_owned(),
        }
    }
}

impl Hub for AppHub {
    type Msg = AppMsg;

    fn handle(&mut self, msg: AppMsg) {
        match msg {
            AppMsg::Link(e) => self.on_link(e),
            AppMsg::Session(e) => self.on_session(e),
            AppMsg::Media(e) => self.on_media(e),
            AppMsg::Settings(e) => self.on_settings(e),
            AppMsg::Health(h) => self.state.health = h,
            AppMsg::Repairing(r) => self.state.repairing = r,
            AppMsg::Ui(a) => self.on_ui(a),
            AppMsg::Addresses(a) => self.state.addresses = a,
            AppMsg::Unit { name, health } => self.on_unit(name, health),
            AppMsg::DevicesStarted => self.devices_started(),
        }
        self.show(Instant::now());
    }

    /// A unit that stays degraded turns into a repair.
    fn tick(&mut self, now: Instant) {
        self.show(now);
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.recheck_at
    }
}

impl AppHub {
    fn show(&mut self, now: Instant) {
        self.recheck_at = self
            .state
            .units
            .values()
            .filter(|(h, _)| matches!(h, Health::Degraded(_)))
            .map(|(_, since)| *since + LONG_DEGRADED)
            .filter(|t| *t > now)
            .min();
        let panel = self.panel(now);
        if panel != self.shown {
            self.shown = panel.clone();
            self.ui.publish(panel);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use owlmic_hub::{Inbox, mailbox};
    use owlmic_link::LinkView;
    use owlmic_session::sessions::Phone;
    use owlmic_settings::store::PlainProtector;
    use std::sync::Mutex;

    struct Rig {
        hub: AppHub,
        link: Inbox<LinkMsg>,
        session: Inbox<SessionMsg>,
        media: Inbox<MediaMsg>,
        settings: Inbox<SettingsMsg>,
        devices: Arc<Mutex<Vec<DeviceCommand>>>,
        autostart: Arc<Mutex<Vec<bool>>>,
        ui: Publisher<PanelState>,
    }

    fn rig() -> Rig {
        let (link, link_in) = mailbox(16);
        let (session, session_in) = mailbox(16);
        let (media, media_in) = mailbox(16);
        let (settings, settings_in) = mailbox(16);
        let devices = Arc::new(Mutex::new(Vec::new()));
        let autostart = Arc::new(Mutex::new(Vec::new()));
        let (d, a) = (devices.clone(), autostart.clone());
        let ui = Publisher::new(PanelState::default(), || {});
        let dir = std::env::temp_dir().join(format!(
            "owlmic-app-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let store = Arc::new(Store::open(
            dir.join("owlmic.json"),
            Box::new(PlainProtector),
        ));
        let w = Wiring {
            link,
            session,
            media,
            settings,
            devices: Box::new(move |c| d.lock().unwrap().push(c)),
            autostart: Box::new(move |on| a.lock().unwrap().push(on)),
            quit: Box::new(|| {}),
        };
        Rig {
            hub: AppHub::new(w, store, ui.clone()),
            link: link_in,
            session: session_in,
            media: media_in,
            settings: settings_in,
            devices,
            autostart,
            ui,
        }
    }

    #[test]
    fn a_welcome_sends_the_pairings_settings_and_starts_its_settings() {
        let mut r = rig();
        let phone = Phone {
            id: [7; 16],
            name: "Pixel 8".into(),
            model: "Pixel 8".into(),
            static_pub: String::new(),
        };
        r.hub.handle(AppMsg::Session(SessionEvent::Welcome {
            conn: 3,
            session_id: [1; 16],
            phone,
        }));
        assert!(matches!(
            r.link.try_recv(),
            Some(LinkMsg::Welcome { conn: 3, .. })
        ));
        assert!(
            matches!(r.settings.try_recv(), Some(SettingsMsg::Session(Some(k))) if k == hex(&[7; 16]))
        );
        r.hub.handle(AppMsg::Session(SessionEvent::Ended {
            session_id: [1; 16],
        }));
        assert!(matches!(
            r.link.try_recv(),
            Some(LinkMsg::SessionEnded { .. })
        ));
        assert!(matches!(r.media.try_recv(), Some(MediaMsg::Ended)));
    }

    #[test]
    fn phone_messages_reach_the_hubs_that_act_on_them() {
        let mut r = rig();
        r.hub
            .handle(AppMsg::Link(LinkEvent::FromPhone(Message::State(
                State::default(),
            ))));
        assert!(matches!(
            r.media.try_recv(),
            Some(MediaMsg::FromPhone(Message::State(_)))
        ));
        r.hub
            .handle(AppMsg::Link(LinkEvent::FromPhone(Message::Settings(
                Settings { changes: vec![] },
            ))));
        assert!(matches!(
            r.settings.try_recv(),
            Some(SettingsMsg::FromPhone(_))
        ));
        r.hub.handle(AppMsg::Link(LinkEvent::Hello {
            conn: 1,
            phone: Phone {
                id: [1; 16],
                name: "A".into(),
                model: "A".into(),
                static_pub: String::new(),
            },
            resume: None,
        }));
        assert!(matches!(
            r.session.try_recv(),
            Some(SessionMsg::Hello { conn: 1, .. })
        ));
    }

    #[test]
    fn media_commands_go_to_the_devices_and_settings_apply_live() {
        let mut r = rig();
        r.hub.handle(AppMsg::Media(MediaEvent::Mic(true)));
        assert_eq!(
            r.devices.lock().unwrap().as_slice(),
            &[DeviceCommand::Mic(true)]
        );
        r.hub.handle(AppMsg::Settings(SettingsEvent::Apply(vec![
            ("general.startWithWindows".into(), "off".into()),
            ("camera.mirror".into(), "on".into()),
        ])));
        assert_eq!(r.autostart.lock().unwrap().as_slice(), &[false]);
        assert!(
            matches!(r.media.try_recv(), Some(MediaMsg::Setting { id, .. }) if id == "camera.mirror")
        );
    }

    #[test]
    fn the_panel_shows_the_approval_and_tiles_cycle_settings() {
        let mut r = rig();
        r.hub
            .handle(AppMsg::Session(SessionEvent::Changed(SessionView {
                phase: PhaseView::Approval {
                    phone: "Pixel 8".into(),
                    code: "4821".into(),
                },
                phones: vec![],
            })));
        assert!(
            matches!(&r.ui.read().connection, Connection::Approval { code, .. } if code == "4821")
        );
        let mut values = std::collections::BTreeMap::new();
        values.insert("camera.framing".to_owned(), "fill".to_owned());
        r.hub
            .handle(AppMsg::Settings(SettingsEvent::View(SettingsView {
                values,
                paired: true,
            })));
        r.hub.handle(AppMsg::Ui(Action::Cycle("camera.framing")));
        assert!(
            matches!(r.settings.try_recv(), Some(SettingsMsg::Set { value, .. }) if value == "fit")
        );
        r.hub.handle(AppMsg::Ui(Action::Allow));
        assert!(matches!(
            r.session.try_recv(),
            Some(SessionMsg::UserDecision { allow: true })
        ));
    }

    #[test]
    fn next_values_wrap_around() {
        assert_eq!(next_value("camera.frameRate", "60").as_deref(), Some("24"));
        assert_eq!(
            next_value("camera.frameRate", "bogus").as_deref(),
            Some("24")
        );
        assert_eq!(next_value("nope", "x"), None);
    }

    #[test]
    fn a_rebuilt_device_hub_hears_again_what_was_on() {
        let mut r = rig();
        r.hub.handle(AppMsg::Media(MediaEvent::Mic(true)));
        r.hub.handle(AppMsg::Media(MediaEvent::Speaker(true)));
        r.hub.handle(AppMsg::Media(MediaEvent::Speaker(false)));
        r.devices.lock().unwrap().clear();
        r.hub.handle(AppMsg::DevicesStarted);
        assert_eq!(
            r.devices.lock().unwrap().as_slice(),
            &[DeviceCommand::Mic(true), DeviceCommand::Speaker(false)]
        );
    }

    #[test]
    fn a_phone_shaking_hands_shows_as_connecting() {
        let mut r = rig();
        r.hub.handle(AppMsg::Link(LinkEvent::View(LinkView {
            connecting: Some(owlmic_link::LINK_WIFI),
            ..LinkView::default()
        })));
        assert_eq!(
            r.ui.read().connection,
            Connection::Connecting {
                link: owlmic_link::LINK_WIFI
            }
        );
    }

    #[test]
    fn reconnecting_shows_the_link_that_carried_the_session() {
        let mut r = rig();
        r.hub.handle(AppMsg::Link(LinkEvent::Proven {
            conn: 2,
            phone: Phone {
                id: [1; 16],
                name: "A".into(),
                model: "A".into(),
                static_pub: String::new(),
            },
            resume: None,
            code: "0000".into(),
            link: owlmic_link::LINK_BLUETOOTH,
        }));
        r.hub.handle(AppMsg::Link(LinkEvent::View(LinkView {
            link: Some(owlmic_link::LINK_USB_DEBUGGING),
            ..LinkView::default()
        })));
        r.hub
            .handle(AppMsg::Link(LinkEvent::View(LinkView::default())));
        r.hub
            .handle(AppMsg::Session(SessionEvent::Changed(SessionView {
                phase: PhaseView::Held { phone: "A".into() },
                phones: vec![],
            })));
        assert!(matches!(
            r.ui.read().connection,
            Connection::Reconnecting { link, .. } if link == owlmic_link::LINK_USB_DEBUGGING
        ));
    }

    #[test]
    fn a_failed_unit_asks_for_a_repair_and_a_long_restart_does_too() {
        let mut r = rig();
        r.hub.handle(AppMsg::Unit {
            name: "listener",
            health: Health::Failed("port".into()),
        });
        assert!(r.ui.read().repair.net);
        r.hub.handle(AppMsg::Unit {
            name: "listener",
            health: Health::Ok,
        });
        assert!(!r.ui.read().repair.net);
        r.hub.handle(AppMsg::Unit {
            name: "devices",
            health: Health::Degraded("restarting".into()),
        });
        assert!(!r.ui.read().repair.mic, "a short restart is fine");
        let due = r.hub.next_deadline().unwrap();
        r.hub.tick(due);
        assert!(r.ui.read().repair.mic);
        assert_eq!(r.hub.next_deadline(), None, "nothing more to wait for");
        r.hub.handle(AppMsg::Unit {
            name: "bluetooth",
            health: Health::Failed("no radio".into()),
        });
        assert!(!r.ui.read().repair.net, "Bluetooth is optional");
    }

    #[test]
    fn approval_and_busy_answers_reach_the_link() {
        let mut r = rig();
        r.hub
            .handle(AppMsg::Session(SessionEvent::Approval { conn: 4 }));
        assert!(matches!(
            r.link.try_recv(),
            Some(LinkMsg::Pending { conn: 4 })
        ));
        r.hub.handle(AppMsg::Session(SessionEvent::Rejected {
            conn: 5,
            reason: owlmic_proto::messages::RejectReason::Busy,
            owner: Some("Pixel 8".into()),
        }));
        assert!(
            matches!(r.link.try_recv(), Some(LinkMsg::Reject { conn: 5, owner: Some(o), .. }) if o == "Pixel 8")
        );
    }

    #[test]
    fn views_may_be_dropped_but_lifecycle_messages_never() {
        assert!(AppMsg::Link(LinkEvent::View(LinkView::default())).droppable());
        assert!(AppMsg::Settings(SettingsEvent::View(SettingsView::default())).droppable());
        assert!(!AppMsg::Link(LinkEvent::ConnClosed { conn: 1 }).droppable());
        assert!(
            !AppMsg::Session(SessionEvent::Ended {
                session_id: [0; 16]
            })
            .droppable()
        );
        assert!(!AppMsg::Ui(Action::Allow).droppable());
    }
}
