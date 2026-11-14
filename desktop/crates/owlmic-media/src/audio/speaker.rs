//! The PC's sound to the phone (SYSTEM_DESIGN section 17.3): 48 kHz stereo from the loopback
//! capture, in 10 ms packets (20 ms on Bluetooth), as PCM on cables and Opus otherwise.

use super::opus::Encoder;
use std::sync::Mutex;

pub const CHANNELS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeakerCodec {
    /// 16-bit little-endian, interleaved stereo.
    Pcm,
    Opus {
        bitrate: i32,
        frame_ms: u32,
    },
}

impl SpeakerCodec {
    /// The design's choice per link: PCM on cables, Opus 128 kbps on Wi-Fi, 64 kbps in 20 ms
    /// frames on Bluetooth.
    pub fn for_link(link: u8) -> Self {
        match link {
            1 | 2 => SpeakerCodec::Pcm,
            4 => SpeakerCodec::Opus {
                bitrate: 64_000,
                frame_ms: 20,
            },
            _ => SpeakerCodec::Opus {
                bitrate: 128_000,
                frame_ms: 10,
            },
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            SpeakerCodec::Pcm => "pcm",
            SpeakerCodec::Opus { .. } => "opus",
        }
    }

    pub fn frame_ms(&self) -> u32 {
        match self {
            SpeakerCodec::Pcm => 10,
            SpeakerCodec::Opus { frame_ms, .. } => *frame_ms,
        }
    }
}

pub type PacketOut = Box<dyn Fn(u32, &[u8]) + Send + Sync>;

struct State {
    codec: Option<SpeakerCodec>,
    encoder: Option<Encoder>,
    frame: Vec<f32>,
    fill: usize,
    packet: Vec<u8>,
    timestamp_us: u32,
    /// The phone's latest loss, so a new encoder starts with the right amount of FEC.
    loss: u8,
}

pub struct SpeakerSender {
    state: Mutex<State>,
    out: PacketOut,
}

impl SpeakerSender {
    /// `out` gets each packet's timestamp and payload.
    pub fn new(out: PacketOut) -> Self {
        Self {
            state: Mutex::new(State {
                codec: None,
                encoder: None,
                frame: vec![0.0; 48 * 20 * CHANNELS],
                fill: 0,
                packet: vec![0; 4000],
                timestamp_us: 0,
                loss: 0,
            }),
            out,
        }
    }

    pub fn start(&self, codec: SpeakerCodec) {
        let mut s = self.lock();
        s.encoder = match codec {
            SpeakerCodec::Opus { bitrate, .. } => Encoder::new(CHANNELS, bitrate),
            SpeakerCodec::Pcm => None,
        };
        let loss = s.loss;
        if let Some(e) = s.encoder.as_mut() {
            e.set_loss(loss);
        }
        s.codec = Some(codec);
        s.fill = 0;
    }

    pub fn stop(&self) {
        self.lock().codec = None;
    }

    pub fn is_on(&self) -> bool {
        self.lock().codec.is_some()
    }

    /// The loss the phone reports for the speaker stream: Opus adds as much redundancy.
    pub fn set_loss(&self, pct: u8) {
        let mut s = self.lock();
        s.loss = pct.min(100);
        if let Some(e) = s.encoder.as_mut() {
            e.set_loss(pct.min(100));
        }
    }

    #[cfg(test)]
    pub(crate) fn loss(&self) -> u8 {
        self.lock().loss
    }

    /// Interleaved stereo at 48 kHz, in any amount; whole frames go out as packets.
    pub fn push(&self, samples: &[f32]) {
        let mut guard = self.lock();
        let s = &mut *guard;
        let Some(codec) = s.codec else { return };
        let frame_len = 48 * codec.frame_ms() as usize * CHANNELS;
        let mut rest = samples;
        while !rest.is_empty() {
            let take = (frame_len - s.fill).min(rest.len());
            s.frame[s.fill..s.fill + take].copy_from_slice(&rest[..take]);
            s.fill += take;
            rest = &rest[take..];
            if s.fill < frame_len {
                break;
            }
            s.fill = 0;
            let n = match (codec, s.encoder.as_mut()) {
                (SpeakerCodec::Opus { .. }, Some(enc)) => {
                    enc.encode(&s.frame[..frame_len], &mut s.packet)
                }
                _ => {
                    for (i, v) in s.frame[..frame_len].iter().enumerate() {
                        let pcm = (v.clamp(-1.0, 1.0) * 32767.0) as i16;
                        s.packet[i * 2..i * 2 + 2].copy_from_slice(&pcm.to_le_bytes());
                    }
                    frame_len * 2
                }
            };
            if n > 0 {
                (self.out)(s.timestamp_us, &s.packet[..n]);
            }
            s.timestamp_us = s.timestamp_us.wrapping_add(codec.frame_ms() * 1000);
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Timestamp and length of each packet sent.
    type Sent = Arc<Mutex<Vec<(u32, usize)>>>;

    fn sender() -> (SpeakerSender, Sent) {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let s = sent.clone();
        (
            SpeakerSender::new(Box::new(move |ts, p| s.lock().unwrap().push((ts, p.len())))),
            sent,
        )
    }

    #[test]
    fn pcm_goes_out_in_10_ms_packets_with_rising_timestamps() {
        let (sp, sent) = sender();
        sp.start(SpeakerCodec::Pcm);
        sp.push(&vec![0.1; 960 * 2 + 100]);
        assert_eq!(*sent.lock().unwrap(), vec![(0, 1920), (10_000, 1920)]);
        sp.push(&vec![0.1; 860]);
        assert_eq!(sent.lock().unwrap().len(), 3);
    }

    #[test]
    fn opus_on_bluetooth_uses_20_ms_frames_and_nothing_goes_out_when_off() {
        let (sp, sent) = sender();
        sp.push(&[0.0; 960]);
        assert!(sent.lock().unwrap().is_empty());
        sp.start(SpeakerCodec::for_link(4));
        sp.push(&vec![0.2; 1920 * 2]);
        let s = sent.lock().unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[1].0, 20_000);
        assert!(s[0].1 > 0 && s[0].1 < 400);
    }
}
