//! The whole PC side over loopback: a simulated phone connects through the real server, carrier
//! and Link Hub threads, while the test plays the App and Session Hubs.

use crate::handshake::phone::SimPhone;
use crate::hub::{LinkEvent, LinkHub, LinkMsg};
use crate::media::{MediaSink, Routes, carrier_hello_packet};
use crate::server::{self, Shared};
use crate::wire::{Item, Reader, Writer};
use owlmic_hub::Outbox;
use owlmic_proto::crypto::{Key, KeyPair};
use owlmic_proto::frame::{CHANNEL_CONTROL, CHANNEL_MEDIA, MediaHeader, kind, stream};
use owlmic_proto::messages::{FeatureState, Message, State};
use owlmic_session::{Decision, Identity};
use std::collections::BTreeMap;
use std::io::Write;
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

struct Collect(Mutex<Vec<Vec<u8>>>);
impl MediaSink for Collect {
    fn deliver(&self, _h: &MediaHeader, p: &[u8]) {
        self.0.lock().unwrap().push(p.to_vec());
    }
}

struct Pc {
    routes: Arc<Routes>,
    mic: Arc<Collect>,
    port: u16,
    to_hub: Outbox<LinkMsg>,
    events: mpsc::Receiver<LinkEvent>,
}

fn pc() -> Pc {
    let mic = Arc::new(Collect(Mutex::new(Vec::new())));
    let routes = Arc::new(Routes::new(Some(mic.clone()), None));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (to_hub, inbox) = owlmic_hub::mailbox(64);
    let shared = Arc::new(Shared::new(
        routes.clone(),
        to_hub.clone(),
        Arc::new(crate::netinfo::Simple),
    ));
    std::thread::spawn(move || server::serve(listener, shared));
    let (tx, events) = mpsc::channel();
    let ours = Arc::new(Identity {
        keys: KeyPair::generate(),
        pc_id: [9; 16],
        name: "DESKTOP-A".into(),
    });
    let r = routes.clone();
    let tx = Mutex::new(tx);
    owlmic_hub::spawn_supervised(
        "link",
        move || {
            let tx = tx.lock().unwrap().clone();
            LinkHub::new(
                ours.clone(),
                r.clone(),
                Some(Arc::new(UdpSocket::bind("127.0.0.1:0").unwrap())),
                Arc::new(AtomicBool::new(false)),
                move |e| {
                    let _ = tx.send(e);
                },
            )
        },
        inbox,
        |_| {},
    );
    Pc {
        routes,
        mic,
        port,
        to_hub,
        events,
    }
}

impl Pc {
    fn next(&self, want: impl Fn(&LinkEvent) -> bool) -> LinkEvent {
        loop {
            let e = self
                .events
                .recv_timeout(Duration::from_secs(3))
                .expect("an event from the Link Hub");
            if want(&e) {
                return e;
            }
        }
    }

    fn open(&self, channel: u8) -> (Writer<TcpStream>, Reader<TcpStream>) {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        stream.write_all(&[channel]).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        (
            Writer::new(stream.try_clone().unwrap()),
            Reader::new(stream, Arc::new(Mutex::new(None))),
        )
    }

    /// A control connection over USB debugging taken to WELCOME, the test playing the Session
    /// Hub. Returns the connection and its link's auth key.
    fn link(
        &self,
        phone: &mut SimPhone,
        decision: Decision,
        session_id: [u8; 16],
    ) -> (Writer<TcpStream>, Reader<TcpStream>, Key) {
        let (mut w, mut r) = self.open(CHANNEL_CONTROL);
        let resume = (decision == Decision::Resume).then_some(session_id);
        w.send_raw(kind::HELLO, &phone.hello(crate::LINK_USB_DEBUGGING, resume))
            .unwrap();
        let LinkEvent::Hello { conn, .. } = self.next(|e| matches!(e, LinkEvent::Hello { .. }))
        else {
            unreachable!()
        };
        self.to_hub.send(LinkMsg::Decided { conn, decision });
        let (k, ack) = control(&mut r);
        assert_eq!(k, kind::HELLO_ACK);
        w.send_raw(kind::PROOF, &phone.proof(&ack)).unwrap();
        let LinkEvent::Proven { link, .. } = self.next(|e| matches!(e, LinkEvent::Proven { .. }))
        else {
            unreachable!()
        };
        assert_eq!(link, crate::LINK_USB_DEBUGGING);
        self.to_hub.send(LinkMsg::Welcome {
            conn,
            session_id,
            settings: BTreeMap::new(),
        });
        let (k, welcome) = control(&mut r);
        assert_eq!(k, kind::WELCOME);
        let Ok(Some(Message::Welcome(wm))) = Message::from_payload(k, &welcome) else {
            panic!()
        };
        assert!(phone.welcome_is_genuine(&wm));
        let Some(Message::Settings(s)) = message(&mut r) else {
            panic!("SETTINGS right after WELCOME")
        };
        assert!(
            s.changes
                .iter()
                .any(|c| c.id == "camera.lens" && c.version == 0),
            "every shared setting with its version"
        );
        let auth = phone.session.as_ref().unwrap().auth;
        (w, r, auth)
    }
}

fn control(r: &mut Reader<TcpStream>) -> (u8, Vec<u8>) {
    match r.read_item().unwrap() {
        Item::Control { kind, payload } => (kind, payload),
        Item::Media(_) => panic!("expected a control frame"),
    }
}

fn message(r: &mut Reader<TcpStream>) -> Option<Message> {
    let (k, p) = control(r);
    Message::from_payload(k, &p).ok().flatten()
}

fn mic_packet(seq: u32, data: &[u8]) -> Vec<u8> {
    [
        MediaHeader {
            stream: stream::MIC,
            keyframe: false,
            seq,
            timestamp_us: 0,
        }
        .encode()
        .as_slice(),
        data,
    ]
    .concat()
}

fn wait_for(f: impl Fn() -> bool) {
    for _ in 0..150 {
        if f() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_phone_connects_streams_its_mic_and_hears_the_speaker() {
    let pc = pc();
    let mut phone = SimPhone::new(1);
    let (mut w, mut r, auth) = pc.link(&mut phone, Decision::Known, [4; 16]);
    assert!(matches!(
        pc.next(|e| matches!(e, LinkEvent::LinkUp { .. })),
        LinkEvent::LinkUp { link: 1, .. }
    ));
    assert!(matches!(message(&mut r), Some(Message::Switch(_))));

    w.send(&Message::State(State {
        mic: FeatureState::On,
        camera: FeatureState::Off,
        speaker: FeatureState::On,
    }))
    .unwrap();
    pc.next(|e| matches!(e, LinkEvent::FromPhone(Message::State(_))));

    // Media on a TCP control connection is never taken: anything on the phone could send it.
    w.media(&mic_packet(1, b"injected")).unwrap();
    let (mut mw, mut mr) = pc.open(CHANNEL_MEDIA);
    mw.media(&carrier_hello_packet([4; 16], &auth)).unwrap();
    mw.media(&mic_packet(2, b"pcm")).unwrap();
    wait_for(|| !pc.mic.0.lock().unwrap().is_empty());
    assert_eq!(*pc.mic.0.lock().unwrap(), vec![b"pcm".to_vec()]);

    // Once the hub has passed this, it has the carrier too.
    pc.to_hub.send(LinkMsg::Send(Message::KeyframeRequest));
    assert!(matches!(message(&mut r), Some(Message::KeyframeRequest)));
    pc.routes.send_speaker(10_000, b"speaker");
    let Item::Media(back) = mr.read_item().unwrap() else {
        panic!("expected speaker media")
    };
    let h = MediaHeader::decode(&back).unwrap();
    assert_eq!(h.stream, stream::SPEAKER);
    assert_eq!(&back[MediaHeader::LEN..], b"speaker");
}

#[test]
fn a_standby_links_carrier_is_proven_with_its_own_keys_and_media_moves_there() {
    let pc = pc();
    let mut phone = SimPhone::new(1);
    let (w1, mut r1, auth1) = pc.link(&mut phone, Decision::Known, [5; 16]);
    assert!(matches!(message(&mut r1), Some(Message::Switch(_))));
    let (mut mw1, mut mr1) = pc.open(CHANNEL_MEDIA);
    mw1.media(&carrier_hello_packet([5; 16], &auth1)).unwrap();

    let (_w2, mut r2, auth2) = pc.link(&mut phone, Decision::Resume, [5; 16]);
    assert_ne!(auth1, auth2, "each link has its own keys");
    let (mut mw2, mut mr2) = pc.open(CHANNEL_MEDIA);
    mw2.media(&carrier_hello_packet([5; 16], &auth2)).unwrap();
    mw2.media(&mic_packet(1, b"standby")).unwrap();
    wait_for(|| !pc.mic.0.lock().unwrap().is_empty());
    assert_eq!(
        *pc.mic.0.lock().unwrap(),
        vec![b"standby".to_vec()],
        "the standby link's carrier was accepted"
    );

    drop(r1);
    drop(w1);
    assert!(
        matches!(message(&mut r2), Some(Message::Switch(_))),
        "SWITCH goes on the link media moves to"
    );
    assert!(mr1.read_item().is_err(), "the old link's channel closed");
    pc.next(|e| matches!(e, LinkEvent::LinkUp { .. }));
    pc.routes.send_speaker(0, b"here");
    let Item::Media(back) = mr2.read_item().unwrap() else {
        panic!("expected speaker media")
    };
    assert_eq!(&back[MediaHeader::LEN..], b"here");
}

#[test]
fn a_busy_pc_turns_a_second_phone_away_with_the_owners_name() {
    let pc = pc();
    let mut phone = SimPhone::new(2);
    let (mut w, mut r) = pc.open(CHANNEL_CONTROL);
    w.send_raw(kind::HELLO, &phone.hello(crate::LINK_USB_DEBUGGING, None))
        .unwrap();
    let LinkEvent::Hello { conn, .. } = pc.next(|e| matches!(e, LinkEvent::Hello { .. })) else {
        unreachable!()
    };
    pc.to_hub.send(LinkMsg::Decided {
        conn,
        decision: Decision::Busy {
            owner: "Pixel 8".into(),
        },
    });
    assert_eq!(control(&mut r).0, kind::HELLO_ACK);
    let (k, reject) = control(&mut r);
    assert_eq!(k, kind::REJECT);
    assert_eq!(
        String::from_utf8(reject).unwrap(),
        r#"{"reason":"busy","owner":"Pixel 8"}"#
    );
}

#[test]
fn an_old_phone_hears_which_version_this_pc_speaks() {
    let pc = pc();
    let mut phone = SimPhone::new(3);
    let mut hello: serde_json::Value =
        serde_json::from_slice(&phone.hello(crate::LINK_USB_DEBUGGING, None)).unwrap();
    hello["proto"] = 2.into();
    let (mut w, mut r) = pc.open(CHANNEL_CONTROL);
    w.send_raw(kind::HELLO, &serde_json::to_vec(&hello).unwrap())
        .unwrap();
    let (k, reject) = control(&mut r);
    assert_eq!(k, kind::REJECT);
    assert_eq!(
        String::from_utf8(reject).unwrap(),
        format!(
            r#"{{"reason":"version","proto":{}}}"#,
            owlmic_proto::VERSION
        )
    );
}
