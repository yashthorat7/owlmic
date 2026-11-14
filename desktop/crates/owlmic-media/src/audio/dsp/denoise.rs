use super::agc::Agc;
use super::floor::Floor;
use super::gate::Gate;
use nnnoiseless::DenoiseState;

/// What the mic's processing does, from the phone's mic settings (see MediaHub's noise plan).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Processing {
    /// RNNoise mixed in, in percent. 0 is off.
    pub denoise_pct: u32,
    /// The noise gate's margin over the room's noise floor in dB, or no gate.
    pub gate_margin_db: Option<i32>,
    /// The normalization (AGC) to a steady speech level.
    pub normalize: bool,
    /// The user's volume on top of the normalization, in dB.
    pub volume_db: i32,
}

impl Default for Processing {
    fn default() -> Self {
        Self {
            denoise_pct: 100,
            gate_margin_db: None,
            normalize: true,
            volume_db: 0,
        }
    }
}

pub struct AudioDsp {
    denoise: Box<DenoiseState<'static>>,
    floor: Floor,
    gate: Gate,
    agc: Agc,
    scratch_in: [f32; DenoiseState::FRAME_SIZE],
    scratch_out: [f32; DenoiseState::FRAME_SIZE],
    /// The frame before the current one. RNNoise's output is one frame (10 ms) late, so this is
    /// the raw audio that lines up with it. Mixing in the current frame instead cancels parts of
    /// the voice and makes it sound metallic.
    dry: [f32; DenoiseState::FRAME_SIZE],
}

impl AudioDsp {
    pub fn new() -> Self {
        Self {
            denoise: DenoiseState::new(),
            floor: Floor::new(),
            gate: Gate::new(48000.0),
            agc: Agc::new(),
            scratch_in: [0.0; DenoiseState::FRAME_SIZE],
            scratch_out: [0.0; DenoiseState::FRAME_SIZE],
            dry: [0.0; DenoiseState::FRAME_SIZE],
        }
    }

    /// Denoise, then the gate, then the normalization and the volume with the soft limiter, one
    /// 10 ms frame at a time: the gate and the normalization both decide from the frame's level
    /// against the room's noise floor. With everything off the audio passes untouched.
    pub fn process(&mut self, samples: &mut [i16], p: Processing) {
        if samples.is_empty() {
            return;
        }

        let strength_ratio = (p.denoise_pct.min(100) as f32) / 100.0;
        let volume = 10.0f32.powf(p.volume_db as f32 / 20.0);
        let leveled = p.normalize || p.volume_db != 0;

        for chunk in samples.chunks_mut(DenoiseState::FRAME_SIZE) {
            if chunk.len() < DenoiseState::FRAME_SIZE {
                continue;
            }

            for (dest, &src) in self.scratch_in.iter_mut().zip(chunk.iter()) {
                *dest = src as f32;
            }

            // Only run denoise if strength is high enough
            let frame = if strength_ratio > 0.001 {
                let _vad = self
                    .denoise
                    .process_frame(&mut self.scratch_out, &self.scratch_in);

                for (&dry, denoised_ref) in self.dry.iter().zip(self.scratch_out.iter_mut()) {
                    let denoised = *denoised_ref;
                    *denoised_ref = dry * (1.0 - strength_ratio) + denoised * strength_ratio;
                }
                self.dry = self.scratch_in;
                &mut self.scratch_out
            } else {
                self.dry = self.scratch_in;
                &mut self.scratch_in
            };

            if p.gate_margin_db.is_none() && !leveled {
                // Nothing measures the room: the floor starts afresh when something does.
                self.floor.reset();
            } else {
                let measured = self.floor.measure(&frame[..]);
                if let Some(margin) = p.gate_margin_db {
                    self.gate
                        .process(&mut frame[..], measured.is_above(margin as f32));
                }
                if leveled {
                    self.agc
                        .process(&mut frame[..], measured, p.normalize, volume);
                }
            }

            for (dest, &processed) in chunk.iter_mut().zip(frame.iter()) {
                *dest = processed.clamp(-32768.0, 32767.0) as i16;
            }
        }
    }

    pub fn reset(&mut self) {
        self.scratch_in.fill(0.0);
        self.scratch_out.fill(0.0);
        self.dry.fill(0.0);
        self.floor.reset();
        self.gate.reset();
        self.agc.reset();
    }
}

impl Default for AudioDsp {
    fn default() -> Self {
        Self::new()
    }
}
