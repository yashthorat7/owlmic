//! Noise gate for microphone audio: while no frame stands above the room's noise floor by the
//! gate's margin, the mic fades down, so the room noise left between words goes quiet. It decides
//! against the floor (floor.rs), not a fixed level, so a far voice passes as well as a near one.
//! It stays open 150 ms after the last loud frame, through the short gaps inside speech.

/// How far a closed gate turns the mic down: -30 dB.
const FLOOR: f32 = 0.0316;

/// Frames (10 ms) the gate stays open after the last frame above the margin.
const HOLD_FRAMES: u32 = 15;

pub struct Gate {
    gain: f32,
    quiet_frames: u32,
    open_coef: f32,
    close_coef: f32,
}

impl Gate {
    pub fn new(sample_rate: f32) -> Self {
        let coef = |seconds: f32| (-1.0 / (seconds * sample_rate)).exp();
        Self {
            gain: 1.0,
            quiet_frames: HOLD_FRAMES,
            open_coef: coef(0.002), // 2 ms to open, so no word loses its start
            close_coef: coef(0.060), // 60 ms to close, so it fades rather than cuts
        }
    }

    /// Processes one frame in place (16-bit PCM values as floats). `loud` says the frame stands
    /// above the floor by the gate's margin.
    pub fn process(&mut self, samples: &mut [f32], loud: bool) {
        self.quiet_frames = if loud {
            0
        } else {
            self.quiet_frames.saturating_add(1)
        };
        let target = if self.quiet_frames < HOLD_FRAMES {
            1.0
        } else {
            FLOOR
        };
        for sample in samples.iter_mut() {
            if !sample.is_finite() {
                *sample = 0.0;
                continue;
            }
            let smooth = if target > self.gain {
                self.open_coef
            } else {
                self.close_coef
            };
            self.gain = smooth * self.gain + (1.0 - smooth) * target;
            *sample *= self.gain;
        }
    }

    pub fn reset(&mut self) {
        self.gain = 1.0;
        self.quiet_frames = HOLD_FRAMES;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quiet_room_fades_down() {
        let mut gate = Gate::new(48000.0);
        let mut last = 0.0;
        for _ in 0..100 {
            let mut noise = vec![100.0; 480];
            gate.process(&mut noise, false);
            last = noise[479];
        }
        assert!(last < 100.0 * FLOOR * 1.1, "noise stayed at {last}");
    }

    #[test]
    fn speech_passes_whole_and_holds_through_a_short_gap() {
        let mut gate = Gate::new(48000.0);
        let mut speech = vec![3000.0; 480];
        gate.process(&mut speech, true);
        assert!((speech[479] - 3000.0).abs() < 1.0);
        for _ in 0..HOLD_FRAMES - 1 {
            let mut gap = vec![200.0; 480];
            gate.process(&mut gap, false);
            assert!((gap[479] - 200.0).abs() < 1.0, "closed inside the hold");
        }
    }

    #[test]
    fn a_closed_gate_opens_within_a_few_milliseconds() {
        let mut gate = Gate::new(48000.0);
        for _ in 0..30 {
            gate.process(&mut [50.0f32; 480], false);
        }
        let mut word = vec![3000.0; 480];
        gate.process(&mut word, true);
        assert!(
            word[288] > 2700.0,
            "6 ms into the word it is at {}",
            word[288]
        );
    }
}
