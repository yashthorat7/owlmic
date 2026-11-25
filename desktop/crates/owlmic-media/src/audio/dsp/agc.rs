//! Automatic Gain Control (AGC) for microphone audio: the voice at one steady loudness, however
//! far from the phone the speaker sits.
//!
//! It works per 10 ms frame. Only speech frames (clearly above the room's noise floor, see
//! floor.rs) move it: a smoothed speech level, quick to come down for a louder voice and slower
//! to rise for a quieter one, sets the gain that brings it to the target. Between words the gain
//! holds, so the room never swells up in the pauses. The user's volume goes on top, then a peak
//! catch and a soft limiter keep it from clipping.

use super::floor::Frame;

/// Where speech lands, as frame RMS in dBFS.
const TARGET_DB: f32 = -20.0;

/// Up to +36 dB brings a 3–4 m far voice to -20 dBFS when the room is quiet; -20 dB tames close shouts.
const MAX_GAIN_DB: f32 = 36.0;
const MIN_GAIN_DB: f32 = -20.0;
/// Never lift the room noise floor above this dBFS when floor is known, unless SNR demands minimum headroom.
const MAX_NOISE_OUT_DB: f32 = -36.0;
const MIN_FLOOR_CAP_DB: f32 = 12.0;

/// Normal continuous-speech tracking: quick on louder syllables, gentle on softer ones.
const LEVEL_UP: f32 = 0.35;
const LEVEL_DOWN: f32 = 0.06;

/// Continuous gain tracking rates per 10 ms frame when not running an eased step transition.
const GAIN_DOWN: f32 = 0.45;
const GAIN_UP: f32 = 0.16;

/// Pause frames (250 ms) before a quieter utterance triggers fast onset re-anchoring.
const PAUSE_REANCHOR_FRAMES: u32 = 25;
/// Consecutive quieter speech frames (20 ms) before re-anchoring after a pause.
const ONSET_CONFIRM_FRAMES: u32 = 2;
/// Consecutive quieter speech frames (120 ms) while talking continuously before catching up.
const SUSTAINED_DROP_FRAMES: u32 = 12;
const DROP_THRESHOLD_DB: f32 = 6.0;

/// Easing curve duration (in 10 ms frames) for large distance changes.
const EASE_UP_FRAMES: u32 = 12;
const EASE_DOWN_FRAMES: u32 = 4;

/// Peak protection ceiling and maximum sample-ramp length on sudden loud bursts.
const PEAK: f32 = 29_000.0;
const BURST_RAMP_SAMPLES: usize = 48;

pub struct Agc {
    /// The smoothed level of speech frames, in dBFS; none before the first one.
    speech_db: Option<f32>,
    /// The leveling gain now, held through pauses.
    gain_db: f32,
    /// The linear gain (leveling and volume) the last frame ended with: the next one ramps from it.
    applied: f32,
    /// Non-speech frames since last speech.
    silence_frames: u32,
    /// Whether the previous pause exceeded `PAUSE_REANCHOR_FRAMES` at the start of this utterance.
    after_long_pause: bool,
    /// Consecutive speech frames more than `DROP_THRESHOLD_DB` quieter than `speech_db`.
    quiet_streak: u32,
    /// Peak level seen during the current `quiet_streak` so onset locks onto the voiced syllable.
    quiet_peak_db: f32,
    /// Active dB-space easing curve state for large distance transitions.
    ease_start_db: f32,
    ease_target_db: f32,
    ease_step: u32,
    ease_len: u32,
}

impl Agc {
    pub fn new() -> Self {
        Self {
            speech_db: None,
            gain_db: 0.0,
            applied: 1.0,
            silence_frames: 0,
            after_long_pause: true,
            quiet_streak: 0,
            quiet_peak_db: -100.0,
            ease_start_db: 0.0,
            ease_target_db: 0.0,
            ease_step: 0,
            ease_len: 0,
        }
    }

    fn max_gain_for(frame: Frame) -> f32 {
        match frame.floor_db {
            Some(floor) => (MAX_NOISE_OUT_DB - floor).clamp(MIN_FLOOR_CAP_DB, MAX_GAIN_DB),
            None => MAX_GAIN_DB,
        }
    }

    /// Processes one frame in place (16-bit PCM values as floats, measured as `frame`): the
    /// leveling when `normalize` is on, then `volume` (a linear gain the user sets on top), then
    /// the peak catch and the soft limiter.
    pub fn process(&mut self, samples: &mut [f32], frame: Frame, normalize: bool, volume: f32) {
        if samples.is_empty() {
            return;
        }
        let speech = normalize && frame.is_speech();
        let want_db = if normalize {
            if speech {
                let level = frame.level_db;
                let was_long_pause = self.silence_frames >= PAUSE_REANCHOR_FRAMES;
                if self.silence_frames > 0 {
                    self.after_long_pause = was_long_pause;
                    self.quiet_streak = 0;
                    self.quiet_peak_db = -100.0;
                }
                self.silence_frames = 0;

                let updated = match self.speech_db {
                    None => level,
                    Some(s) if level > s => {
                        self.quiet_streak = 0;
                        s + (level - s) * LEVEL_UP
                    }
                    Some(s) if s - level > DROP_THRESHOLD_DB => {
                        self.quiet_streak = self.quiet_streak.saturating_add(1);
                        self.quiet_peak_db = self.quiet_peak_db.max(level);
                        let confirm = if self.after_long_pause {
                            ONSET_CONFIRM_FRAMES
                        } else {
                            SUSTAINED_DROP_FRAMES
                        };
                        if self.quiet_streak >= confirm {
                            self.quiet_streak = 0;
                            self.after_long_pause = false;
                            self.quiet_peak_db
                        } else {
                            s + (level - s) * LEVEL_DOWN
                        }
                    }
                    Some(s) => {
                        self.quiet_streak = 0;
                        s + (level - s) * LEVEL_DOWN
                    }
                };
                self.speech_db = Some(updated);
            } else {
                self.silence_frames = self.silence_frames.saturating_add(1);
                if self.silence_frames >= PAUSE_REANCHOR_FRAMES {
                    self.after_long_pause = true;
                }
                self.quiet_streak = 0;
            }
            let max_gain = Self::max_gain_for(frame);
            self.speech_db.map_or(self.gain_db, |s| {
                (TARGET_DB - s).clamp(MIN_GAIN_DB, max_gain)
            })
        } else {
            self.silence_frames = 0;
            self.quiet_streak = 0;
            0.0
        };

        if !normalize || speech {
            let delta = want_db - self.gain_db;
            if delta.abs() > 3.0 {
                let len = if delta > 0.0 {
                    EASE_UP_FRAMES
                } else {
                    EASE_DOWN_FRAMES
                };
                if self.ease_len == 0 || (want_db - self.ease_target_db).abs() > 2.0 {
                    self.ease_start_db = self.gain_db;
                    self.ease_target_db = want_db;
                    self.ease_step = 0;
                    self.ease_len = len;
                }
            }
            if self.ease_len > 0 {
                self.ease_step += 1;
                let t = (self.ease_step as f32 / self.ease_len as f32).clamp(0.0, 1.0);
                // Smooth ease-in-out cubic blended with ease-out so onset lifts immediately without overshoot.
                let smooth = t * t * (3.0 - 2.0 * t);
                let out_curve = 1.0 - (1.0 - t) * (1.0 - t);
                let eased = 0.5 * smooth + 0.5 * out_curve;
                self.gain_db =
                    self.ease_start_db + (self.ease_target_db - self.ease_start_db) * eased;
                if self.ease_step >= self.ease_len {
                    self.ease_len = 0;
                }
            } else {
                let rate = if want_db < self.gain_db {
                    GAIN_DOWN
                } else {
                    GAIN_UP
                };
                self.gain_db += delta * rate;
            }
        }

        let mut end = db_to_gain(self.gain_db) * volume;
        let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        let mut ramp = samples.len();
        if peak * end > PEAK {
            end = PEAK / peak;
            if normalize {
                self.gain_db = gain_to_db(end / volume);
                self.ease_len = 0;
                if speech {
                    self.speech_db = Some(
                        self.speech_db
                            .map_or(frame.level_db, |s| s.max(frame.level_db)),
                    );
                }
            }
            // Fast 1 ms ramp instead of 0-sample step to prevent clicks while clamping peaks.
            ramp = samples.len().min(BURST_RAMP_SAMPLES);
        }

        let start = self.applied;
        for (i, sample) in samples.iter_mut().enumerate() {
            if !sample.is_finite() {
                *sample = 0.0;
                continue;
            }
            let mut g = if i < ramp {
                start + (end - start) * (i + 1) as f32 / ramp as f32
            } else {
                end
            };
            let abs_s = sample.abs();
            if abs_s * g > PEAK {
                g = PEAK / abs_s;
            }
            *sample = soft_limit(*sample * g);
        }
        self.applied = end;
    }

    /// Resets the speech level and the gain.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for Agc {
    fn default() -> Self {
        Self::new()
    }
}

fn db_to_gain(db: f32) -> f32 {
    10.0f32.powf(db / 20.0)
}

fn gain_to_db(gain: f32) -> f32 {
    20.0 * gain.max(1e-6).log10()
}

/// Smooth hyperbolic soft limiter for samples exceeding 28000.0 peak.
#[inline(always)]
fn soft_limit(x: f32) -> f32 {
    const THRESHOLD: f32 = 28000.0;
    const MARGIN: f32 = 4767.0; // 32767.0 - 28000.0
    let abs_x = x.abs();
    if abs_x <= THRESHOLD {
        x
    } else {
        let over = abs_x - THRESHOLD;
        let compressed = THRESHOLD + MARGIN * (over / (over + MARGIN));
        if x > 0.0 { compressed } else { -compressed }
    }
}

#[cfg(test)]
mod tests {
    use super::super::floor::{Floor, level_db};
    use super::*;

    /// Frames of a steady tone at `db` (frame RMS), after a quiet room so the floor is known.
    fn run(
        agc: &mut Agc,
        floor: &mut Floor,
        db: f32,
        frames: usize,
        normalize: bool,
        volume: f32,
    ) -> f32 {
        let amp = 32768.0 * 10.0f32.powf(db / 20.0) * std::f32::consts::SQRT_2;
        let mut last = 0.0;
        for n in 0..frames {
            let mut samples: Vec<f32> = (0..480)
                .map(|i| amp * ((n * 480 + i) as f32 * 0.0262).sin())
                .collect();
            let frame = floor.measure(&samples);
            agc.process(&mut samples, frame, normalize, volume);
            last = level_db(&samples);
        }
        last
    }

    fn quiet_room(floor: &mut Floor) {
        for _ in 0..100 {
            floor.measure(&[3.0f32; 480]);
        }
    }

    #[test]
    fn quiet_and_loud_speech_both_land_on_the_target() {
        for db in [-48.0, -30.0, -10.0] {
            let (mut agc, mut floor) = (Agc::new(), Floor::new());
            quiet_room(&mut floor);
            let out = run(&mut agc, &mut floor, db, 100, true, 1.0);
            assert!((out - TARGET_DB).abs() < 1.0, "{db} dBFS in, {out} out");
        }
    }

    #[test]
    fn volume_goes_on_top_and_off_leaves_the_level() {
        let (mut agc, mut floor) = (Agc::new(), Floor::new());
        quiet_room(&mut floor);
        let out = run(&mut agc, &mut floor, -35.0, 100, true, db_to_gain(6.0));
        assert!((out - (TARGET_DB + 6.0)).abs() < 1.0, "+6 dB gave {out}");

        let (mut agc, mut floor) = (Agc::new(), Floor::new());
        quiet_room(&mut floor);
        let out = run(&mut agc, &mut floor, -35.0, 100, false, 1.0);
        assert!((out + 35.0).abs() < 0.1, "leveling off moved it to {out}");
        let out = run(&mut agc, &mut floor, -35.0, 100, false, 0.5);
        assert!(
            (out + 41.0).abs() < 0.2,
            "the volume still applies with leveling off: {out}"
        );
    }

    #[test]
    fn a_loud_burst_never_clips() {
        let (mut agc, mut floor) = (Agc::new(), Floor::new());
        quiet_room(&mut floor);
        run(&mut agc, &mut floor, -48.0, 100, true, 1.0);
        let amp = 30_000.0;
        let mut samples: Vec<f32> = (0..480).map(|i| amp * (i as f32 * 0.0262).sin()).collect();
        let frame = floor.measure(&samples);
        agc.process(&mut samples, frame, true, db_to_gain(12.0));
        assert!(samples.iter().all(|s| s.abs() <= PEAK + 1.0));
    }

    #[test]
    fn it_handles_non_finite_samples() {
        let mut agc = Agc::new();
        let mut floor = Floor::new();
        let mut samples = vec![f32::NAN, f32::INFINITY, -f32::INFINITY, 1000.0];
        let frame = floor.measure(&[1000.0; 4]);
        agc.process(&mut samples, frame, true, 1.0);
        assert_eq!(&samples[..3], &[0.0, 0.0, 0.0]);
        assert!(samples[3].is_finite());
    }
}
