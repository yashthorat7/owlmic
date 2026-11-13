//! A bounded queue that never blocks a hub: when it is full, a message marked droppable (a view,
//! a report, anything a newer message supersedes) makes room. Lifecycle and decision messages are
//! never dropped, so a burst of them can take a mailbox past its capacity.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

struct Shared<T> {
    queue: Mutex<State<T>>,
    /// A message arrived, or the last sender left.
    ready: Condvar,
    /// The hub took a message, or stopped receiving.
    space: Condvar,
    capacity: usize,
    droppable: fn(&T) -> bool,
}

struct State<T> {
    items: VecDeque<T>,
    senders: usize,
    receiving: bool,
}

pub struct Outbox<T> {
    shared: Arc<Shared<T>>,
}

pub struct Inbox<T> {
    shared: Arc<Shared<T>>,
}

pub(crate) enum Recv<T> {
    Msg(T),
    Deadline,
    Closed,
}

/// A mailbox none of whose messages may be dropped.
pub fn mailbox<T>(capacity: usize) -> (Outbox<T>, Inbox<T>) {
    mailbox_with(capacity, |_| false)
}

/// A mailbox that may drop the messages `droppable` picks when it is full.
pub fn mailbox_with<T>(capacity: usize, droppable: fn(&T) -> bool) -> (Outbox<T>, Inbox<T>) {
    assert!(capacity > 0);
    let shared = Arc::new(Shared {
        queue: Mutex::new(State {
            items: VecDeque::with_capacity(capacity),
            senders: 1,
            receiving: true,
        }),
        ready: Condvar::new(),
        space: Condvar::new(),
        capacity,
        droppable,
    });
    (
        Outbox {
            shared: shared.clone(),
        },
        Inbox { shared },
    )
}

impl<T> Shared<T> {
    fn lock(&self) -> std::sync::MutexGuard<'_, State<T>> {
        self.queue.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl<T> Outbox<T> {
    /// Queues `msg` without blocking. A full mailbox drops its oldest droppable message, or `msg`
    /// itself when that is droppable and nothing else is.
    pub fn send(&self, msg: T) {
        let mut s = self.shared.lock();
        if !s.receiving {
            return;
        }
        if s.items.len() >= self.shared.capacity {
            let droppable = self.shared.droppable;
            match s.items.iter().position(droppable) {
                Some(i) => {
                    s.items.remove(i);
                }
                None if droppable(&msg) => return,
                None => {}
            }
        }
        s.items.push_back(msg);
        drop(s);
        self.shared.ready.notify_one();
    }

    /// For a producer that may wait, such as a connection's reader: waits while the mailbox is
    /// full, so a flood from the network can't grow it.
    pub fn send_wait(&self, msg: T) {
        let mut s = self.shared.lock();
        while s.receiving && s.items.len() >= self.shared.capacity {
            s = self.shared.space.wait(s).unwrap_or_else(|p| p.into_inner());
        }
        drop(s);
        self.send(msg);
    }
}

impl<T> Clone for Outbox<T> {
    fn clone(&self) -> Self {
        self.shared.lock().senders += 1;
        Self {
            shared: self.shared.clone(),
        }
    }
}

impl<T> Drop for Outbox<T> {
    fn drop(&mut self) {
        let mut s = self.shared.lock();
        s.senders -= 1;
        if s.senders == 0 {
            drop(s);
            self.shared.ready.notify_all();
        }
    }
}

impl<T> Inbox<T> {
    pub(crate) fn recv_until(&self, deadline: Option<Instant>) -> Recv<T> {
        let mut s = self.shared.lock();
        loop {
            if let Some(m) = s.items.pop_front() {
                drop(s);
                self.shared.space.notify_one();
                return Recv::Msg(m);
            }
            if s.senders == 0 {
                return Recv::Closed;
            }
            match deadline {
                None => s = self.shared.ready.wait(s).unwrap_or_else(|p| p.into_inner()),
                Some(d) => {
                    let now = Instant::now();
                    if d <= now {
                        return Recv::Deadline;
                    }
                    s = self
                        .shared
                        .ready
                        .wait_timeout(s, d - now)
                        .unwrap_or_else(|p| p.into_inner())
                        .0;
                }
            }
        }
    }

    /// Takes a waiting message without blocking.
    pub fn try_recv(&self) -> Option<T> {
        let m = self.shared.lock().items.pop_front();
        self.shared.space.notify_one();
        m
    }
}

impl<T> Drop for Inbox<T> {
    fn drop(&mut self) {
        let mut s = self.shared.lock();
        s.receiving = false;
        s.items.clear();
        drop(s);
        self.shared.space.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Odd numbers stand for views and reports, even ones for lifecycle messages.
    fn odd(n: &u32) -> bool {
        n % 2 == 1
    }

    #[test]
    fn a_full_mailbox_drops_droppable_messages_and_never_critical_ones() {
        let (tx, rx) = mailbox_with(3, odd);
        tx.send(1);
        tx.send(2);
        tx.send(4);
        tx.send(6);
        tx.send(8);
        tx.send(3);
        let got: Vec<u32> = std::iter::from_fn(|| rx.try_recv()).collect();
        assert_eq!(
            got,
            vec![2, 4, 6, 8],
            "both views went, every lifecycle message stayed"
        );
    }

    #[test]
    fn a_plain_mailbox_never_drops() {
        let (tx, rx) = mailbox(1);
        for n in 0..5u32 {
            tx.send(n);
        }
        assert_eq!(std::iter::from_fn(|| rx.try_recv()).count(), 5);
    }

    #[test]
    fn a_waiting_sender_goes_on_once_there_is_room() {
        let (tx, rx) = mailbox(1);
        tx.send(1u32);
        let t = std::thread::spawn(move || tx.send_wait(2));
        std::thread::sleep(Duration::from_millis(30));
        assert!(!t.is_finished(), "the mailbox is full");
        assert_eq!(rx.try_recv(), Some(1));
        t.join().unwrap();
        assert_eq!(rx.try_recv(), Some(2));
    }

    #[test]
    fn receiving_waits_for_a_deadline_and_ends_when_senders_go() {
        let (tx, rx) = mailbox::<u8>(1);
        let start = Instant::now();
        assert!(matches!(
            rx.recv_until(Some(start + Duration::from_millis(15))),
            Recv::Deadline
        ));
        assert!(start.elapsed() >= Duration::from_millis(15));
        let tx2 = tx.clone();
        drop(tx);
        tx2.send(9);
        assert!(matches!(rx.recv_until(None), Recv::Msg(9)));
        drop(tx2);
        assert!(matches!(rx.recv_until(None), Recv::Closed));
    }
}
