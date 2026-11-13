//! TCP connections to the control port (protocol/README.md, section 3): the first byte says
//! whether it is a control channel or, through `adb reverse`, a media channel. Each connection
//! reads on its own thread and hands frames to the Link Hub.

use crate::hub::{Carrier, LinkMsg, Opened};
use crate::media::Routes;
use crate::netinfo::NetInfo;
use crate::wire::{CipherSlot, Closer, Item, Outgoing, Reader};
use owlmic_hub::Outbox;
use owlmic_proto::frame::{CHANNEL_CONTROL, CHANNEL_MEDIA, MediaHeader, stream};
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A backstop that ends a forgotten connection's thread. The approval wait is 2 minutes with
/// nothing on the wire, and the heartbeat ends dead links much sooner.
const CONTROL_READ_TIMEOUT: Duration = Duration::from_secs(150);
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const FIRST_BYTE_TIMEOUT: Duration = Duration::from_secs(3);
/// Connections open at once, Bluetooth included; more are closed at once.
const MAX_CONNECTIONS: usize = 16;

pub struct Shared {
    pub routes: Arc<Routes>,
    pub to_hub: Outbox<LinkMsg>,
    pub next_conn: AtomicU64,
    pub net: Arc<dyn NetInfo>,
    pub open: AtomicUsize,
}

impl Shared {
    pub fn new(routes: Arc<Routes>, to_hub: Outbox<LinkMsg>, net: Arc<dyn NetInfo>) -> Self {
        Self {
            routes,
            to_hub,
            next_conn: AtomicU64::new(1),
            net,
            open: AtomicUsize::new(0),
        }
    }

    pub fn conn_id(&self) -> u64 {
        self.next_conn.fetch_add(1, Ordering::Relaxed)
    }
}

/// One of the [`MAX_CONNECTIONS`]; freed when dropped.
pub struct Slot(Arc<Shared>);

/// A slot for a new connection, or `None` when too many are open.
pub fn claim(shared: &Arc<Shared>) -> Option<Slot> {
    // Counted first and given back when over, so a claim never waits on another.
    let slot = Slot(shared.clone());
    (shared.open.fetch_add(1, Ordering::AcqRel) < MAX_CONNECTIONS).then_some(slot)
}

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.open.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Accepts connections until the listener fails. Runs on its own supervised thread.
pub fn serve(listener: TcpListener, shared: Arc<Shared>) -> Result<(), String> {
    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(e)
                if matches!(
                    e.kind(),
                    ErrorKind::ConnectionReset
                        | ErrorKind::ConnectionAborted
                        | ErrorKind::Interrupted
                ) =>
            {
                continue;
            }
            Err(e) => return Err(e.to_string()),
        };
        let Some(slot) = claim(&shared) else { continue };
        let shared = shared.clone();
        let _ = std::thread::Builder::new()
            .name("owlmic-conn".into())
            .spawn(move || {
                accept(stream, &shared);
                drop(slot);
            });
    }
    Err("the listener stopped".into())
}

fn accept(mut stream: TcpStream, shared: &Shared) {
    let Ok(peer) = stream.peer_addr() else { return };
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(FIRST_BYTE_TIMEOUT));
    let mut channel = [0u8; 1];
    if stream.read_exact(&mut channel).is_err() {
        return;
    }
    let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
    let (Ok(read_half), Ok(write_half), Ok(closer)) =
        (stream.try_clone(), stream.try_clone(), stream.try_clone())
    else {
        return;
    };
    let close: Closer = Arc::new(move || {
        let _ = closer.shutdown(std::net::Shutdown::Both);
    });
    // Through adb reverse the phone always arrives from this PC itself.
    let usb_debugging = peer.ip().is_loopback();
    match channel[0] {
        CHANNEL_CONTROL => {
            let _ = stream.set_read_timeout(Some(CONTROL_READ_TIMEOUT));
            let link = if usb_debugging {
                crate::LINK_USB_DEBUGGING
            } else {
                shared.net.link_for_peer(peer.ip())
            };
            run_control(read_half, Box::new(write_half), link, peer, close, shared);
        }
        CHANNEL_MEDIA if usb_debugging => {
            run_media_channel(read_half, Box::new(write_half), close, shared)
        }
        _ => close(),
    }
}

/// Runs one control connection until it closes. Also used for Bluetooth, where the same stream
/// carries the audio; TCP control connections never carry media.
pub fn run_control(
    read: impl Read,
    write: Box<dyn Write + Send>,
    link: u8,
    peer: SocketAddr,
    close: Closer,
    shared: &Shared,
) {
    let conn = shared.conn_id();
    let out = Outgoing::start(write, close);
    let rx: CipherSlot = Arc::new(Mutex::new(None));
    shared.to_hub.send(LinkMsg::Opened(Opened {
        conn,
        link,
        peer,
        out: out.clone(),
        rx: rx.clone(),
    }));
    let carries_media = link == crate::LINK_BLUETOOTH;
    let mut reader = Reader::new(read, rx);
    while let Ok(item) = reader.read_item() {
        match item {
            Item::Control { kind, payload } => shared.to_hub.send_wait(LinkMsg::Frame {
                conn,
                kind,
                payload,
            }),
            // Taken only once the connection is live: before that it has no keys in the routes.
            Item::Media(mut packet) if carries_media => shared.routes.receive(conn, &mut packet),
            Item::Media(_) => {}
        }
    }
    out.close();
    shared.to_hub.send(LinkMsg::Closed { conn });
}

/// The media channel through `adb reverse`: a carrier hello first, proving which live
/// connection it belongs to, then media packets. Its liveness is its control link's: no read
/// timeout, since a phone with nothing on sends nothing here.
fn run_media_channel(
    read: TcpStream,
    write: Box<dyn Write + Send>,
    close: Closer,
    shared: &Shared,
) {
    let channel = shared.conn_id();
    let mut reader = Reader::new(&read, Arc::new(Mutex::new(None)));
    let Ok(Item::Media(hello)) = reader.read_item() else {
        return close();
    };
    let proven = MediaHeader::decode(&hello)
        .filter(|h| h.stream == stream::CARRIER_HELLO)
        .and_then(|_| shared.routes.carrier_hello(&hello[MediaHeader::LEN..]));
    let Some((session_id, conn)) = proven else {
        return close();
    };
    let _ = read.set_read_timeout(None);
    let out = Outgoing::start(write, close);
    shared.to_hub.send(LinkMsg::CarrierUp {
        conn,
        session_id,
        carrier: Carrier::Channel {
            id: channel,
            out: out.clone(),
        },
    });
    while let Ok(item) = reader.read_item() {
        if let Item::Media(mut packet) = item {
            shared.routes.receive(conn, &mut packet);
        }
    }
    out.close();
    shared.to_hub.send(LinkMsg::CarrierDown { channel });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connections_past_the_cap_are_refused_until_one_closes() {
        let (to_hub, _inbox) = owlmic_hub::mailbox(4);
        let shared = Arc::new(Shared::new(
            Arc::new(Routes::new(None, None)),
            to_hub,
            Arc::new(crate::netinfo::Simple),
        ));
        let mut slots: Vec<Slot> = (0..MAX_CONNECTIONS)
            .filter_map(|_| claim(&shared))
            .collect();
        assert_eq!(slots.len(), MAX_CONNECTIONS);
        assert!(claim(&shared).is_none());
        assert!(claim(&shared).is_none());
        // The refused claims gave their count back: one closing frees exactly one.
        slots.pop();
        assert!(claim(&shared).is_some());
    }
}
