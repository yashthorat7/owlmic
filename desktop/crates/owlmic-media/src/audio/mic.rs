//! The phone's mic into the jitter buffer (SYSTEM_DESIGN section 17.1): PCM on cables, Opus on
//! wireless links, with Opus's own concealment and forward error correction filling losses.

use super::opus::Decoder;
use super::pipeline::JitterBuffer;
use std::sync::{Arc, Mutex};

/// Mono, 10 ms at 48 kHz.
pub const FRAME: usize = 480;
/// The longest Opus frame, 120 ms.
const MAX_FRAME: usize = 5760;
/// More lost frames than this are left to the jitter buffer's fade instead of concealed.
const MAX_CONCEALED: u32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicCodec {
    /// 16-bit little-endian samples, mono, 48 kHz.
    Pcm,
    Opus,
}

struct State {
    on: bool,
    codec: MicCodec,
    decoder: Option<Decoder>,
    next_seq: Option<u32>,
    last_frame: usize,
    pcm: Vec<i16>,
    clock: Unwrap,
}

/// Extends the header's wrapping 32-bit microsecond timestamps.
#[derive(Default)]
struct Unwrap {
    high: u64,
    last: Option<u32>,
}

impl Unwrap {
    fn extend(&mut self, ts: u32) -> u64 {
        if self.last.is_some_and(|l| ts < l && l - ts > 1 << 31) {
            self.high += 1 << 32;
        }
        self.last = Some(ts);
        self.high | ts as u64
    }
}

pub struct MicReceiver {
    jitter: Arc<JitterBuffer>,
    state: Mutex<State>,
}

impl MicReceiver {
    pub fn new(jitter: Arc<JitterBuffer>) -> Self {
        Self {
            jitter,
            state: Mutex::new(State {
                on: false,
                codec: MicCodec::Pcm,
                decoder: None,
                next_seq: None,
                last_frame: FRAME,
                pcm: vec![0; MAX_FRAME],
                clock: Unwrap::default(),
            }),
        }
    }

    /// The phone started its mic stream (STREAM_START). `link` sets the jitter buffer's target.
    pub fn start(&self, codec: MicCodec, link: u8) {
        let mut s = self.lock();
        s.on = true;
        s.codec = codec;
        s.decoder = if codec == MicCodec::Opus {
            Decoder::new(1)
        } else {
            None
        };
        s.next_seq = None;
        s.clock = Unwrap::default();
        self.jitter.reset();
        self.jitter.set_level(link);
    }

    pub fn stop(&self) {
        self.lock().on = false;
        self.jitter.reset();
    }

    pub fn set_link(&self, link: u8) {
        self.jitter.set_level(link);
    }

    pub fn packet(&self, seq: u32, timestamp_us: u32, payload: &[u8]) {
        let mut guard = self.lock();
        let s = &mut *guard;
        if !s.on {
            return;
        }
        let lost = match s.next_seq {
            Some(expected) => {
                let ahead = seq.wrapping_sub(expected);
                if ahead >= 1 << 31 {
                    return; // late or repeated
                }
                ahead
            }
            None => 0,
        };
        s.next_seq = Some(seq.wrapping_add(1));
        let arrival = s.clock.extend(timestamp_us);
        match s.codec {
            MicCodec::Pcm => {
                let n = (payload.len() / 2).min(s.pcm.len());
                for (out, b) in s.pcm.iter_mut().zip(payload.as_chunks::<2>().0) {
                    *out = i16::from_le_bytes(*b);
                }
                self.jitter.record_arrival(arrival);
                self.jitter.push_samples(&mut s.pcm[..n]);
            }
            MicCodec::Opus => {
                let Some(dec) = s.decoder.as_mut() else {
                    return;
                };
                if (1..=MAX_CONCEALED).contains(&lost) {
                    let frame = s.last_frame;
                    for _ in 1..lost {
                        let n = dec.decode(None, &mut s.pcm[..frame], false);
                        self.jitter.push_samples(&mut s.pcm[..n]);
                    }
                    // The lost frame right before this one is rebuilt from this packet's redundancy.
                    let n = dec.decode(Some(payload), &mut s.pcm[..frame], true);
                    self.jitter.push_samples(&mut s.pcm[..n]);
                }
                let n = dec.decode(Some(payload), &mut s.pcm, false);
                if n > 0 {
                    s.last_frame = n;
                    self.jitter.record_arrival(arrival);
                    self.jitter.push_samples(&mut s.pcm[..n]);
                }
            }
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::opus::Encoder;

    fn tone(frame: usize) -> Vec<f32> {
        (0..FRAME)
            .map(|i| {
                (2.0 * std::f32::consts::PI * 300.0 * (frame * FRAME + i) as f32 / 48_000.0).sin()
                    * 0.3
            })
            .collect()
    }

    #[test]
    fn pcm_packets_fill_the_buffer_and_late_ones_are_dropped() {
        let jb = Arc::new(JitterBuffer::new());
        let mic = MicReceiver::new(jb.clone());
        mic.start(MicCodec::Pcm, 1);
        let pcm: Vec<u8> = (0..FRAME as i16).flat_map(|s| s.to_le_bytes()).collect();
        mic.packet(10, 0, &pcm);
        mic.packet(11, 10_000, &pcm);
        mic.packet(10, 0, &pcm);
        assert_eq!(jb.len(), 2 * FRAME);
    }

    #[test]
    fn opus_losses_are_filled_so_timing_holds() {
        let jb = Arc::new(JitterBuffer::new());
        let mic = MicReceiver::new(jb.clone());
        mic.start(MicCodec::Opus, 3);
        let mut enc = Encoder::new(1, 48_000).unwrap();
        let mut buf = [0u8; 1500];
        for f in 0..10u32 {
            let n = enc.encode(&tone(f as usize), &mut buf);
            if f == 4 || f == 5 {
                continue; // lost on the way
            }
            mic.packet(f, f * 10_000, &buf[..n]);
        }
        assert_eq!(
            jb.len(),
            10 * FRAME,
            "two lost frames came back as concealment and FEC"
        );
    }

    #[test]
    fn nothing_is_taken_while_the_mic_is_off() {
        let jb = Arc::new(JitterBuffer::new());
        let mic = MicReceiver::new(jb.clone());
        mic.packet(1, 0, &[0; 960]);
        assert_eq!(jb.len(), 0);
    }
}
