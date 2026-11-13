mod constants;
mod controls;
mod resample;
mod ring;
#[cfg(test)]
mod tests;

pub use constants::*;
use controls::GATE_OFF;

use crate::audio::dsp::{AudioDsp, Processing};
use resample::{JitterStats, Playout};
use ring::SampleRing;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicUsize, Ordering};
use std::time::Instant;

/// The mic's jitter buffer. The receive thread pushes and the output device's thread pops; each
/// lock here belongs to one of the two sides, so neither ever waits on the other.
pub struct JitterBuffer {
    ring: SampleRing,
    base_target_samples: AtomicUsize,
    adaptive_target_samples: AtomicUsize,
    /// The mic settings' processing (see controls.rs); read once per pushed packet.
    denoise_pct: AtomicU32,
    /// The gate's margin over the noise floor in dB, or `GATE_OFF`.
    gate_margin_db: AtomicI32,
    agc_enabled: AtomicBool,
    volume_db: AtomicI32,
    /// Receive side.
    dsp: Mutex<AudioDsp>,
    /// Receive side.
    stats: Mutex<JitterStats>,
    /// Output side.
    playout: Mutex<Playout>,
    output_rate: AtomicU32,
}

impl JitterBuffer {
    pub fn new() -> Self {
        let base_target = USB_TARGET_MS * SAMPLES_PER_MS;
        Self {
            ring: SampleRing::new(),
            base_target_samples: AtomicUsize::new(base_target),
            adaptive_target_samples: AtomicUsize::new(base_target),
            denoise_pct: AtomicU32::new(Processing::default().denoise_pct),
            gate_margin_db: AtomicI32::new(GATE_OFF),
            agc_enabled: AtomicBool::new(true),
            volume_db: AtomicI32::new(0),
            dsp: Mutex::new(AudioDsp::new()),
            stats: Mutex::new(JitterStats::new()),
            playout: Mutex::new(Playout::new()),
            output_rate: AtomicU32::new(SAMPLE_RATE),
        }
    }

    pub fn set_level(&self, level: u8) {
        let target_ms = match level {
            1 | 2 => USB_TARGET_MS,
            3 => WIFI_TARGET_MS,
            4 => BT_TARGET_MS,
            _ => USB_TARGET_MS,
        };
        let samples = target_ms * SAMPLES_PER_MS;
        self.base_target_samples.store(samples, Ordering::Relaxed);
        self.adaptive_target_samples
            .store(samples, Ordering::Relaxed);
    }

    pub fn record_arrival(&self, capture_ts: u64) {
        let now = Instant::now();
        if let Ok(mut stats) = self.stats.lock() {
            if let (Some(last_arr), Some(last_cap)) = (stats.last_arrival, stats.last_capture_ts) {
                let delta_arrival_us = now.duration_since(last_arr).as_micros() as f64;
                let delta_capture_us = capture_ts.saturating_sub(last_cap) as f64;
                let d = (delta_arrival_us - delta_capture_us).abs();
                stats.jitter_estimate_us += (d - stats.jitter_estimate_us) / 16.0;

                let jitter_ms = (stats.jitter_estimate_us / 1000.0) as usize;
                let base = self.base_target_samples.load(Ordering::Relaxed);
                let adaptive_ms = (base / SAMPLES_PER_MS) + (jitter_ms * 2);
                let clamped_ms = adaptive_ms.min(MAX_ADAPTIVE_TARGET_MS);
                self.adaptive_target_samples
                    .store(clamped_ms * SAMPLES_PER_MS, Ordering::Relaxed);
            }
            stats.last_arrival = Some(now);
            stats.last_capture_ts = Some(capture_ts);
        }
    }

    /// Receive side: noise reduction runs on `samples` where they are, then they are queued.
    pub fn push_samples(&self, samples: &mut [i16]) {
        if samples.is_empty() {
            return;
        }
        if let Ok(mut dsp) = self.dsp.lock() {
            dsp.process(samples, self.processing());
        }
        self.ring.push(samples);
    }

    /// Receive side: drops what is queued; the output starts over at its next callback.
    pub fn reset(&self) {
        self.ring.flush();
        if let Ok(mut dsp) = self.dsp.lock() {
            dsp.reset();
        }
    }

    pub fn target_samples(&self) -> usize {
        self.adaptive_target_samples.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.ring.len()
    }
}

impl Default for JitterBuffer {
    fn default() -> Self {
        Self::new()
    }
}
