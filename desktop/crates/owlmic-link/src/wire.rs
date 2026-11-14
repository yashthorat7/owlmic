//! Control frames and wrapped media on a byte stream (protocol/README.md, sections 3 and 4),
//! encrypted on wireless links once the session is up. Each connection reads on its own thread
//! and writes on another, fed by [`Outgoing`], so a slow socket stalls nobody else.

use owlmic_proto::crypto::{CONTROL_STREAM, Cipher};
use owlmic_proto::frame::{ControlHeader, MEDIA_MARKER, wrap_for_stream};
use owlmic_proto::messages::Message;
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::sync::{Arc, Condvar, Mutex};

/// Ends a connection: shuts its socket, so its reader stops too.
pub type Closer = Arc<dyn Fn() + Send + Sync>;

pub enum Item {
    /// A control frame: its type and its (decrypted) payload.
    Control { kind: u8, payload: Vec<u8> },
    /// A media packet from a stream carrier.
    Media(Vec<u8>),
}

/// The receive cipher, set by the Link Hub right before WELCOME goes out, so the reader turns
/// on decryption exactly where the phone turns on encryption.
pub type CipherSlot = Arc<Mutex<Option<Cipher>>>;

pub struct Reader<R> {
    inner: R,
    cipher: CipherSlot,
    counter: u64,
}

impl<R: Read> Reader<R> {
    pub fn new(inner: R, cipher: CipherSlot) -> Self {
        Self {
            inner,
            cipher,
            counter: 0,
        }
    }

    pub fn read_item(&mut self) -> io::Result<Item> {
        let mut first = [0u8; 1];
        self.inner.read_exact(&mut first)?;
        if first[0] == MEDIA_MARKER {
            let mut len = [0u8; 2];
            self.inner.read_exact(&mut len)?;
            let mut packet = vec![0; u16::from_be_bytes(len) as usize];
            self.inner.read_exact(&mut packet)?;
            return Ok(Item::Media(packet));
        }
        let mut rest = [0u8; 3];
        self.inner.read_exact(&mut rest)?;
        let raw = [first[0], rest[0], rest[1], rest[2]];
        let header = ControlHeader::decode(raw)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "bad frame"))?;
        let mut payload = vec![0; header.len as usize];
        self.inner.read_exact(&mut payload)?;
        let cipher = self.cipher.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(c) = cipher.as_ref() {
            payload = c
                .open(CONTROL_STREAM, self.counter, &raw, &payload)
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "frame failed authentication")
                })?;
            self.counter += 1;
        }
        Ok(Item::Control {
            kind: header.kind,
            payload,
        })
    }
}

pub struct Writer<W> {
    inner: W,
    cipher: Option<Cipher>,
    counter: u64,
}

impl<W: Write> Writer<W> {
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            cipher: None,
            counter: 0,
        }
    }

    /// From now on, control frames are sealed.
    pub fn encrypt_with(&mut self, cipher: Cipher) {
        self.cipher = Some(cipher);
        self.counter = 0;
    }

    pub fn send(&mut self, msg: &Message) -> io::Result<()> {
        self.send_raw(msg.kind(), &msg.to_payload())
    }

    /// Sends a payload already serialized, such as a HELLO_ACK whose bytes are in the transcript.
    pub fn send_raw(&mut self, kind: u8, payload: &[u8]) -> io::Result<()> {
        let frame = match &self.cipher {
            None => {
                let header = ControlHeader {
                    kind,
                    len: payload.len() as u32,
                }
                .encode();
                [header.as_slice(), payload].concat()
            }
            Some(c) => {
                let header = ControlHeader {
                    kind,
                    len: payload.len() as u32 + 16,
                }
                .encode();
                let sealed = c.seal(CONTROL_STREAM, self.counter, &header, payload);
                self.counter += 1;
                [header.as_slice(), &sealed].concat()
            }
        };
        self.inner.write_all(&frame)?;
        self.inner.flush()
    }

    pub fn media(&mut self, packet: &[u8]) -> io::Result<()> {
        let mut out = Vec::with_capacity(packet.len() + 3);
        wrap_for_stream(packet, &mut out);
        self.inner.write_all(&out)
    }
}

/// Control frames waiting before a connection counts as stuck and is closed.
const MAX_CONTROL: usize = 256;
/// Media packets waiting: about a third of a second of speaker audio.
const MAX_MEDIA: usize = 32;

enum Job {
    Frame(u8, Vec<u8>),
    Encrypt(Box<Cipher>),
    Close,
}

struct Queue {
    /// In order and never dropped: a missing sealed frame would break the counter after it.
    control: VecDeque<Job>,
    /// Wrapped for the stream; the oldest goes when the carrier can't keep up.
    media: VecDeque<Vec<u8>>,
    spare: Vec<Vec<u8>>,
    stopped: bool,
}

struct OutShared {
    queue: Mutex<Queue>,
    ready: Condvar,
    close: Closer,
}

/// A connection's outgoing side: a bounded queue and one writer thread. Nothing that sends ever
/// waits on the socket.
#[derive(Clone)]
pub struct Outgoing {
    shared: Arc<OutShared>,
}

impl Outgoing {
    /// Starts the writer thread for `write`. A failed write ends the connection with `close`.
    pub fn start(write: Box<dyn Write + Send>, close: Closer) -> Self {
        let shared = Arc::new(OutShared {
            queue: Mutex::new(Queue {
                control: VecDeque::new(),
                media: VecDeque::with_capacity(MAX_MEDIA),
                spare: Vec::new(),
                stopped: false,
            }),
            ready: Condvar::new(),
            close,
        });
        let s = shared.clone();
        let _ = std::thread::Builder::new()
            .name("owlmic-write".into())
            .spawn(move || write_until_stopped(&s, Writer::new(write)));
        Self { shared }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Queue> {
        self.shared.queue.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn push(&self, job: Job) {
        let mut q = self.lock();
        if q.stopped {
            return;
        }
        if q.control.len() >= MAX_CONTROL {
            drop(q);
            return self.close();
        }
        q.control.push_back(job);
        drop(q);
        self.shared.ready.notify_one();
    }

    pub fn send(&self, msg: &Message) {
        self.send_raw(msg.kind(), msg.to_payload());
    }

    /// A payload already serialized, such as a HELLO_ACK whose bytes are in the transcript.
    pub fn send_raw(&self, kind: u8, payload: Vec<u8>) {
        self.push(Job::Frame(kind, payload));
    }

    /// Control frames queued after this are sealed.
    pub fn encrypt_with(&self, cipher: Cipher) {
        self.push(Job::Encrypt(Box::new(cipher)));
    }

    /// Ends the connection once everything queued so far is written, as after a REJECT.
    pub fn close_when_sent(&self) {
        self.push(Job::Close);
    }

    /// Queues one media packet. When the queue is full the oldest waiting packet goes.
    pub fn media(&self, packet: &[u8]) {
        let mut q = self.lock();
        if q.stopped {
            return;
        }
        let mut buf = if q.media.len() >= MAX_MEDIA {
            q.media.pop_front().unwrap_or_default()
        } else {
            q.spare.pop().unwrap_or_default()
        };
        buf.clear();
        wrap_for_stream(packet, &mut buf);
        q.media.push_back(buf);
        drop(q);
        self.shared.ready.notify_one();
    }

    /// Ends the connection now and stops the writer thread.
    pub fn close(&self) {
        self.lock().stopped = true;
        self.shared.ready.notify_all();
        (self.shared.close)();
    }
}

enum Next {
    Job(Job),
    Media(Vec<u8>),
}

fn write_until_stopped(shared: &OutShared, mut w: Writer<Box<dyn Write + Send>>) {
    loop {
        let next = {
            let mut q = shared.queue.lock().unwrap_or_else(|p| p.into_inner());
            loop {
                if q.stopped {
                    return;
                }
                if let Some(j) = q.control.pop_front() {
                    break Next::Job(j);
                }
                if let Some(m) = q.media.pop_front() {
                    break Next::Media(m);
                }
                q = shared.ready.wait(q).unwrap_or_else(|p| p.into_inner());
            }
        };
        let written = match next {
            Next::Job(Job::Frame(kind, payload)) => w.send_raw(kind, &payload).is_ok(),
            Next::Job(Job::Encrypt(c)) => {
                w.encrypt_with(*c);
                true
            }
            Next::Job(Job::Close) => false,
            Next::Media(buf) => {
                let ok = w.inner.write_all(&buf).is_ok();
                let mut q = shared.queue.lock().unwrap_or_else(|p| p.into_inner());
                if q.spare.len() < MAX_MEDIA {
                    q.spare.push(buf);
                }
                ok
            }
        };
        if !written {
            shared
                .queue
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .stopped = true;
            (shared.close)();
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use owlmic_proto::messages::Ping;

    #[test]
    fn frames_round_trip_in_the_clear_and_sealed() {
        let key = [7u8; 32];
        let mut w = Writer::new(Vec::new());
        w.send(&Message::Ping(Ping { t: 1 })).unwrap();
        w.media(&[1, 2, 3]).unwrap();
        w.encrypt_with(Cipher::new(&key));
        w.send(&Message::Ping(Ping { t: 2 })).unwrap();
        w.send(&Message::Ping(Ping { t: 3 })).unwrap();

        let slot: CipherSlot = Arc::new(Mutex::new(None));
        let mut r = Reader::new(w.inner.as_slice(), slot.clone());
        assert!(
            matches!(r.read_item().unwrap(), Item::Control { payload, .. } if payload == br#"{"t":1}"#)
        );
        assert!(matches!(r.read_item().unwrap(), Item::Media(p) if p == [1, 2, 3]));
        *slot.lock().unwrap() = Some(Cipher::new(&key));
        assert!(
            matches!(r.read_item().unwrap(), Item::Control { payload, .. } if payload == br#"{"t":2}"#)
        );
        assert!(
            matches!(r.read_item().unwrap(), Item::Control { payload, .. } if payload == br#"{"t":3}"#)
        );
    }

    #[test]
    fn a_tampered_sealed_frame_is_refused() {
        let mut w = Writer::new(Vec::new());
        w.encrypt_with(Cipher::new(&[1; 32]));
        w.send(&Message::Ping(Ping { t: 9 })).unwrap();
        let mut bytes = w.inner;
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        let mut r = Reader::new(
            bytes.as_slice(),
            Arc::new(Mutex::new(Some(Cipher::new(&[1; 32])))),
        );
        assert!(r.read_item().is_err());
    }

    /// A writer the test can read back, and whether the connection was closed.
    #[derive(Clone, Default)]
    struct Sink(Arc<Mutex<Vec<u8>>>);
    impl Write for Sink {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn wait_for(f: impl Fn() -> bool) {
        for _ in 0..200 {
            if f() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("timed out");
    }

    #[test]
    fn queued_frames_go_out_in_order_then_the_connection_closes() {
        let sink = Sink::default();
        let closed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let c = closed.clone();
        let out = Outgoing::start(
            Box::new(sink.clone()),
            Arc::new(move || c.store(true, std::sync::atomic::Ordering::SeqCst)),
        );
        out.send(&Message::Ping(Ping { t: 1 }));
        out.encrypt_with(Cipher::new(&[3; 32]));
        out.send(&Message::Ping(Ping { t: 2 }));
        out.close_when_sent();
        out.send(&Message::Ping(Ping { t: 3 }));
        wait_for(|| closed.load(std::sync::atomic::Ordering::SeqCst));
        let bytes = sink.0.lock().unwrap().clone();
        let slot: CipherSlot = Arc::new(Mutex::new(None));
        let mut r = Reader::new(bytes.as_slice(), slot.clone());
        assert!(
            matches!(r.read_item().unwrap(), Item::Control { payload, .. } if payload == br#"{"t":1}"#)
        );
        *slot.lock().unwrap() = Some(Cipher::new(&[3; 32]));
        assert!(
            matches!(r.read_item().unwrap(), Item::Control { payload, .. } if payload == br#"{"t":2}"#)
        );
        assert!(r.read_item().is_err(), "nothing after the close");
    }

    /// A socket that blocks every write until the test lets it go.
    struct Stuck(Arc<(Mutex<bool>, Condvar)>, Sink);
    impl Write for Stuck {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let (open, cv) = &*self.0;
            let mut g = open.lock().unwrap();
            while !*g {
                g = cv.wait(g).unwrap();
            }
            self.1.write(buf)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_stuck_carrier_never_blocks_the_sender_and_keeps_the_newest_media() {
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let sink = Sink::default();
        let out = Outgoing::start(Box::new(Stuck(gate.clone(), sink.clone())), Arc::new(|| {}));
        for i in 0..200u8 {
            out.media(&[i]);
        }
        *gate.0.lock().unwrap() = true;
        gate.1.notify_all();
        wait_for(|| sink.0.lock().unwrap().ends_with(&[MEDIA_MARKER, 0, 1, 199]));
        let n = sink.0.lock().unwrap().len() / 4;
        assert!(
            n <= MAX_MEDIA + 1,
            "{n} packets went out; the rest were dropped"
        );
    }
}
