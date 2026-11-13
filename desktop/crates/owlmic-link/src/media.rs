//! The media path through the Transporter (SYSTEM_DESIGN section 10): carriers hand packets
//! straight to the pipelines and take the speaker's packets straight out. No hub sits in between;
//! the Link Hub only installs each connection's keys and picks the carrier.

use crate::monitor::StreamStats;
use crate::wire::Outgoing;
use owlmic_proto::crypto::{self, Cipher, Key, ReplayWindow, SessionKeys};
use owlmic_proto::frame::{MediaHeader, stream};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// The speaker's `seq` never wraps under one set of keys: at this value sending stops until the
/// Link Hub has every link shake hands again (protocol/README.md, section 6).
const SEQ_LIMIT: u32 = u32::MAX;

/// A pipeline that takes one stream's packets.
pub trait MediaSink: Send + Sync {
    fn deliver(&self, header: &MediaHeader, payload: &[u8]);
}

/// Where outgoing media goes: the active connection's carrier.
#[derive(Clone)]
pub enum Path {
    Udp {
        socket: Arc<UdpSocket>,
        to: SocketAddr,
    },
    Stream(Outgoing),
}

/// One live connection. Every link of a session shakes hands on its own, so each has its own
/// keys (protocol/README.md, section 4).
struct Link {
    auth: Key,
    rx: Cipher,
    tx: Key,
    encrypted: bool,
    /// The phone's address on Wi-Fi and USB tethering, where its datagrams come from.
    ip: Option<IpAddr>,
    replay: [ReplayWindow; 4],
}

struct Session {
    id: [u8; 16],
    links: HashMap<u64, Link>,
    /// Datagram senders a carrier hello tied to their connection.
    datagrams: HashMap<SocketAddr, u64>,
}

impl Session {
    /// The live connection whose own auth key made a carrier hello's MAC.
    fn hello(&self, payload: &[u8]) -> Option<u64> {
        let (id, mac) = payload.split_at_checked(16)?;
        if id != self.id {
            return None;
        }
        self.links
            .iter()
            .find(|(_, l)| mac == crypto::carrier_mac(&l.auth, &self.id))
            .map(|(conn, _)| *conn)
    }

    /// Decrypts and checks one packet from `conn` where it is. Returns its header and the
    /// payload's length.
    fn accept(&mut self, conn: u64, packet: &mut [u8]) -> Option<(MediaHeader, usize)> {
        let h = MediaHeader::decode(packet)?;
        if h.stream == stream::CARRIER_HELLO || h.stream > stream::SPEAKER {
            return None;
        }
        let link = self.links.get_mut(&conn)?;
        let len = if link.encrypted {
            link.rx
                .open_in_place(h.stream, h.seq as u64, packet, MediaHeader::LEN)?
        } else {
            packet.len() - MediaHeader::LEN
        };
        link.replay[h.stream as usize]
            .accept(h.seq)
            .then_some((h, len))
    }
}

struct Out {
    path: Path,
    seal: Option<Cipher>,
    packet: Vec<u8>,
}

pub struct Routes {
    session: Mutex<Option<Session>>,
    out: Mutex<Option<Out>>,
    mic: Option<Arc<dyn MediaSink>>,
    camera: Option<Arc<dyn MediaSink>>,
    pub stats: [StreamStats; 4],
    /// One counter across links, so no key ever sees a `seq` twice.
    speaker_seq: AtomicU32,
    clock: Instant,
}

impl Routes {
    pub fn new(mic: Option<Arc<dyn MediaSink>>, camera: Option<Arc<dyn MediaSink>>) -> Self {
        Self {
            session: Mutex::new(None),
            out: Mutex::new(None),
            mic,
            camera,
            stats: Default::default(),
            speaker_seq: AtomicU32::new(0),
            clock: Instant::now(),
        }
    }

    /// Microseconds since the routes were made: the PC's media clock.
    pub fn now_us(&self) -> u64 {
        self.clock.elapsed().as_micros() as u64
    }

    /// Connection `conn` finished its handshake for `session_id`. Another session's links go.
    pub fn add_link(
        &self,
        session_id: [u8; 16],
        conn: u64,
        keys: &SessionKeys,
        encrypted: bool,
        ip: Option<IpAddr>,
    ) {
        let mut session = self.lock_session();
        if session.as_ref().is_none_or(|s| s.id != session_id) {
            *session = Some(Session {
                id: session_id,
                links: HashMap::new(),
                datagrams: HashMap::new(),
            });
            for s in &self.stats {
                s.reset();
            }
        }
        if let Some(s) = session.as_mut() {
            s.links.insert(
                conn,
                Link {
                    auth: keys.auth,
                    rx: Cipher::new(&keys.phone_to_pc),
                    tx: keys.pc_to_phone,
                    encrypted,
                    ip,
                    replay: Default::default(),
                },
            );
        }
    }

    pub fn remove_link(&self, conn: u64) {
        if let Some(s) = self.lock_session().as_mut() {
            s.links.remove(&conn);
            s.datagrams.retain(|_, c| *c != conn);
        }
    }

    pub fn clear(&self) {
        *self.lock_session() = None;
        *self.lock_out() = None;
    }

    /// Checks a carrier hello (stream 0) from a stream carrier. Returns the session and the
    /// connection whose keys proved it.
    pub fn carrier_hello(&self, payload: &[u8]) -> Option<([u8; 16], u64)> {
        let session = self.lock_session();
        let s = session.as_ref()?;
        Some((s.id, s.hello(payload)?))
    }

    /// One datagram. A carrier hello from a live link's address ties that address to its
    /// connection and returns the session and connection the first time; media from a tied
    /// address goes to the pipelines.
    pub fn datagram(&self, from: SocketAddr, packet: &mut [u8]) -> Option<([u8; 16], u64)> {
        let h = MediaHeader::decode(packet)?;
        let mut session = self.lock_session();
        let s = session.as_mut()?;
        if h.stream == stream::CARRIER_HELLO {
            // Only a live link's own address gets its MAC checked at all.
            if !s.links.values().any(|l| l.ip == Some(from.ip())) {
                return None;
            }
            let conn = s.hello(&packet[MediaHeader::LEN..])?;
            if s.links[&conn].ip != Some(from.ip()) || s.datagrams.get(&from) == Some(&conn) {
                return None;
            }
            s.datagrams.retain(|_, c| *c != conn);
            s.datagrams.insert(from, conn);
            return Some((s.id, conn));
        }
        let conn = *s.datagrams.get(&from)?;
        let accepted = s.accept(conn, packet);
        drop(session);
        if let Some((h, len)) = accepted {
            self.deliver(
                &h,
                &packet[MediaHeader::LEN..MediaHeader::LEN + len],
                packet.len(),
            );
        }
        None
    }

    /// One media packet from `conn`'s stream carrier: the USB debugging media channel, or a
    /// Bluetooth connection itself.
    pub fn receive(&self, conn: u64, packet: &mut [u8]) {
        let accepted = self
            .lock_session()
            .as_mut()
            .and_then(|s| s.accept(conn, packet));
        if let Some((h, len)) = accepted {
            self.deliver(
                &h,
                &packet[MediaHeader::LEN..MediaHeader::LEN + len],
                packet.len(),
            );
        }
    }

    fn deliver(&self, h: &MediaHeader, payload: &[u8], size: usize) {
        self.stats[h.stream as usize].packet(h.seq, h.timestamp_us, self.now_us(), size);
        let sink = match h.stream {
            stream::MIC => &self.mic,
            stream::CAMERA => &self.camera,
            _ => &None,
        };
        if let Some(sink) = sink {
            sink.deliver(h, payload);
        }
    }

    /// Moves outgoing media to `conn`'s carrier, between two packets, sealed with its keys.
    pub fn set_out(&self, out: Option<(u64, Path)>) {
        let next = out.and_then(|(conn, path)| {
            let session = self.lock_session();
            let link = session.as_ref()?.links.get(&conn)?;
            Some(Out {
                path,
                seal: link.encrypted.then(|| Cipher::new(&link.tx)),
                packet: Vec::with_capacity(2048),
            })
        });
        *self.lock_out() = next;
    }

    /// Sends one speaker packet on the active carrier. Dropped when there is none. Never waits
    /// on a socket: stream carriers queue it.
    pub fn send_speaker(&self, timestamp_us: u32, payload: &[u8]) {
        let mut out = self.lock_out();
        let Some(o) = out.as_mut() else { return };
        let seq = self.speaker_seq.load(Ordering::Relaxed);
        if seq == SEQ_LIMIT {
            return;
        }
        self.speaker_seq.store(seq + 1, Ordering::Relaxed);
        let header = MediaHeader {
            stream: stream::SPEAKER,
            keyframe: false,
            seq,
            timestamp_us,
        };
        o.packet.clear();
        o.packet.extend_from_slice(&header.encode());
        o.packet.extend_from_slice(payload);
        if let Some(c) = &o.seal {
            c.seal_in_place(stream::SPEAKER, seq as u64, &mut o.packet, MediaHeader::LEN);
        }
        match &o.path {
            Path::Udp { socket, to } => {
                let _ = socket.send_to(&o.packet, to);
            }
            Path::Stream(stream) => stream.media(&o.packet),
        }
    }

    /// The speaker's `seq` reached its limit: every link must shake hands again.
    pub fn speaker_seq_exhausted(&self) -> bool {
        self.speaker_seq.load(Ordering::Relaxed) == SEQ_LIMIT
    }

    /// After every link that used the old keys is gone.
    pub fn restart_speaker_seq(&self) {
        let _out = self.lock_out();
        self.speaker_seq.store(0, Ordering::Relaxed);
    }

    fn lock_session(&self) -> std::sync::MutexGuard<'_, Option<Session>> {
        self.session.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn lock_out(&self) -> std::sync::MutexGuard<'_, Option<Out>> {
        self.out.lock().unwrap_or_else(|p| p.into_inner())
    }

    #[cfg(test)]
    pub(crate) fn set_speaker_seq(&self, seq: u32) {
        self.speaker_seq.store(seq, Ordering::Relaxed);
    }
}

/// A carrier hello packet, as the phone sends it (and tests do).
pub fn carrier_hello_packet(session_id: [u8; 16], auth: &Key) -> Vec<u8> {
    let header = MediaHeader {
        stream: stream::CARRIER_HELLO,
        keyframe: false,
        seq: 0,
        timestamp_us: 0,
    };
    [
        header.encode().as_slice(),
        &session_id,
        &crypto::carrier_mac(auth, &session_id),
    ]
    .concat()
}

#[cfg(test)]
mod tests {
    use super::*;
    use owlmic_proto::crypto::session_keys;

    struct Collect(Mutex<Vec<(u32, Vec<u8>)>>);
    impl MediaSink for Collect {
        fn deliver(&self, h: &MediaHeader, p: &[u8]) {
            self.0.lock().unwrap().push((h.seq, p.to_vec()));
        }
    }

    fn sealed(keys: &SessionKeys, stream: u8, seq: u32, payload: &[u8]) -> Vec<u8> {
        let h = MediaHeader {
            stream,
            keyframe: false,
            seq,
            timestamp_us: seq * 10_000,
        }
        .encode();
        [
            h.as_slice(),
            &Cipher::new(&keys.phone_to_pc).seal(stream, seq as u64, &h, payload),
        ]
        .concat()
    }

    #[test]
    fn sealed_mic_packets_reach_the_pipeline_once_and_forgeries_never() {
        let mic = Arc::new(Collect(Mutex::new(Vec::new())));
        let routes = Routes::new(Some(mic.clone()), None);
        let keys = session_keys(&[4; 32]);
        routes.add_link([1; 16], 7, &keys, true, None);
        routes.receive(7, &mut sealed(&keys, stream::MIC, 1, b"a"));
        routes.receive(7, &mut sealed(&keys, stream::MIC, 1, b"a"));
        let mut forged = sealed(&keys, stream::MIC, 2, b"b");
        *forged.last_mut().unwrap() ^= 1;
        routes.receive(7, &mut forged);
        routes.receive(7, &mut sealed(&keys, stream::MIC, 3, b"c"));
        routes.receive(8, &mut sealed(&keys, stream::MIC, 4, b"not a live link"));
        assert_eq!(
            *mic.0.lock().unwrap(),
            vec![(1, b"a".to_vec()), (3, b"c".to_vec())]
        );
        assert_eq!(routes.stats[1].packets.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn every_live_link_of_the_session_proves_its_own_carrier() {
        let routes = Routes::new(None, None);
        let (wifi, usb) = (session_keys(&[5; 32]), session_keys(&[6; 32]));
        routes.add_link([2; 16], 1, &wifi, true, None);
        routes.add_link([2; 16], 2, &usb, false, None);
        let hello = |keys: &SessionKeys| carrier_hello_packet([2; 16], &keys.auth);
        assert_eq!(
            routes.carrier_hello(&hello(&usb)[MediaHeader::LEN..]),
            Some(([2; 16], 2)),
            "the standby link's hello, with its own keys"
        );
        assert_eq!(
            routes.carrier_hello(&hello(&wifi)[MediaHeader::LEN..]),
            Some(([2; 16], 1))
        );
        let forged = carrier_hello_packet([2; 16], &[0; 32]);
        assert_eq!(routes.carrier_hello(&forged[MediaHeader::LEN..]), None);
        routes.remove_link(2);
        assert_eq!(routes.carrier_hello(&hello(&usb)[MediaHeader::LEN..]), None);
    }

    #[test]
    fn a_datagram_carrier_is_tied_to_its_own_address_and_link() {
        let mic = Arc::new(Collect(Mutex::new(Vec::new())));
        let routes = Routes::new(Some(mic.clone()), None);
        let keys = session_keys(&[7; 32]);
        let phone: SocketAddr = "192.168.1.7:5000".parse().unwrap();
        let stranger: SocketAddr = "192.168.1.9:5000".parse().unwrap();
        routes.add_link([3; 16], 4, &keys, true, Some(phone.ip()));
        let mut hello = carrier_hello_packet([3; 16], &keys.auth);
        assert_eq!(routes.datagram(stranger, &mut hello.clone()), None);
        assert_eq!(routes.datagram(phone, &mut hello), Some(([3; 16], 4)));
        assert_eq!(
            routes.datagram(phone, &mut carrier_hello_packet([3; 16], &keys.auth)),
            None,
            "a repeated hello changes nothing"
        );
        routes.datagram(stranger, &mut sealed(&keys, stream::MIC, 1, b"x"));
        routes.datagram(phone, &mut sealed(&keys, stream::MIC, 2, b"y"));
        assert_eq!(*mic.0.lock().unwrap(), vec![(2, b"y".to_vec())]);
    }

    #[test]
    fn speaker_packets_go_out_sealed_on_the_active_carrier() {
        let routes = Routes::new(None, None);
        let keys = session_keys(&[6; 32]);
        routes.add_link([3; 16], 1, &keys, true, None);
        let phone = UdpSocket::bind("127.0.0.1:0").unwrap();
        phone
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        let pc = Arc::new(UdpSocket::bind("127.0.0.1:0").unwrap());
        routes.send_speaker(0, b"lost: no carrier yet");
        routes.set_out(Some((
            1,
            Path::Udp {
                socket: pc,
                to: phone.local_addr().unwrap(),
            },
        )));
        routes.send_speaker(480_000, b"pcm");
        let mut buf = [0u8; 256];
        let n = phone.recv(&mut buf).unwrap();
        let h = MediaHeader::decode(&buf[..n]).unwrap();
        assert_eq!(h.stream, stream::SPEAKER);
        let plain =
            Cipher::new(&keys.pc_to_phone).open(h.stream, h.seq as u64, &buf[..10], &buf[10..n]);
        assert_eq!(plain.as_deref(), Some(b"pcm".as_slice()));
    }

    #[test]
    fn the_speaker_stops_before_its_seq_would_wrap() {
        let routes = Routes::new(None, None);
        let keys = session_keys(&[8; 32]);
        routes.add_link([4; 16], 1, &keys, false, None);
        let phone = UdpSocket::bind("127.0.0.1:0").unwrap();
        phone
            .set_read_timeout(Some(std::time::Duration::from_millis(200)))
            .unwrap();
        let pc = Arc::new(UdpSocket::bind("127.0.0.1:0").unwrap());
        routes.set_out(Some((
            1,
            Path::Udp {
                socket: pc,
                to: phone.local_addr().unwrap(),
            },
        )));
        routes.set_speaker_seq(SEQ_LIMIT - 1);
        routes.send_speaker(0, b"last");
        routes.send_speaker(0, b"never");
        let mut buf = [0u8; 64];
        let n = phone.recv(&mut buf).unwrap();
        assert_eq!(MediaHeader::decode(&buf[..n]).unwrap().seq, SEQ_LIMIT - 1);
        assert!(phone.recv(&mut buf).is_err(), "nothing past the limit");
        assert!(routes.speaker_seq_exhausted());
        routes.restart_speaker_seq();
        assert!(!routes.speaker_seq_exhausted());
    }
}
