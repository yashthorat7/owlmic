//! The camera stream between the carrier and the decoder: fragments in, whole frames out in
//! order. H.264 frames can't be skipped one by one, so a decoder that falls behind jumps to the
//! newest keyframe it has, or clears the queue and waits for one.

use super::reassembly::{EncodedFrame, Reassembler};
use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

/// About a quarter second at 30 fps.
const QUEUE: usize = 8;
/// A frame that has waited this long means the decoder is behind.
const MAX_AGE: Duration = Duration::from_millis(200);

/// A frame for the decoder.
pub struct Next {
    pub frame: EncodedFrame,
    /// Newer frames wait behind it: decode this one, but show only the newest.
    pub more: bool,
}

pub struct VideoReceiver {
    reassembler: Mutex<Reassembler>,
    queue: Mutex<VecDeque<(EncodedFrame, Instant)>>,
    ready: Condvar,
    request_keyframe: Box<dyn Fn() + Send + Sync>,
}

impl VideoReceiver {
    /// `request_keyframe` asks the phone for one (through the App and Link Hubs).
    pub fn new(request_keyframe: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            reassembler: Mutex::new(Reassembler::default()),
            queue: Mutex::new(VecDeque::with_capacity(QUEUE)),
            ready: Condvar::new(),
            request_keyframe: Box::new(request_keyframe),
        }
    }

    /// One fragment, payload after the fragment header.
    #[allow(clippy::too_many_arguments)]
    pub fn fragment(
        &self,
        frame: u16,
        index: u16,
        count: u16,
        keyframe: bool,
        timestamp_us: u32,
        data: &[u8],
    ) {
        let pushed = self
            .reassembler
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(
                frame,
                index,
                count,
                keyframe,
                timestamp_us,
                data,
                Instant::now(),
            );
        if pushed.request_keyframe {
            (self.request_keyframe)();
        }
        let Some(f) = pushed.frame else { return };
        let mut q = self.queue.lock().unwrap_or_else(|p| p.into_inner());
        if q.len() == QUEUE && !f.keyframe {
            q.clear();
            drop(q);
            self.resync();
            return;
        }
        if f.keyframe {
            q.clear();
        }
        q.push_back((f, Instant::now()));
        drop(q);
        self.ready.notify_one();
    }

    /// For the decoder thread: the next frame in order, or `None` after `wait`.
    pub fn next_frame(&self, wait: Duration) -> Option<Next> {
        let q = self.queue.lock().unwrap_or_else(|p| p.into_inner());
        let (mut q, _) = self
            .ready
            .wait_timeout_while(q, wait, |q| q.is_empty())
            .unwrap_or_else(|p| p.into_inner());
        skip_stale(&mut q, Instant::now());
        let (frame, _) = q.pop_front()?;
        Some(Next {
            frame,
            more: !q.is_empty(),
        })
    }

    /// The decoder failed: drop what is queued and wait for a keyframe.
    pub fn resync(&self) {
        self.queue.lock().unwrap_or_else(|p| p.into_inner()).clear();
        let mut r = self.reassembler.lock().unwrap_or_else(|p| p.into_inner());
        r.lost_sync();
        if r.take_keyframe_request(Instant::now()) {
            (self.request_keyframe)();
        }
    }
}

/// When the oldest frame has waited past [`MAX_AGE`], everything before the newest keyframe goes.
fn skip_stale(q: &mut VecDeque<(EncodedFrame, Instant)>, now: Instant) {
    if q.front()
        .is_some_and(|(_, at)| now.duration_since(*at) > MAX_AGE)
        && let Some(k) = q.iter().rposition(|(f, _)| f.keyframe)
    {
        q.drain(..k);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn frames_come_out_in_order_and_a_slow_decoder_triggers_a_keyframe_request() {
        let asked = Arc::new(AtomicUsize::new(0));
        let a = asked.clone();
        let rx = VideoReceiver::new(move || {
            a.fetch_add(1, Ordering::SeqCst);
        });
        rx.fragment(1, 0, 1, true, 0, b"k");
        rx.fragment(2, 0, 1, false, 0, b"p");
        let k = rx.next_frame(Duration::ZERO).unwrap();
        assert_eq!((k.frame.data, k.more), (b"k".to_vec(), true));
        let p = rx.next_frame(Duration::ZERO).unwrap();
        assert_eq!((p.frame.data, p.more), (b"p".to_vec(), false));
        for f in 3..3 + QUEUE as u16 + 1 {
            rx.fragment(f, 0, 1, false, 0, b"p");
        }
        assert_eq!(asked.load(Ordering::SeqCst), 1);
        assert!(
            rx.next_frame(Duration::ZERO).is_none(),
            "the queue was dropped"
        );
    }

    #[test]
    fn a_decoder_that_is_behind_jumps_to_the_newest_keyframe() {
        let frame = |data: &[u8], keyframe| EncodedFrame {
            data: data.to_vec(),
            keyframe,
            timestamp_us: 0,
        };
        let t = Instant::now();
        let mut q: VecDeque<_> = [
            (frame(b"p1", false), t),
            (frame(b"k", true), t),
            (frame(b"p2", false), t),
        ]
        .into();
        skip_stale(&mut q, t + Duration::from_millis(100));
        assert_eq!(q.len(), 3, "not behind yet");
        skip_stale(&mut q, t + Duration::from_millis(300));
        assert_eq!(q.front().map(|(f, _)| f.data.clone()), Some(b"k".to_vec()));
        q.pop_front();
        skip_stale(&mut q, t + Duration::from_millis(300));
        assert_eq!(q.len(), 1, "with no keyframe left, every frame is decoded");
    }
}
