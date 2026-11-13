//! The datagram carrier on UDP 7655 (protocol/README.md, section 6): a carrier hello proves which
//! connection a phone's address belongs to; after that its media goes straight to the pipelines.

use crate::hub::{Carrier, LinkMsg};
use crate::media::Routes;
use owlmic_hub::Outbox;
use std::net::UdpSocket;

/// Binds the media port, or any free port when another program holds it; discovery answers
/// with the port actually bound.
pub fn bind(port: u16) -> std::io::Result<UdpSocket> {
    UdpSocket::bind(("0.0.0.0", port)).or_else(|_| UdpSocket::bind(("0.0.0.0", 0)))
}

/// Receives media until the socket fails. Runs on its own supervised thread.
pub fn run(socket: &UdpSocket, routes: &Routes, to_hub: &Outbox<LinkMsg>) -> Result<(), String> {
    let mut buf = vec![0u8; 2048];
    loop {
        let (n, from) = match socket.recv_from(&mut buf) {
            Ok(r) => r,
            // Windows reports an earlier send's ICMP "port unreachable" here; it isn't fatal.
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => continue,
            Err(e) => return Err(e.to_string()),
        };
        if let Some((session_id, conn)) = routes.datagram(from, &mut buf[..n]) {
            to_hub.send(LinkMsg::CarrierUp {
                conn,
                session_id,
                carrier: Carrier::Udp(from),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_taken_media_port_falls_back_to_a_free_one() {
        let taken = UdpSocket::bind("0.0.0.0:0").unwrap();
        let port = taken.local_addr().unwrap().port();
        let socket = bind(port).unwrap();
        let bound = socket.local_addr().unwrap().port();
        assert_ne!(bound, port);
        assert_ne!(bound, 0);
    }
}
