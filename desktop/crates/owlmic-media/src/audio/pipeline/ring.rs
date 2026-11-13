//! The phone's samples between the receive thread (the only producer) and the output device's
//! thread (the only consumer), without locks: each side moves only its own index, so the render
//! thread never waits on the receive path. Indices count samples from the start and never wrap.

use super::constants::RING;
use std::sync::atomic::{AtomicBool, AtomicI16, AtomicU64, Ordering};

const MASK: u64 = RING as u64 - 1;

pub(crate) struct SampleRing {
    slots: Box<[AtomicI16]>,
    head: AtomicU64,
    tail: AtomicU64,
    /// Set by the producer after an overflow or a reset: the consumer drops everything before
    /// `flush_to` and starts playback over.
    flush: AtomicBool,
    flush_to: AtomicU64,
}

impl SampleRing {
    pub(crate) fn new() -> Self {
        Self {
            slots: (0..RING).map(|_| AtomicI16::new(0)).collect(),
            head: AtomicU64::new(0),
            tail: AtomicU64::new(0),
            flush: AtomicBool::new(false),
            flush_to: AtomicU64::new(0),
        }
    }

    /// Producer side. A ring with no room means the consumer stopped; what it holds is stale, so
    /// it is flushed and `samples` dropped.
    pub(crate) fn push(&self, samples: &[i16]) {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        if (tail - head) as usize + samples.len() > RING {
            self.flush();
            return;
        }
        for (i, s) in (tail..).zip(samples) {
            self.slots[(i & MASK) as usize].store(*s, Ordering::Relaxed);
        }
        self.tail
            .store(tail + samples.len() as u64, Ordering::Release);
    }

    /// Producer side: the consumer drops everything queued so far at its next read.
    pub(crate) fn flush(&self) {
        self.flush_to
            .store(self.tail.load(Ordering::Relaxed), Ordering::Relaxed);
        self.flush.store(true, Ordering::Release);
    }

    /// Consumer side: what is queued, for one output callback. Returns whether a flush was asked
    /// for, so playback starts over.
    pub(crate) fn read(&self) -> (Queued<'_>, bool) {
        // The flag first: seeing it guarantees the tail read next is at or past `flush_to`.
        let flushed = self.flush.swap(false, Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        let mut head = self.head.load(Ordering::Relaxed);
        if flushed {
            head = head.max(self.flush_to.load(Ordering::Relaxed));
        }
        (
            Queued {
                ring: self,
                head,
                tail,
            },
            flushed,
        )
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        let mut head = self.head.load(Ordering::Acquire);
        if self.flush.load(Ordering::Acquire) {
            head = head.max(self.flush_to.load(Ordering::Relaxed));
        }
        (self.tail.load(Ordering::Acquire) - head) as usize
    }
}

/// The consumer's view of the queued samples. Taking samples moves the shared read index when
/// the view is dropped.
pub(crate) struct Queued<'a> {
    ring: &'a SampleRing,
    head: u64,
    tail: u64,
}

impl Queued<'_> {
    pub(crate) fn len(&self) -> usize {
        (self.tail - self.head) as usize
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn get(&self, i: usize) -> Option<i16> {
        (i < self.len()).then(|| {
            self.ring.slots[((self.head + i as u64) & MASK) as usize].load(Ordering::Relaxed)
        })
    }

    pub(crate) fn pop_front(&mut self) -> Option<i16> {
        let s = self.get(0)?;
        self.head += 1;
        Some(s)
    }

    /// Drops the oldest `n` samples.
    pub(crate) fn skip(&mut self, n: usize) {
        self.head += n.min(self.len()) as u64;
    }
}

impl Drop for Queued<'_> {
    fn drop(&mut self) {
        self.ring.head.store(self.head, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_come_out_in_order_across_the_wrap() {
        let r = SampleRing::new();
        for round in 0..3 {
            let block: Vec<i16> = (0..RING as i16 / 2 + 7).map(|i| i ^ round).collect();
            r.push(&block);
            let (mut q, flushed) = r.read();
            assert!(!flushed);
            assert_eq!(q.len(), block.len());
            assert!(block.iter().all(|s| q.pop_front() == Some(*s)));
            assert!(q.is_empty());
        }
    }

    #[test]
    fn an_overflow_flushes_and_a_reset_does_too() {
        let r = SampleRing::new();
        r.push(&vec![1; RING]);
        r.push(&[2]);
        let (q, flushed) = r.read();
        assert!(flushed && q.is_empty());
        drop(q);
        r.push(&[3, 4]);
        r.flush();
        r.push(&[5]);
        let (q, flushed) = r.read();
        assert!(flushed);
        assert_eq!(
            (q.len(), q.get(0)),
            (1, Some(5)),
            "what came after the reset stays"
        );
    }
}
