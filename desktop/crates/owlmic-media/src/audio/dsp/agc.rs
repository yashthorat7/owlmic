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

/// A voice 2 m from the phone needs about +25 dB; a shout up close about -15 dB.
const MAX_GAIN_DB: f32 = 30.0;
const MIN_GAIN_DB: f32 = -20.0;

/// The speech level follows a louder frame by this much of the difference, a quieter one by the
/// second: syllables vary by 10 dB or more, and the level should sit near the loud ones.
const LEVEL_UP: f32 = 0.3;
const LEVEL_DOWN: f32 = 0.08;

/// The gain moves towards where it should be by these fractions a frame: fast down (a voice that
/// came closer), gentler up. Together they settle well under a second.
const GAIN_DOWN: f32 = 0.5;
const GAIN_UP: f32 = 0.15;

/// The peak catch keeps a frame's loudest sample under this.
const PEAK: f32 = 29_000.0;

pub struct Agc {
    /// The smoothed level of speech frames, in dBFS; none before the first one.
    speech_db: Option<f32>,
    /// The leveling gain now, held through pauses.
    gain_db: f32,
    /// The linear gain (leveling and volume) the last frame ended with: the next one ramps from it.
    applied: f32,
}

impl Agc {
    pub fn new() -> Self {
        Self {
            speech_db: None,
            gain_db: 0.0,
            applied: 1.0,
        }
    }

    /// Processes one frame in place (16-bit PCM values as floats, measured as `frame`): the
    /// leveling when `normalize` is on, then `volume` (a linear gain the user sets on top), then
    /// the peak catch and the soft limiter.
    pub fn process(&mut self, samples: &mut [f32], frame: Frame, normalize: bool, volume: f32) {
        if samples.is_empty() {
            return;
        }
        let want_db = if normalize {
            if frame.is_speech() {
                let level = frame.level_db;
                let speech = match self.speech_db {
                    None => level,
                    Some(s) if level > s => s + (level - s) * LEVEL_UP,
                    Some(s) => s + (level - s) * LEVEL_DOWN,
                };
                self.speech_db = Some(speech);
            }
            // No speech yet: hold where the gain is.
            self.speech_db.map_or(self.gain_db, |s| {
                (TARGET_DB - s).clamp(MIN_GAIN_DB, MAX_GAIN_DB)
            })
        } else {
            0.0
        };
        let rate = if want_db < self.gain_db {
            GAIN_DOWN
        } else {
            GAIN_UP
        };
        // Leveling on, only speech moves the gain; off, it returns to unity at any time.
        if !normalize || frame.is_speech() {
            self.gain_db += (want_db - self.gain_db) * rate;
        }

        let mut end = db_to_gain(self.gain_db) * volume;
        // A voice that suddenly came close: no frame goes past the peak, from its first sample.
        let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        let mut ramp = samples.len();
        if peak * end > PEAK {
            end = PEAK / peak;
            if normalize {
                self.gain_db = gain_to_db(end / volume);
            }
            // Down at once: a ramp from the old gain would overshoot on the loud samples.
            ramp = 0;
        }

        let start = self.applied;
        for (i, sample) in samples.iter_mut().enumerate() {
            if !sample.is_finite() {
                *sample = 0.0;
                continue;
            }
            let g = if i < ramp {
                start + (end - start) * (i + 1) as f32 / ramp as f32
            } else {
                end
            };
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
