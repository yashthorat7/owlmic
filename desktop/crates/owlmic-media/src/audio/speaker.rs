//! The PC's sound to the phone (SYSTEM_DESIGN section 17.3): 48 kHz stereo from the loopback
//! capture, streamed as compact Opus frames (10 ms on IP/USB, 20 ms on Bluetooth) with
//! adaptive bitrate so packets stay tiny and MTU-safe while audio stays smooth.

use super::opus::Encoder;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const CHANNELS: usize = 2;
const ADAPT_DOWN_EVERY: Duration = Duration::from_secs(2);
const ADAPT_UP_EVERY: Duration = Duration::from_secs(6);
const CLEAN_REPORTS_UP: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpeakerCodec {
    pub bitrate: i32,
    pub min_bitrate: i32,
    pub max_bitrate: i32,
    pub frame_ms: u32,
}

impl SpeakerCodec {
    /// Smoothness-first tiny packets per link: 96 kbps on cables (~120 B), 64 kbps on Wi-Fi
    /// (~80 B), 48 kbps in 20 ms frames on Bluetooth.
    pub fn for_link(link: u8) -> Self {
        match link {
            1 | 2 => Self {
                bitrate: 96_000,
                min_bitrate: 48_000,
                max_bitrate: 96_000,
                frame_ms: 10,
            },
            4 => Self {
                bitrate: 48_000,
                min_bitrate: 24_000,
                max_bitrate: 64_000,
                frame_ms: 20,
            },
            _ => Self {
                bitrate: 64_000,
                min_bitrate: 32_000,
                max_bitrate: 96_000,
                frame_ms: 10,
            },
        }
    }

    pub fn name(&self) -> &'static str {
        "opus"
    }

    pub fn frame_ms(&self) -> u32 {
        self.frame_ms
    }
}

pub type PacketOut = Box<dyn Fn(u32, &[u8]) + Send + Sync>;

struct State {
    codec: Option<SpeakerCodec>,
    encoder: Option<Encoder>,
    frame: Vec<f32>,
    fill: usize,
    timestamp_us: u32,
    /// The phone's latest loss, so a new encoder starts with the right amount of FEC.
    loss: u8,
    bitrate: i32,
    clean_reports: u32,
    last_adapt: Option<Instant>,
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
                timestamp_us: 0,
                loss: 0,
                bitrate: 64_000,
                clean_reports: 0,
                last_adapt: None,
            }),
            out,
        }
    }

    pub fn start(&self, codec: SpeakerCodec) {
        let mut s = self.lock();
        s.encoder = Encoder::new(CHANNELS, codec.bitrate);
        let loss = s.loss;
        if let Some(e) = s.encoder.as_mut() {
            e.set_loss(loss);
        }
        s.codec = Some(codec);
        s.bitrate = codec.bitrate;
        s.clean_reports = 0;
        s.last_adapt = None;
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
        let loss = pct.min(100);
        s.loss = loss;
        if let Some(e) = s.encoder.as_mut() {
            e.set_loss(loss);
        }
    }

    /// Adapts Opus FEC and bitrate from the phone's telemetry without restarting the stream.
    pub fn report(&self, loss_pct: f64, jitter_ms: u32, rtt_ms: u32) {
        self.report_at(loss_pct, jitter_ms, rtt_ms, Instant::now());
    }

    fn report_at(&self, loss_pct: f64, jitter_ms: u32, rtt_ms: u32, now: Instant) {
        let mut s = self.lock();
        s.loss = loss_pct.clamp(0.0, 100.0).round() as u8;
        let loss = s.loss;
        if let Some(e) = s.encoder.as_mut() {
            e.set_loss(loss);
        }
        let Some(codec) = s.codec else { return };
        let stressed = loss_pct > 2.0 || jitter_ms > 25 || rtt_ms > 120;
        if stressed {
            s.clean_reports = 0;
            if s.last_adapt
                .is_none_or(|t| now.duration_since(t) >= ADAPT_DOWN_EVERY)
            {
                let next = ((s.bitrate * 3) / 4).max(codec.min_bitrate);
                if next != s.bitrate {
                    s.bitrate = next;
                    s.last_adapt = Some(now);
                    if let Some(e) = s.encoder.as_mut() {
                        e.set_bitrate(next);
                    }
                }
            }
            return;
        }
        if loss_pct < 1.0 && jitter_ms <= 15 {
            s.clean_reports = s.clean_reports.saturating_add(1);
            if s.clean_reports >= CLEAN_REPORTS_UP
                && s.last_adapt
                    .is_none_or(|t| now.duration_since(t) >= ADAPT_UP_EVERY)
            {
                let next = ((s.bitrate * 115) / 100).min(codec.max_bitrate);
                if next != s.bitrate {
                    s.bitrate = next;
                    s.clean_reports = 0;
                    s.last_adapt = Some(now);
                    if let Some(e) = s.encoder.as_mut() {
                        e.set_bitrate(next);
                    }
                }
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn loss(&self) -> u8 {
        self.lock().loss
    }

    #[cfg(test)]
    pub(crate) fn bitrate(&self) -> i32 {
        self.lock().bitrate
    }

    /// Interleaved stereo at 48 kHz, in any amount; whole frames go out as compact Opus packets.
    pub fn push(&self, samples: &[f32]) {
        let mut rest = samples;
        while !rest.is_empty() {
            let mut pkt = [0u8; 1500];
            let (ts, len) = {
                let mut guard = self.lock();
                let s = &mut *guard;
                let Some(codec) = s.codec else { return };
                let frame_len = 48 * codec.frame_ms() as usize * CHANNELS;
                let take = (frame_len - s.fill).min(rest.len());
                s.frame[s.fill..s.fill + take].copy_from_slice(&rest[..take]);
                s.fill += take;
                rest = &rest[take..];
                if s.fill < frame_len {
                    break;
                }
                s.fill = 0;
                let ts = s.timestamp_us;
                s.timestamp_us = s.timestamp_us.wrapping_add(codec.frame_ms() * 1000);
                let n = s
                    .encoder
                    .as_mut()
                    .map_or(0, |enc| enc.encode(&s.frame[..frame_len], &mut pkt));
                (ts, n.min(pkt.len()))
            };
            if len > 0 {
                (self.out)(ts, &pkt[..len]);
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
    fn opus_goes_out_in_tiny_10_ms_packets_with_rising_timestamps() {
        let (sp, sent) = sender();
        sp.start(SpeakerCodec::for_link(1));
        sp.push(&vec![0.1; 960 * 2 + 100]);
        {
            let s = sent.lock().unwrap();
            assert_eq!(s.len(), 2);
            assert_eq!(s[0].0, 0);
            assert_eq!(s[1].0, 10_000);
            assert!(s[0].1 > 0 && s[0].1 < 250, "packet size {}", s[0].1);
        }
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

    #[test]
    fn report_adapts_bitrate_down_under_loss_and_recovers_when_clean() {
        let (sp, _) = sender();
        let codec = SpeakerCodec::for_link(3);
        sp.start(codec);
        let t0 = Instant::now();
        sp.report_at(5.0, 30, 50, t0);
        assert!(sp.bitrate() < codec.bitrate);
        assert_eq!(sp.loss(), 5);
        let low = sp.bitrate();
        for i in 1..=3 {
            sp.report_at(0.0, 5, 20, t0 + Duration::from_secs(i * 3));
        }
        assert!(sp.bitrate() > low);
    }
}
