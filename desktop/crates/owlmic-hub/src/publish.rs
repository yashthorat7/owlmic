//! The App Hub publishes one immutable state snapshot; the UI thread reads the newest one. A
//! burst of changes wakes the UI once, so it redraws at most as often as it can read.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub type Snapshot<S> = Arc<S>;

pub struct Publisher<S> {
    latest: Arc<Mutex<Snapshot<S>>>,
    waiting: Arc<AtomicBool>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl<S> Clone for Publisher<S> {
    fn clone(&self) -> Self {
        Self {
            latest: self.latest.clone(),
            waiting: self.waiting.clone(),
            wake: self.wake.clone(),
        }
    }
}

impl<S> Publisher<S> {
    /// `wake` tells the UI thread a new snapshot is waiting (on Windows, a posted message).
    pub fn new(initial: S, wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            latest: Arc::new(Mutex::new(Arc::new(initial))),
            waiting: Arc::new(AtomicBool::new(false)),
            wake: Arc::new(wake),
        }
    }

    pub fn publish(&self, state: S) {
        *self.latest.lock().unwrap_or_else(|p| p.into_inner()) = Arc::new(state);
        if !self.waiting.swap(true, Ordering::AcqRel) {
            (self.wake)();
        }
    }

    /// For the UI thread: the newest snapshot. Clears the wake-up so the next change wakes it again.
    pub fn read(&self) -> Snapshot<S> {
        self.waiting.store(false, Ordering::Release);
        self.latest
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn a_burst_of_changes_wakes_the_reader_once() {
        let wakes = Arc::new(AtomicUsize::new(0));
        let w = wakes.clone();
        let p = Publisher::new(0u32, move || {
            w.fetch_add(1, Ordering::SeqCst);
        });
        for i in 1..=10 {
            p.publish(i);
        }
        assert_eq!(wakes.load(Ordering::SeqCst), 1);
        assert_eq!(*p.read(), 10);
        p.publish(11);
        assert_eq!(wakes.load(Ordering::SeqCst), 2);
    }
}
