//! The Link Hub's thread (SYSTEM_DESIGN section 14): it drives each connection's handshake,
//! decides which link carries the session, keeps heartbeats, reports and the recovery ladder. It
//! asks the Session Hub about phones only through `emit` and the App Hub.

use crate::handshake::{Ours, ServerHandshake};
use crate::media::{Path, Routes};
use crate::monitor::{HEARTBEAT, Monitor, REPORT_EVERY, StatsMark, report};
use crate::ratelimit::HandshakeLimit;
use crate::reconnect::{Reconnector, STALL, Step};
use crate::switcher::Switcher;
use crate::wire::{CipherSlot, Outgoing};
use owlmic_proto::crypto::Cipher;
use owlmic_proto::frame::{kind, stream};
use owlmic_proto::messages::{
    Bye, FeatureState, Message, Pending, Ping, Reject, RejectReason, Settings, StreamRef, Switch,
};
use owlmic_session::sessions::Phone;
use owlmic_session::{Decision, Identity};
use owlmic_settings::store::Versioned;
use std::collections::{BTreeMap, HashMap};
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A handshake step that takes longer than this is abandoned (protocol/README.md, section 4).
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(3);
/// How long a new phone may wait for Allow or Deny, with nothing on the wire.
const APPROVAL_WAIT: Duration = Duration::from_secs(120);
/// A stream that just became expected, or just moved links, gets this long to start before the
/// recovery ladder watches it.
const START_GRACE: Duration = Duration::from_secs(2);
/// A link this lossy or slow counts as weak (the yellow indicator).
const WEAK_LOSS_PCT: f64 = 5.0;
const WEAK_RTT_MS: u32 = 250;

/// The PC's Bluetooth address while its RFCOMM server runs, for HELLO_ACK.
pub type BtAddr = Arc<Mutex<Option<String>>>;

pub struct Opened {
    pub conn: u64,
    pub link: u8,
    pub peer: SocketAddr,
    pub out: Outgoing,
    pub rx: CipherSlot,
}

/// A media carrier a carrier hello proved.
pub enum Carrier {
    /// The phone's datagram address on Wi-Fi or USB tethering.
    Udp(SocketAddr),
    /// The media channel through `adb reverse`.
    Channel { id: u64, out: Outgoing },
}

pub enum LinkMsg {
    Opened(Opened),
    Frame {
        conn: u64,
        kind: u8,
        payload: Vec<u8>,
    },
    Closed {
        conn: u64,
    },
    /// A carrier for live connection `conn`.
    CarrierUp {
        conn: u64,
        session_id: [u8; 16],
        carrier: Carrier,
    },
    CarrierDown {
        channel: u64,
    },
    /// The Session Hub's answer to [`LinkEvent::Hello`].
    Decided {
        conn: u64,
        decision: Decision,
    },
    /// A new phone waits for the user: send it PENDING with the code.
    Pending {
        conn: u64,
    },
    Welcome {
        conn: u64,
        session_id: [u8; 16],
        settings: BTreeMap<String, Versioned>,
    },
    Reject {
        conn: u64,
        reason: RejectReason,
        owner: Option<String>,
    },
    SessionEnded {
        session_id: [u8; 16],
    },
    /// A message for the phone on the session's active link.
    Send(Message),
}

impl LinkMsg {
    /// What a full mailbox may drop: a keyframe request the receiver will repeat.
    pub fn droppable(&self) -> bool {
        matches!(self, LinkMsg::Send(Message::KeyframeRequest))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LinkEvent {
    Hello {
        conn: u64,
        phone: Phone,
        resume: Option<[u8; 16]>,
    },
    Proven {
        conn: u64,
        phone: Phone,
        resume: Option<[u8; 16]>,
        code: String,
        link: u8,
    },
    ConnClosed {
        conn: u64,
    },
    LinkUp {
        session_id: [u8; 16],
        link: u8,
    },
    LinkLost {
        session_id: [u8; 16],
    },
    Bye {
        session_id: [u8; 16],
    },
    /// A message from the phone for the other hubs: STATE, SETTINGS, REPORT, STREAM_*.
    FromPhone(Message),
    /// A stream that should flow has stopped; `tell_user` after 5 seconds without recovery.
    Stalled {
        stream: u8,
        tell_user: bool,
    },
    Recovered {
        stream: u8,
    },
    View(LinkView),
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LinkView {
    /// The link carrying the session, if one does.
    pub link: Option<u8>,
    pub weak: bool,
    pub rtt_ms: u32,
    /// With no session: the best link a phone is shaking hands on.
    pub connecting: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Stage {
    AwaitHello,
    Deciding,
    AwaitProof,
    /// Proven; the Session Hub says Welcome, Pending or Reject.
    AwaitWelcome,
    Approving,
    Live,
}

struct Conn {
    link: u8,
    peer: SocketAddr,
    out: Outgoing,
    rx: CipherSlot,
    stage: Stage,
    since: Instant,
    hs: Option<ServerHandshake>,
    phone: Option<Phone>,
    resume: Option<[u8; 16]>,
    code: String,
    monitor: Monitor,
    carrier: Option<Carrier>,
}

impl Conn {
    /// Closes the connection and its media channel at once.
    fn close(&self) {
        if let Some(Carrier::Channel { out, .. }) = &self.carrier {
            out.close();
        }
        self.out.close();
    }
}

pub struct LinkHub {
    ours: Arc<Identity>,
    bt_addr: BtAddr,
    routes: Arc<Routes>,
    udp: Option<Arc<UdpSocket>>,
    busy: Arc<AtomicBool>,
    emit: Box<dyn Fn(LinkEvent) + Send>,
    conns: HashMap<u64, Conn>,
    session: Option<[u8; 16]>,
    switcher: Switcher,
    limit: HandshakeLimit,
    /// When each stream the phone has on started being watched; `None` while it is off.
    flow_since: [Option<Instant>; 4],
    stalled: [bool; 4],
    reconnect: [Reconnector; 4],
    marks: BTreeMap<u8, StatsMark>,
    last_report: Instant,
    next_tick: Option<Instant>,
    view: LinkView,
}

impl LinkHub {
    /// A new Link Hub owns no connection: whatever the routes held belongs to a hub before it.
    pub fn new(
        ours: Arc<Identity>,
        routes: Arc<Routes>,
        udp: Option<Arc<UdpSocket>>,
        busy: Arc<AtomicBool>,
        emit: impl Fn(LinkEvent) + Send + 'static,
    ) -> Self {
        routes.clear();
        busy.store(false, Ordering::Relaxed);
        Self {
            ours,
            bt_addr: BtAddr::default(),
            routes,
            udp,
            busy,
            emit: Box::new(emit),
            conns: HashMap::new(),
            session: None,
            switcher: Switcher::default(),
            limit: HandshakeLimit::default(),
            flow_since: [None; 4],
            stalled: [false; 4],
            reconnect: Default::default(),
            marks: BTreeMap::new(),
            last_report: Instant::now(),
            next_tick: None,
            view: LinkView::default(),
        }
    }

    /// The PC's Bluetooth address, sent in HELLO_ACK so the phone can find it over Bluetooth later.
    pub fn with_bt_addr(mut self, addr: BtAddr) -> Self {
        self.bt_addr = addr;
        self
    }

    fn reject(
        &mut self,
        conn: u64,
        reason: RejectReason,
        owner: Option<String>,
        proto: Option<u8>,
    ) {
        if let Some(c) = self.conns.get(&conn) {
            c.out.send(&Message::Reject(Reject {
                reason,
                owner,
                proto,
            }));
            c.out.close_when_sent();
        }
    }

    /// Closes `conn` now and forgets it.
    fn drop_conn(&mut self, conn: u64) {
        if let Some(c) = self.conns.get(&conn) {
            c.close();
        }
        self.on_closed(conn);
    }

    fn on_opened(&mut self, o: Opened) {
        let now = Instant::now();
        if !self.limit.allow(o.peer.ip(), now) {
            o.out.close();
            return;
        }
        let conn = Conn {
            link: o.link,
            peer: o.peer,
            out: o.out,
            rx: o.rx,
            stage: Stage::AwaitHello,
            since: now,
            hs: None,
            phone: None,
            resume: None,
            code: String::new(),
            monitor: Monitor::new(o.link, now),
            carrier: None,
        };
        self.conns.insert(o.conn, conn);
        self.schedule(now + HANDSHAKE_TIMEOUT);
    }

    fn on_frame(&mut self, conn_id: u64, kind: u8, payload: Vec<u8>) {
        let now = Instant::now();
        let Some(c) = self.conns.get_mut(&conn_id) else {
            return;
        };
        c.monitor.heard(now);
        match (c.stage, kind) {
            (Stage::AwaitHello, kind::HELLO) => match ServerHandshake::hello(&payload) {
                Ok((hs, hello, phone)) => {
                    let resume = hello
                        .resume
                        .as_deref()
                        .and_then(owlmic_session::id_from_hex);
                    c.hs = Some(hs);
                    c.phone = Some(phone.clone());
                    c.resume = resume;
                    c.stage = Stage::Deciding;
                    c.since = now;
                    (self.emit)(LinkEvent::Hello {
                        conn: conn_id,
                        phone,
                        resume,
                    });
                }
                Err(crate::handshake::HandshakeError::Version) => self.reject(
                    conn_id,
                    RejectReason::Version,
                    None,
                    Some(owlmic_proto::VERSION),
                ),
                Err(_) => self.drop_conn(conn_id),
            },
            (Stage::AwaitProof, kind::PROOF) => {
                let Some(hs) = c.hs.as_ref().filter(|hs| hs.proof(&payload).is_ok()) else {
                    return self.drop_conn(conn_id);
                };
                let Some(phone) = c.phone.clone() else { return };
                c.code = hs.code();
                c.stage = Stage::AwaitWelcome;
                c.since = now;
                (self.emit)(LinkEvent::Proven {
                    conn: conn_id,
                    phone,
                    resume: c.resume,
                    code: c.code.clone(),
                    link: c.link,
                });
            }
            // The phone sends exactly one frame in these stages; anything else is not a phone.
            (Stage::AwaitHello | Stage::AwaitProof, _) => self.drop_conn(conn_id),
            (Stage::Live, _) => self.on_live_frame(conn_id, kind, &payload, now),
            _ => {}
        }
    }

    fn on_live_frame(&mut self, conn_id: u64, kind: u8, payload: &[u8], now: Instant) {
        let Ok(Some(msg)) = Message::from_payload(kind, payload) else {
            return;
        };
        let Some(c) = self.conns.get_mut(&conn_id) else {
            return;
        };
        match msg {
            Message::Ping(p) => c.out.send(&Message::Pong(p)),
            Message::Pong(p) => c.monitor.pong(p.t, self.routes.now_us(), now),
            Message::Bye(_) => {
                if let Some(session_id) = self.session {
                    (self.emit)(LinkEvent::Bye { session_id });
                    self.end_session();
                }
            }
            Message::State(s) => {
                for (st, f) in [(stream::MIC, s.mic), (stream::CAMERA, s.camera)] {
                    let watched = &mut self.flow_since[st as usize];
                    if f != FeatureState::On {
                        *watched = None;
                    } else if watched.is_none() {
                        *watched = Some(now);
                    }
                }
                (self.emit)(LinkEvent::FromPhone(Message::State(s)));
            }
            Message::Report(r) => {
                self.view.weak = r.loss_pct >= WEAK_LOSS_PCT || r.rtt_ms >= WEAK_RTT_MS;
                self.publish_view();
                (self.emit)(LinkEvent::FromPhone(Message::Report(r)));
            }
            m @ (Message::Settings(_)
            | Message::StreamStart(_)
            | Message::StreamStop(_)
            | Message::RestartStream(_)) => (self.emit)(LinkEvent::FromPhone(m)),
            _ => {}
        }
    }

    fn on_decided(&mut self, conn_id: u64, decision: Decision) {
        let ours = self.ours.clone();
        let o = Ours {
            keys: &ours.keys,
            pc_id: ours.pc_id,
            name: &ours.name,
            bt_addr: self
                .bt_addr
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .clone(),
        };
        let Some(c) = self.conns.get_mut(&conn_id) else {
            return;
        };
        if c.stage != Stage::Deciding {
            return;
        }
        let Some(ack) = c.hs.as_mut().and_then(|hs| hs.ack(&o, &decision).ok()) else {
            return self.drop_conn(conn_id);
        };
        c.out.send_raw(kind::HELLO_ACK, ack);
        match decision {
            Decision::Busy { owner } => self.reject(conn_id, RejectReason::Busy, Some(owner), None),
            Decision::Blocked => self.reject(conn_id, RejectReason::Blocked, None, None),
            _ => {
                c.stage = Stage::AwaitProof;
                c.since = Instant::now();
            }
        }
    }

    fn on_pending(&mut self, conn_id: u64) {
        if let Some(c) = self.conns.get_mut(&conn_id)
            && c.stage == Stage::AwaitWelcome
        {
            c.out.send(&Message::Pending(Pending {
                code: c.code.clone(),
            }));
            c.stage = Stage::Approving;
            c.since = Instant::now();
        }
    }

    fn on_welcome(
        &mut self,
        conn_id: u64,
        session_id: [u8; 16],
        settings: &BTreeMap<String, Versioned>,
    ) {
        if self.session.is_some_and(|s| s != session_id) {
            self.end_session();
        }
        let now = Instant::now();
        let Some(c) = self.conns.get_mut(&conn_id) else {
            return;
        };
        let Some(hs) = c.hs.take() else { return };
        let welcome = hs.welcome(session_id, settings);
        let Some(keys) = hs.into_keys() else { return };
        let wireless = crate::is_wireless(c.link);
        // Decryption must be on before WELCOME leaves: the phone encrypts its very next frame.
        if wireless {
            *c.rx.lock().unwrap_or_else(|p| p.into_inner()) = Some(Cipher::new(&keys.phone_to_pc));
        }
        c.out.send(&welcome);
        if wireless {
            c.out.encrypt_with(Cipher::new(&keys.pc_to_phone));
        }
        // Both sides start from the same versions (protocol/README.md, section 4).
        c.out.send(&Message::Settings(Settings {
            changes: owlmic_settings::book::shared_snapshot(settings),
        }));
        let datagrams = matches!(c.link, crate::LINK_WIFI | crate::LINK_USB_TETHERING);
        self.routes.add_link(
            session_id,
            conn_id,
            &keys,
            wireless,
            datagrams.then(|| c.peer.ip()),
        );
        c.stage = Stage::Live;
        c.monitor = Monitor::new(c.link, now);
        let link = c.link;
        self.session = Some(session_id);
        self.busy.store(true, Ordering::Relaxed);
        self.switcher.add(conn_id, link, now);
        if self.switcher.active() == Some(conn_id) {
            self.activate(conn_id);
        }
        self.schedule(now + HEARTBEAT);
    }

    /// Media moves to `conn`: the phone is told with SWITCH on that link, and the speaker goes out
    /// on its carrier.
    fn activate(&mut self, conn_id: u64) {
        let (Some(session_id), Some(c)) = (self.session, self.conns.get(&conn_id)) else {
            return;
        };
        c.out.send(&Message::Switch(Switch { link: c.link }));
        let link = c.link;
        let now = Instant::now();
        for (i, since) in self.flow_since.iter_mut().enumerate() {
            if since.is_some() {
                *since = Some(now);
            }
            self.routes.stats[i].new_sequence();
        }
        self.update_out();
        self.view.link = Some(link);
        self.publish_view();
        (self.emit)(LinkEvent::LinkUp { session_id, link });
    }

    fn update_out(&self) {
        let out = self.switcher.active().and_then(|a| {
            let c = self.conns.get(&a)?;
            let path = match (&c.carrier, c.link) {
                (_, crate::LINK_BLUETOOTH) => Path::Stream(c.out.clone()),
                (Some(Carrier::Channel { out, .. }), _) => Path::Stream(out.clone()),
                (Some(Carrier::Udp(to)), _) => Path::Udp {
                    socket: self.udp.clone()?,
                    to: *to,
                },
                (None, _) => return None,
            };
            Some((a, path))
        });
        self.routes.set_out(out);
    }

    fn on_carrier(&mut self, conn_id: u64, session_id: [u8; 16], carrier: Carrier) {
        let live = self
            .conns
            .get(&conn_id)
            .is_some_and(|c| c.stage == Stage::Live);
        if self.session != Some(session_id) || !live {
            if let Carrier::Channel { out, .. } = carrier {
                out.close();
            }
            return;
        }
        let Some(c) = self.conns.get_mut(&conn_id) else {
            return;
        };
        // A new carrier hello replaces the connection's carrier.
        if let Some(Carrier::Channel { out, .. }) = c.carrier.replace(carrier) {
            out.close();
        }
        if self.switcher.active() == Some(conn_id) {
            self.update_out();
        }
    }

    fn on_carrier_down(&mut self, channel: u64) {
        let Some((&conn, c)) = self.conns.iter_mut().find(
            |(_, c)| matches!(&c.carrier, Some(Carrier::Channel { id, .. }) if *id == channel),
        ) else {
            return;
        };
        c.carrier = None;
        if self.switcher.active() == Some(conn) {
            self.update_out();
        }
    }

    fn on_closed(&mut self, conn_id: u64) {
        let Some(c) = self.conns.remove(&conn_id) else {
            return;
        };
        c.close();
        self.routes.remove_link(conn_id);
        if c.stage < Stage::Live {
            (self.emit)(LinkEvent::ConnClosed { conn: conn_id });
            return;
        }
        let now = Instant::now();
        match self.switcher.remove(conn_id, now) {
            Some(next) => self.activate(next),
            None if self.switcher.active().is_none() => {
                self.routes.set_out(None);
                self.view.link = None;
                self.publish_view();
                if let Some(session_id) = self.session {
                    (self.emit)(LinkEvent::LinkLost { session_id });
                }
            }
            None => {}
        }
    }

    fn end_session(&mut self) {
        let live: Vec<u64> = self
            .conns
            .iter()
            .filter(|(_, c)| c.stage == Stage::Live)
            .map(|(id, _)| *id)
            .collect();
        for id in live {
            if let Some(c) = self.conns.remove(&id) {
                // A BYE queued just before still goes out.
                c.out.close_when_sent();
                if let Some(Carrier::Channel { out, .. }) = &c.carrier {
                    out.close();
                }
            }
        }
        self.session = None;
        self.switcher.clear();
        self.routes.clear();
        self.flow_since = [None; 4];
        self.stalled = [false; 4];
        self.reconnect = Default::default();
        self.marks.clear();
        self.busy.store(false, Ordering::Relaxed);
        self.view = LinkView::default();
        self.publish_view();
    }

    /// Every link shakes hands again, for new keys, before the speaker's `seq` would wrap. The
    /// phone comes back on each with `resume`.
    fn rekey(&mut self) {
        let live: Vec<u64> = self
            .conns
            .iter()
            .filter(|(_, c)| c.stage == Stage::Live)
            .map(|(id, _)| *id)
            .collect();
        for id in live {
            self.drop_conn(id);
        }
        self.routes.restart_speaker_seq();
    }

    fn publish_view(&self) {
        (self.emit)(LinkEvent::View(self.view));
    }

    fn update_connecting(&mut self) {
        let connecting = self
            .conns
            .values()
            .filter(|c| {
                self.session.is_none()
                    && matches!(
                        c.stage,
                        Stage::Deciding | Stage::AwaitProof | Stage::AwaitWelcome
                    )
            })
            .map(|c| c.link)
            .min();
        if connecting != self.view.connecting {
            self.view.connecting = connecting;
            self.publish_view();
        }
    }

    fn schedule(&mut self, at: Instant) {
        self.next_tick = Some(self.next_tick.map_or(at, |t| t.min(at)));
    }

    fn heartbeat(&mut self, now: Instant) {
        let ping = Message::Ping(Ping {
            t: self.routes.now_us(),
        });
        let mut dead = Vec::new();
        for (id, c) in &self.conns {
            let waited = now.saturating_duration_since(c.since);
            match c.stage {
                Stage::Live if c.monitor.is_dead(now) => dead.push(*id),
                Stage::Live => c.out.send(&ping),
                // The phone sends nothing while it waits for Allow (protocol/README.md, section 4).
                Stage::Approving if waited > APPROVAL_WAIT + HANDSHAKE_TIMEOUT => dead.push(*id),
                Stage::Approving => {}
                _ if waited > HANDSHAKE_TIMEOUT => dead.push(*id),
                _ => {}
            }
        }
        for id in dead {
            self.drop_conn(id);
        }
        if self.routes.speaker_seq_exhausted() {
            self.rekey();
        }
        if let Some(next) = self.switcher.upgrade(now) {
            self.activate(next);
        }
        if let Some(active) = self.switcher.active() {
            self.view.rtt_ms = self.conns.get(&active).map_or(0, |c| c.monitor.rtt_ms());
            if now.saturating_duration_since(self.last_report) >= REPORT_EVERY {
                let r = report(
                    &[
                        (stream::MIC, &self.routes.stats[1]),
                        (stream::CAMERA, &self.routes.stats[2]),
                    ],
                    &mut self.marks,
                    now.saturating_duration_since(self.last_report),
                    self.view.rtt_ms,
                );
                self.last_report = now;
                if let Some(c) = self.conns.get(&active) {
                    c.out.send(&Message::Report(r));
                }
            }
            self.check_streams(active, now);
        }
    }

    /// The recovery ladder for each stream the phone says is on.
    fn check_streams(&mut self, active: u64, now: Instant) {
        let now_us = self.routes.now_us();
        for s in [stream::MIC, stream::CAMERA] {
            let i = s as usize;
            let Some(since) = self.flow_since[i] else {
                self.reconnect[i].flowing();
                continue;
            };
            let last_us = self.routes.stats[i].last_packet_us.load(Ordering::Relaxed);
            let by_packet = (last_us != 0)
                .then(|| now.checked_sub(Duration::from_micros(now_us.saturating_sub(last_us))))
                .flatten();
            let last = by_packet
                .into_iter()
                .chain([since + START_GRACE])
                .max()
                .unwrap_or(now)
                .min(now);
            let age = now.duration_since(last);
            match self.reconnect[i].check(last, now) {
                Some(step) => self.take_step(active, s, step),
                None if age < STALL && self.stalled[i] => {
                    self.stalled[i] = false;
                    (self.emit)(LinkEvent::Recovered { stream: s });
                }
                None => {}
            }
            if age >= STALL {
                self.stalled[i] = true;
                let tell_user = self.reconnect[i].should_tell_user(now);
                (self.emit)(LinkEvent::Stalled {
                    stream: s,
                    tell_user,
                });
            }
        }
    }

    fn take_step(&mut self, active: u64, s: u8, step: Step) {
        match step {
            // Re-creating the channel is the phone's part; the old carrier stays until a new
            // carrier hello replaces it.
            Step::RestartSource | Step::RecreateChannel => {
                if let Some(c) = self.conns.get(&active) {
                    c.out.send(&Message::RestartStream(StreamRef { stream: s }));
                    if s == stream::CAMERA {
                        c.out.send(&Message::KeyframeRequest);
                    }
                }
            }
            Step::Rehandshake => self.drop_conn(active),
            Step::SwitchLink => {
                if self.switcher.standby().is_some() {
                    self.drop_conn(active);
                }
            }
            Step::Hold => {}
        }
    }
}

impl owlmic_hub::Hub for LinkHub {
    type Msg = LinkMsg;

    fn handle(&mut self, msg: LinkMsg) {
        match msg {
            LinkMsg::Opened(o) => self.on_opened(o),
            LinkMsg::Frame {
                conn,
                kind,
                payload,
            } => self.on_frame(conn, kind, payload),
            LinkMsg::Closed { conn } => self.on_closed(conn),
            LinkMsg::CarrierUp {
                conn,
                session_id,
                carrier,
            } => self.on_carrier(conn, session_id, carrier),
            LinkMsg::CarrierDown { channel } => self.on_carrier_down(channel),
            LinkMsg::Decided { conn, decision } => self.on_decided(conn, decision),
            LinkMsg::Pending { conn } => self.on_pending(conn),
            LinkMsg::Welcome {
                conn,
                session_id,
                settings,
            } => self.on_welcome(conn, session_id, &settings),
            LinkMsg::Reject {
                conn,
                reason,
                owner,
            } => self.reject(conn, reason, owner, None),
            LinkMsg::SessionEnded { session_id } => {
                if self.session == Some(session_id) {
                    if let Some(c) = self.switcher.active().and_then(|a| self.conns.get(&a)) {
                        c.out.send(&Message::Bye(Bye {
                            reason: Some("ended".into()),
                        }));
                    }
                    self.end_session();
                }
            }
            LinkMsg::Send(m) => {
                if let Some(c) = self.switcher.active().and_then(|a| self.conns.get(&a)) {
                    c.out.send(&m);
                }
            }
        }
        self.update_connecting();
    }

    fn tick(&mut self, now: Instant) {
        self.next_tick = None;
        self.heartbeat(now);
        self.update_connecting();
        if !self.conns.is_empty() {
            self.schedule(now + HEARTBEAT);
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.next_tick
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handshake::phone::SimPhone;
    use crate::wire::{Item, Reader};
    use owlmic_hub::Hub;
    use owlmic_proto::crypto::{Key, KeyPair};
    use owlmic_proto::messages::State;
    use std::io::Write;
    use std::sync::mpsc;

    /// What the PC wrote to one phone, and whether it closed the connection.
    #[derive(Clone, Default)]
    struct Wire(Arc<Mutex<Vec<u8>>>, Arc<AtomicBool>);

    impl Write for Wire {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Wire {
        fn closed(&self) -> bool {
            self.1.load(Ordering::SeqCst)
        }

        /// Every frame so far, waiting for at least `n`. Frames after WELCOME are opened with
        /// `keys` (the PC's direction), as the phone would.
        fn frames(&self, n: usize, keys: Option<Key>) -> Vec<Message> {
            for _ in 0..200 {
                let bytes = self.0.lock().unwrap().clone();
                let slot: CipherSlot = Arc::new(Mutex::new(None));
                let mut r = Reader::new(bytes.as_slice(), slot.clone());
                let mut out = Vec::new();
                while let Ok(Item::Control { kind, payload }) = r.read_item() {
                    if kind == kind::WELCOME
                        && let Some(k) = keys
                    {
                        *slot.lock().unwrap() = Some(Cipher::new(&k));
                    }
                    out.extend(Message::from_payload(kind, &payload).ok().flatten());
                }
                if out.len() >= n {
                    return out;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            panic!("fewer than {n} frames");
        }
    }

    struct Rig {
        hub: LinkHub,
        events: mpsc::Receiver<LinkEvent>,
        routes: Arc<Routes>,
    }

    fn rig() -> Rig {
        let (tx, events) = mpsc::channel();
        let routes = Arc::new(Routes::new(None, None));
        let ours = Arc::new(Identity {
            keys: KeyPair::generate(),
            pc_id: [9; 16],
            name: "DESKTOP-A".into(),
        });
        let hub = LinkHub::new(
            ours,
            routes.clone(),
            None,
            Arc::new(AtomicBool::new(false)),
            move |e| {
                let _ = tx.send(e);
            },
        );
        Rig {
            hub,
            events,
            routes,
        }
    }

    impl Rig {
        /// A Wi-Fi connection: wireless, so encrypted, and 3 missed heartbeats before it is dead.
        fn open(&mut self, conn: u64) -> Wire {
            let wire = Wire::default();
            let closed = wire.1.clone();
            let out = Outgoing::start(
                Box::new(wire.clone()),
                Arc::new(move || closed.store(true, Ordering::SeqCst)),
            );
            self.hub.handle(LinkMsg::Opened(Opened {
                conn,
                link: crate::LINK_WIFI,
                peer: "192.168.1.7:5000".parse().unwrap(),
                out,
                rx: Arc::new(Mutex::new(None)),
            }));
            wire
        }

        fn frame(&mut self, conn: u64, msg: &Message) {
            self.hub.handle(LinkMsg::Frame {
                conn,
                kind: msg.kind(),
                payload: msg.to_payload(),
            });
        }

        /// HELLO to PROOF, the test playing the Session Hub.
        fn prove(&mut self, conn: u64, wire: &Wire, phone: &mut SimPhone, decision: Decision) {
            self.hub.handle(LinkMsg::Frame {
                conn,
                kind: kind::HELLO,
                payload: phone.hello(crate::LINK_WIFI, None),
            });
            self.hub.handle(LinkMsg::Decided { conn, decision });
            let bytes = wire.0.clone();
            let ack = loop {
                let b = bytes.lock().unwrap().clone();
                if b.len() > 4 {
                    break b[4..].to_vec();
                }
                std::thread::sleep(Duration::from_millis(5));
            };
            self.hub.handle(LinkMsg::Frame {
                conn,
                kind: kind::PROOF,
                payload: phone.proof(&ack),
            });
        }

        fn live(&mut self, conn: u64, wire: &Wire, session_id: [u8; 16]) -> SimPhone {
            let mut phone = SimPhone::new(1);
            self.prove(conn, wire, &mut phone, Decision::Known);
            self.hub.handle(LinkMsg::Welcome {
                conn,
                session_id,
                settings: BTreeMap::new(),
            });
            phone
        }

        fn saw(&self, want: impl Fn(&LinkEvent) -> bool) -> bool {
            self.events.try_iter().any(|e| want(&e))
        }
    }

    #[test]
    fn a_phone_waiting_for_approval_is_kept_for_two_minutes() {
        let mut r = rig();
        let wire = r.open(1);
        let mut phone = SimPhone::new(1);
        r.prove(1, &wire, &mut phone, Decision::New);
        assert!(
            r.saw(|e| matches!(e, LinkEvent::View(v) if v.connecting == Some(crate::LINK_WIFI))),
            "the panel shows Connecting"
        );
        r.hub.handle(LinkMsg::Pending { conn: 1 });
        let frames = wire.frames(2, None);
        assert!(matches!(&frames[1], Message::Pending(p) if p.code == phone.code()));
        let t = Instant::now();
        r.hub.tick(t + Duration::from_secs(90));
        assert!(!wire.closed(), "the phone sends nothing while it waits");
        r.hub.tick(t + APPROVAL_WAIT + Duration::from_secs(10));
        assert!(wire.closed());
        assert!(r.saw(|e| matches!(e, LinkEvent::ConnClosed { conn: 1 })));
    }

    #[test]
    fn a_stream_gets_time_to_start_before_the_ladder_watches_it() {
        let mut r = rig();
        let wire = r.open(1);
        let phone = r.live(1, &wire, [1; 16]);
        let keys = phone.session.as_ref().unwrap().pc_to_phone;
        r.frame(
            1,
            &Message::State(State {
                mic: FeatureState::On,
                ..State::default()
            }),
        );
        let restarts = |wire: &Wire, n| {
            wire.frames(n, Some(keys))
                .iter()
                .filter(|m| matches!(m, Message::RestartStream(_)))
                .count()
        };
        let t = Instant::now();
        r.hub.tick(t + Duration::from_millis(1500));
        assert!(!r.saw(|e| matches!(e, LinkEvent::Stalled { .. })));
        // WELCOME, SETTINGS, SWITCH and a PING.
        assert_eq!(
            restarts(&wire, 4),
            0,
            "no packet yet, but it only just started"
        );
        r.hub.tick(t + Duration::from_millis(3200));
        assert!(r.saw(|e| matches!(e, LinkEvent::Stalled { stream: 1, .. })));
        assert_eq!(restarts(&wire, 6), 1, "now the first step");
        assert!(!wire.closed());
    }

    #[test]
    fn every_link_shakes_hands_again_before_the_speakers_seq_wraps() {
        let mut r = rig();
        let wire = r.open(1);
        r.live(1, &wire, [2; 16]);
        r.routes.set_speaker_seq(u32::MAX);
        r.hub.tick(Instant::now());
        assert!(wire.closed());
        assert!(r.saw(|e| matches!(e, LinkEvent::LinkLost { .. })));
        assert!(!r.routes.speaker_seq_exhausted());
    }

    #[test]
    fn a_new_session_ends_the_old_ones_links_and_switches_to_the_new_link() {
        let mut r = rig();
        let old = r.open(1);
        r.live(1, &old, [3; 16]);
        let new = r.open(2);
        let mut phone = SimPhone::new(1);
        r.prove(2, &new, &mut phone, Decision::Known);
        r.hub.handle(LinkMsg::Welcome {
            conn: 2,
            session_id: [4; 16],
            settings: BTreeMap::new(),
        });
        assert!(old.closed(), "the old session's link went");
        let keys = phone.session.as_ref().unwrap().pc_to_phone;
        assert!(
            new.frames(4, Some(keys))
                .iter()
                .any(|m| matches!(m, Message::Switch(_)))
        );
        r.hub.handle(LinkMsg::Send(Message::KeyframeRequest));
        assert!(
            new.frames(5, Some(keys))
                .iter()
                .any(|m| matches!(m, Message::KeyframeRequest)),
            "messages for the phone go on the new link"
        );
    }
}
