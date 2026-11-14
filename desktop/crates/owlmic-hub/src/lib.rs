//! The shape every Owlmic hub shares (SYSTEM_DESIGN section 11): one supervised thread per hub
//! with a bounded mailbox, health reported upward once a second, and an immutable state snapshot
//! for the UI.

mod mailbox;
mod publish;
mod supervise;

pub use mailbox::{Inbox, Outbox, mailbox, mailbox_with};
pub use publish::Publisher;
pub use supervise::supervise;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant};
use supervise::Restarts;

/// How often a hub's health is sampled.
const HEALTH_EVERY: Duration = Duration::from_secs(1);

/// What a hub or an I/O thread reports about itself.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Health {
    #[default]
    Ok,
    Degraded(String),
    Failed(String),
}

/// A hub's logic. The runner owns the thread: it blocks on the mailbox until a message arrives or
/// the hub's next deadline passes.
pub trait Hub: Send + 'static {
    type Msg: Send + 'static;

    fn handle(&mut self, msg: Self::Msg);

    /// Timers: called when [`Hub::next_deadline`] has passed.
    fn tick(&mut self, _now: Instant) {}

    /// When the hub next needs [`Hub::tick`], or `None` to sleep until a message.
    fn next_deadline(&self) -> Option<Instant> {
        None
    }

    /// How the hub is doing, sampled once a second.
    fn health(&self) -> Health {
        Health::Ok
    }
}

/// Runs the hub `make` builds on its own named thread until every [`Outbox`] for `inbox` is gone.
/// A hub that panics is rebuilt with `make` after a backoff; messages that arrive while it is
/// down are lost. The hub is built on its own thread, so thread-bound resources stay there.
/// Health goes to `report` when it changes.
pub fn spawn_supervised<H: Hub>(
    name: &str,
    mut make: impl FnMut() -> H + Send + 'static,
    inbox: Inbox<H::Msg>,
    report: impl Fn(Health) + Send + 'static,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || {
            let mut restarts = Restarts::new(Instant::now());
            let mut hub = None;
            let mut restart_at = Some(Instant::now());
            let mut check_at = Instant::now();
            let mut last = None;
            loop {
                let now = Instant::now();
                if restart_at.is_some_and(|t| t <= now) {
                    restart_at = None;
                    restarts.started(now);
                    match catch_unwind(AssertUnwindSafe(&mut make)) {
                        Ok(h) => hub = Some(h),
                        Err(_) => restart_at = Some(now + restarts.failed(now, "not built")),
                    }
                }
                let own = hub.as_ref().and_then(H::next_deadline);
                let deadline = [restart_at, own, Some(check_at)]
                    .into_iter()
                    .flatten()
                    .min();
                let msg = match inbox.recv_until(deadline) {
                    mailbox::Recv::Msg(m) => Some(m),
                    mailbox::Recv::Deadline => None,
                    mailbox::Recv::Closed => return,
                };
                let now = Instant::now();
                if let Some(h) = hub.as_mut() {
                    let ran = catch_unwind(AssertUnwindSafe(|| {
                        if let Some(m) = msg {
                            h.handle(m);
                        }
                        if h.next_deadline().is_some_and(|d| d <= now) {
                            h.tick(now);
                        }
                    }));
                    if ran.is_err() {
                        hub = None;
                        restart_at = Some(now + restarts.failed(now, "stopped unexpectedly"));
                    }
                }
                if now >= check_at {
                    check_at = now + HEALTH_EVERY;
                    let health = match hub.as_ref() {
                        Some(h) => catch_unwind(AssertUnwindSafe(|| h.health()))
                            .unwrap_or_else(|_| Health::Failed("health check".into())),
                        None => restarts.down(),
                    };
                    if last.as_ref() != Some(&health) {
                        report(health.clone());
                        last = Some(health);
                    }
                }
            }
        })
        .expect("spawning a hub thread")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    struct Fragile(mpsc::Sender<u32>);

    impl Hub for Fragile {
        type Msg = u32;
        fn handle(&mut self, msg: u32) {
            assert!(msg != 0, "boom");
            self.0.send(msg).unwrap();
        }
    }

    #[test]
    fn a_hub_that_panics_is_rebuilt_and_reports_it() {
        let (tx, rx) = mpsc::channel();
        let (htx, health) = mpsc::channel();
        let built = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let b = built.clone();
        let (out, inbox) = mailbox(8);
        let thread = spawn_supervised(
            "fragile",
            move || {
                b.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Fragile(tx.clone())
            },
            inbox,
            move |h| {
                let _ = htx.send(h);
            },
        );
        out.send(1);
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)), Ok(1));
        assert_eq!(health.recv_timeout(Duration::from_secs(2)), Ok(Health::Ok));
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        out.send(0);
        std::thread::sleep(Duration::from_millis(300));
        std::panic::set_hook(hook);
        out.send(2);
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)), Ok(2));
        assert_eq!(built.load(std::sync::atomic::Ordering::SeqCst), 2);
        drop(out);
        thread.join().unwrap();
    }

    struct Echo {
        out: mpsc::Sender<u32>,
        deadline: Option<Instant>,
    }

    impl Hub for Echo {
        type Msg = u32;
        fn handle(&mut self, msg: u32) {
            self.out.send(msg).unwrap();
            if msg == 7 {
                self.deadline = Some(Instant::now() + Duration::from_millis(20));
            }
        }
        fn tick(&mut self, _now: Instant) {
            self.deadline = None;
            self.out.send(1000).unwrap();
        }
        fn next_deadline(&self) -> Option<Instant> {
            self.deadline
        }
    }

    #[test]
    fn a_hub_handles_messages_and_timers_then_stops_when_its_senders_go() {
        let (tx, rx) = mpsc::channel();
        let (outbox, inbox) = mailbox(4);
        let thread = spawn_supervised(
            "echo",
            move || Echo {
                out: tx.clone(),
                deadline: None,
            },
            inbox,
            |_| {},
        );
        outbox.send(3);
        outbox.send(7);
        assert_eq!(rx.recv_timeout(Duration::from_secs(1)), Ok(3));
        assert_eq!(rx.recv_timeout(Duration::from_secs(1)), Ok(7));
        assert_eq!(rx.recv_timeout(Duration::from_secs(1)), Ok(1000));
        drop(outbox);
        thread.join().unwrap();
    }
}
