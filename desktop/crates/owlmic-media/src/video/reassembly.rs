//! Video fragments back into whole H.264 frames (SYSTEM_DESIGN section 17.2). A frame missing a
//! fragment after two frame intervals is dropped, and then only a keyframe can restart the
//! picture, so one is requested, at most every 500 ms.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

const KEYFRAME_REQUEST_GAP: Duration = Duration::from_millis(500);
/// Two frame intervals at the slowest frame rate the phone sends (15 fps when it is hot).
const GIVE_UP_AFTER: Duration = Duration::from_millis(133);
const MAX_PARTIAL: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedFrame {
    pub data: Vec<u8>,
    pub keyframe: bool,
    pub timestamp_us: u32,
}

struct Partial {
    parts: Vec<Option<Vec<u8>>>,
    missing: usize,
    keyframe: bool,
    timestamp_us: u32,
    first_seen: Instant,
}

#[derive(Default)]
pub struct Reassembler {
    partial: BTreeMap<u16, Partial>,
    last_done: Option<u16>,
    need_keyframe: bool,
    last_request: Option<Instant>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Pushed {
    pub frame: Option<EncodedFrame>,
    pub request_keyframe: bool,
}

/// True when frame id `a` comes after `b`, with wrapping.
fn after(a: u16, b: u16) -> bool {
    a != b && a.wrapping_sub(b) < 1 << 15
}

impl Reassembler {
    /// One fragment: `frame`, `index` and `count` from its fragment header.
    #[allow(clippy::too_many_arguments)]
    pub fn push(
        &mut self,
        frame: u16,
        index: u16,
        count: u16,
        keyframe: bool,
        timestamp_us: u32,
        data: &[u8],
        now: Instant,
    ) -> Pushed {
        let mut out = Pushed {
            frame: None,
            request_keyframe: self.expire(now),
        };
        if self.last_done.is_some_and(|d| !after(frame, d)) || index >= count {
            return self.finish(out, now);
        }
        let p = self.partial.entry(frame).or_insert_with(|| Partial {
            parts: vec![None; count as usize],
            missing: count as usize,
            keyframe,
            timestamp_us,
            first_seen: now,
        });
        if p.parts.len() != count as usize {
            return self.finish(out, now);
        }
        if p.parts[index as usize].is_none() {
            p.parts[index as usize] = Some(data.to_vec());
            p.missing -= 1;
        }
        if p.missing > 0 {
            if self.partial.len() > MAX_PARTIAL {
                let oldest = *self.partial.keys().next().unwrap();
                self.partial.remove(&oldest);
                self.need_keyframe = true;
                out.request_keyframe = true;
            }
            return self.finish(out, now);
        }
        let done = self.partial.remove(&frame).unwrap();
        // Anything older that is still incomplete can never be decoded now.
        let stale: Vec<u16> = self
            .partial
            .keys()
            .copied()
            .filter(|f| !after(*f, frame))
            .collect();
        if !stale.is_empty() {
            self.need_keyframe = true;
            out.request_keyframe = true;
            for f in stale {
                self.partial.remove(&f);
            }
        }
        self.last_done = Some(frame);
        if self.need_keyframe && !done.keyframe {
            out.request_keyframe = true;
            return self.finish(out, now);
        }
        if done.keyframe {
            self.need_keyframe = false;
        }
        let data = done.parts.into_iter().flatten().flatten().collect();
        out.frame = Some(EncodedFrame {
            data,
            keyframe: done.keyframe,
            timestamp_us: done.timestamp_us,
        });
        self.finish(out, now)
    }

    /// The decoder failed or fell behind: wait for a keyframe.
    pub fn lost_sync(&mut self) {
        self.need_keyframe = true;
    }

    /// Asks for a keyframe now, unless one was asked for less than 500 ms ago.
    pub fn take_keyframe_request(&mut self, now: Instant) -> bool {
        if self
            .last_request
            .is_some_and(|t| now.duration_since(t) < KEYFRAME_REQUEST_GAP)
        {
            return false;
        }
        self.last_request = Some(now);
        true
    }

    fn expire(&mut self, now: Instant) -> bool {
        let before = self.partial.len();
        self.partial
            .retain(|_, p| now.duration_since(p.first_seen) < GIVE_UP_AFTER);
        if self.partial.len() < before {
            self.need_keyframe = true;
            return true;
        }
        false
    }

    fn finish(&mut self, mut out: Pushed, now: Instant) -> Pushed {
        if out.request_keyframe {
            out.request_keyframe = self.take_keyframe_request(now);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragments_in_any_order_make_one_frame() {
        let t = Instant::now();
        let mut r = Reassembler::default();
        assert_eq!(r.push(1, 1, 2, true, 7, b"cd", t), Pushed::default());
        let p = r.push(1, 0, 2, true, 7, b"ab", t);
        assert_eq!(
            p.frame,
            Some(EncodedFrame {
                data: b"abcd".to_vec(),
                keyframe: true,
                timestamp_us: 7
            })
        );
    }

    #[test]
    fn a_lost_fragment_waits_for_a_keyframe_and_asks_for_one_at_most_every_500_ms() {
        let t = Instant::now();
        let mut r = Reassembler::default();
        r.push(1, 0, 1, true, 0, b"k", t);
        r.push(2, 0, 2, false, 0, b"x", t);
        let p = r.push(3, 0, 1, false, 0, b"p", t + Duration::from_millis(10));
        assert_eq!(
            p.frame, None,
            "frame 2 never completed, so frame 3 can't decode"
        );
        assert!(p.request_keyframe);
        let p = r.push(4, 0, 1, false, 0, b"p", t + Duration::from_millis(20));
        assert!(!p.request_keyframe, "throttled");
        let p = r.push(5, 0, 1, true, 0, b"K", t + Duration::from_millis(30));
        assert_eq!(p.frame.map(|f| f.data), Some(b"K".to_vec()));
        assert!(
            r.push(6, 0, 1, false, 0, b"p", t + Duration::from_millis(40))
                .frame
                .is_some()
        );
    }

    #[test]
    fn an_incomplete_frame_expires_after_two_intervals() {
        let t = Instant::now();
        let mut r = Reassembler::default();
        r.push(1, 0, 2, false, 0, b"x", t);
        let p = r.push(9, 0, 2, false, 0, b"y", t + Duration::from_millis(200));
        assert!(p.request_keyframe);
    }

    #[test]
    fn frame_ids_wrap() {
        assert!(after(0, u16::MAX));
        assert!(!after(u16::MAX, 0));
    }
}
