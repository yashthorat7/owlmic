//! Discovery, PC side (SYSTEM_DESIGN section 14.2): answers each probe at once, and announces
//! itself three times when it starts or a network changes. Costs nothing between probes.

use crate::netinfo::NetInfo;
use crate::ratelimit::{PROBES_PER_SECOND, TokenBucket};
use owlmic_proto::discovery::{Answer, Probe};
use std::net::UdpSocket;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct Me {
    pub pc_id: [u8; 16],
    pub key_hint: [u8; 8],
    pub name: String,
    pub tcp_port: u16,
    /// The media port actually bound, which may not be the usual one.
    pub media_port: u16,
}

/// Whether a phone (by id) is approved on this PC.
pub type IsApproved = Arc<dyn Fn(&[u8; 16]) -> bool + Send + Sync>;

pub struct Answerer {
    socket: UdpSocket,
    me: Me,
    busy: Arc<AtomicBool>,
    approved: IsApproved,
    net: Arc<dyn NetInfo>,
}

impl Answerer {
    pub fn bind(
        port: u16,
        me: Me,
        busy: Arc<AtomicBool>,
        approved: IsApproved,
        net: Arc<dyn NetInfo>,
    ) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(("0.0.0.0", port))?;
        socket.set_broadcast(true)?;
        Ok(Self {
            socket,
            me,
            busy,
            approved,
            net,
        })
    }

    fn answer(&self, phone_id: Option<&[u8; 16]>, link: u8) -> Vec<u8> {
        Answer {
            pc_id: self.me.pc_id,
            key_hint: self.me.key_hint,
            tcp_port: self.me.tcp_port,
            media_port: self.me.media_port,
            proto: owlmic_proto::VERSION,
            busy: self.busy.load(Ordering::Relaxed),
            approval_required: phone_id.is_some_and(|id| !(self.approved)(id)),
            link,
            name: self.me.name.clone(),
        }
        .encode()
    }

    /// Three announcements, a moment apart, so a phone that is already searching hears one. Each
    /// network hears the link it is: a USB tethering network says so.
    pub fn announce(&self, port: u16) {
        for i in 0..3 {
            for to in self.net.broadcasts(port) {
                let packet = self.answer(None, self.net.link_for_peer(to.ip()));
                let _ = self.socket.send_to(&packet, to);
            }
            if i < 2 {
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }

    /// Answers probes until the socket fails. Runs on its own supervised thread; the answerer is
    /// shared so announcements can use it too.
    pub fn run(&self) -> Result<(), String> {
        let mut bucket = TokenBucket::new(PROBES_PER_SECOND, Instant::now());
        let mut buf = [0u8; 512];
        loop {
            let (n, from) = match self.socket.recv_from(&mut buf) {
                Ok(r) => r,
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => continue,
                Err(e) => return Err(e.to_string()),
            };
            let Some(probe) = Probe::decode(&buf[..n]) else {
                continue;
            };
            if !bucket.allow(Instant::now()) {
                continue;
            }
            let link = self.net.link_for_peer(from.ip());
            let _ = self
                .socket
                .send_to(&self.answer(Some(&probe.phone_id), link), from);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    fn probe(socket: &UdpSocket, to: SocketAddr, phone_id: [u8; 16]) {
        let p = Probe {
            phone_id,
            name: "Pixel 8".into(),
        };
        socket.send_to(&p.encode(), to).unwrap();
    }

    fn me(media_port: u16) -> Me {
        Me {
            pc_id: [2; 16],
            key_hint: [3; 8],
            name: "DESKTOP-A".into(),
            tcp_port: 7653,
            media_port,
        }
    }

    fn phone() -> UdpSocket {
        let phone = UdpSocket::bind("127.0.0.1:0").unwrap();
        phone
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        phone
    }

    #[test]
    fn a_probe_gets_an_answer_saying_who_and_whether_approval_is_needed() {
        let busy = Arc::new(AtomicBool::new(false));
        let a = Answerer::bind(
            0,
            me(7655),
            busy.clone(),
            Arc::new(|id| *id == [1; 16]),
            Arc::new(crate::netinfo::Simple),
        )
        .unwrap();
        let port = a.socket.local_addr().unwrap().port();
        std::thread::spawn(move || a.run());
        let phone = phone();
        let mut buf = [0u8; 512];
        for (id, approval) in [([1u8; 16], false), ([9u8; 16], true)] {
            probe(&phone, ([127, 0, 0, 1], port).into(), id);
            let n = phone.recv(&mut buf).unwrap();
            let answer = Answer::decode(&buf[..n]).unwrap();
            assert_eq!(answer.name, "DESKTOP-A");
            assert_eq!(answer.pc_id, [2; 16]);
            assert_eq!(answer.media_port, 7655);
            assert_eq!(answer.approval_required, approval);
            assert!(!answer.busy);
        }
        busy.store(true, Ordering::Relaxed);
        probe(&phone, ([127, 0, 0, 1], port).into(), [1; 16]);
        let n = phone.recv(&mut buf).unwrap();
        assert!(Answer::decode(&buf[..n]).unwrap().busy);
    }

    #[test]
    fn the_answer_carries_the_media_port_actually_bound() {
        let taken = UdpSocket::bind("0.0.0.0:0").unwrap();
        let media = crate::udp::bind(taken.local_addr().unwrap().port()).unwrap();
        let bound = media.local_addr().unwrap().port();
        let a = Answerer::bind(
            0,
            me(bound),
            Arc::default(),
            Arc::new(|_| true),
            Arc::new(crate::netinfo::Simple),
        )
        .unwrap();
        let port = a.socket.local_addr().unwrap().port();
        std::thread::spawn(move || a.run());
        let phone = phone();
        probe(&phone, ([127, 0, 0, 1], port).into(), [1; 16]);
        let mut buf = [0u8; 512];
        let n = phone.recv(&mut buf).unwrap();
        assert_eq!(Answer::decode(&buf[..n]).unwrap().media_port, bound);
    }

    /// Announcements to one listening phone, on a network that is USB tethering.
    struct Tethered(SocketAddr);
    impl NetInfo for Tethered {
        fn link_for_peer(&self, peer: IpAddr) -> u8 {
            if peer == self.0.ip() {
                crate::LINK_USB_TETHERING
            } else {
                crate::LINK_WIFI
            }
        }
        fn broadcasts(&self, _port: u16) -> Vec<SocketAddr> {
            vec![self.0]
        }
    }

    #[test]
    fn announcements_say_which_link_each_network_is() {
        let phone = phone();
        let to = SocketAddr::new(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            phone.local_addr().unwrap().port(),
        );
        let a = Answerer::bind(
            0,
            me(7655),
            Arc::default(),
            Arc::new(|_| true),
            Arc::new(Tethered(to)),
        )
        .unwrap();
        a.announce(to.port());
        let mut buf = [0u8; 512];
        let n = phone.recv(&mut buf).unwrap();
        assert_eq!(
            Answer::decode(&buf[..n]).unwrap().link,
            crate::LINK_USB_TETHERING
        );
    }
}
