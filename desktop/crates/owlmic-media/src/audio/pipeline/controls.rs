//! Processing switches the phone's mic settings own, next to the ones in mod.rs, and the output
//! device's rate.

use super::JitterBuffer;
use crate::audio::dsp::Processing;
use std::sync::atomic::Ordering;

/// `gate_margin_db` when there is no gate.
pub(super) const GATE_OFF: i32 = i32::MIN;

impl JitterBuffer {
    /// PC noise reduction: how much RNNoise is mixed in, in percent, and the noise gate's
    /// margin over the room's noise floor in dB (none: no gate).
    pub fn set_noise(&self, denoise_pct: u32, gate_margin_db: Option<i32>) {
        self.denoise_pct
            .store(denoise_pct.min(100), Ordering::Release);
        self.gate_margin_db
            .store(gate_margin_db.unwrap_or(GATE_OFF), Ordering::Release);
    }

    /// The normalization to a steady speech level ("Auto level").
    pub fn set_auto_level(&self, on: bool) {
        self.agc_enabled.store(on, Ordering::Release);
    }

    /// The user's volume on top of the normalization, in dB.
    pub fn set_volume_db(&self, db: i32) {
        self.volume_db.store(db, Ordering::Release);
    }

    /// What the processing does to the next packet.
    pub fn processing(&self) -> Processing {
        let gate = self.gate_margin_db.load(Ordering::Acquire);
        Processing {
            denoise_pct: self.denoise_pct.load(Ordering::Acquire),
            gate_margin_db: (gate != GATE_OFF).then_some(gate),
            normalize: self.agc_enabled.load(Ordering::Acquire),
            volume_db: self.volume_db.load(Ordering::Acquire),
        }
    }

    /// The output device's sample rate. The phone's 48 kHz is resampled to it on the way out.
    pub fn set_output_rate(&self, rate: u32) {
        self.output_rate.store(rate.max(1), Ordering::Relaxed);
    }
}
