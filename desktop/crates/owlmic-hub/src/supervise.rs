//! Supervision (SYSTEM_DESIGN section 11.3): a hub that panics is rebuilt, and a long-lived I/O
//! thread that stops is started again, each after a backoff that grows while it keeps failing. A
//! unit that keeps failing is reported as failed and still retried, slowly, since a port that
//! was taken or a radio that was off can come back.

use crate::Health;
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant};

const BACKOFF: [Duration; 5] = [
    Duration::from_millis(100),
    Duration::from_millis(500),
    Duration::from_secs(2),
    Duration::from_secs(10),
    Duration::from_secs(30),
];
/// Failures in a row before a unit counts as failed rather than restarting.
pub const FAILED_AFTER: u32 = 3;
/// A run this long clears the count of failures in a row.
const STABLE: Duration = Duration::from_secs(60);

pub(crate) struct Restarts {
    in_a_row: u32,
    started: Instant,
    reason: String,
}

impl Restarts {
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            in_a_row: 0,
            started: now,
            reason: String::new(),
        }
    }

    pub(crate) fn started(&mut self, now: Instant) {
        self.started = now;
    }

    /// Records a failure. Returns how long to wait before starting again.
    pub(crate) fn failed(&mut self, now: Instant, reason: &str) -> Duration {
        if now.duration_since(self.started) >= STABLE {
            self.in_a_row = 0;
        }
        let wait = BACKOFF[(self.in_a_row as usize).min(BACKOFF.len() - 1)];
        self.in_a_row += 1;
        reason.clone_into(&mut self.reason);
        wait
    }

    /// The health of a unit that is down.
    pub(crate) fn down(&self) -> Health {
        if self.in_a_row >= FAILED_AFTER {
            Health::Failed(self.reason.clone())
        } else {
            Health::Degraded(self.reason.clone())
        }
    }
}

/// Runs `run` on its own thread for the life of the app, starting it again whenever it returns or
/// panics. `run` gets `up` to call once it is set up and working (a socket bound, say); its
/// error says why it stopped. Health goes to `report` when it changes.
pub fn supervise(
    name: &str,
    mut run: impl FnMut(&dyn Fn()) -> Result<(), String> + Send + 'static,
    report: impl Fn(Health) + Send + 'static,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || {
            let last = RefCell::new(None::<Health>);
            let say = |h: Health| {
                if last.borrow().as_ref() != Some(&h) {
                    report(h.clone());
                    *last.borrow_mut() = Some(h);
                }
            };
            let up = || say(Health::Ok);
            let mut restarts = Restarts::new(Instant::now());
            loop {
                restarts.started(Instant::now());
                let reason = match catch_unwind(AssertUnwindSafe(|| run(&up))) {
                    Ok(Ok(())) => "stopped".to_owned(),
                    Ok(Err(e)) => e,
                    Err(_) => "stopped unexpectedly".to_owned(),
                };
                let wait = restarts.failed(Instant::now(), &reason);
                say(restarts.down());
                std::thread::sleep(wait);
            }
        })
        .expect("spawning a supervised thread")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn the_backoff_grows_while_failing_and_resets_after_a_stable_run() {
        let t = Instant::now();
        let mut r = Restarts::new(t);
        assert_eq!(r.failed(t, "port"), Duration::from_millis(100));
        assert_eq!(r.down(), Health::Degraded("port".into()));
        assert_eq!(r.failed(t, "port"), Duration::from_millis(500));
        assert_eq!(r.failed(t, "port"), Duration::from_secs(2));
        assert_eq!(r.down(), Health::Failed("port".into()));
        for _ in 0..5 {
            r.failed(t, "port");
        }
        assert_eq!(r.failed(t, "port"), Duration::from_secs(30), "capped");
        r.started(t);
        assert_eq!(
            r.failed(t + STABLE, "again"),
            Duration::from_millis(100),
            "a minute of working clears the count"
        );
    }

    #[test]
    fn a_thread_that_stops_is_started_again_and_says_when_it_is_up() {
        let (tx, rx) = mpsc::channel();
        let mut runs = 0;
        let _ = supervise(
            "flaky",
            move |up| {
                runs += 1;
                if runs < 3 {
                    return Err("busy".into());
                }
                up();
                loop {
                    std::thread::park();
                }
            },
            move |h| tx.send(h).unwrap(),
        );
        let wait = Duration::from_secs(3);
        assert_eq!(rx.recv_timeout(wait), Ok(Health::Degraded("busy".into())));
        assert_eq!(rx.recv_timeout(wait), Ok(Health::Ok), "repeats aren't sent");
    }
}
