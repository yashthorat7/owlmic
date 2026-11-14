//! What the leveling and the gate decide from, once per 10 ms frame: how loud the frame is, and
//! how loud the room is under the voice (its noise floor). Speech is whatever stands clearly above
//! that floor, so the decisions hold at any distance from the mic: a far voice is quieter, but so
//! is nothing else.

/// Frames quieter than this carry no signal (digital silence, a paused or starting mic): they
/// teach the floor nothing.
const NO_SIGNAL_DB: f32 = -90.0;

/// The floor is the quietest frame of the last 2 s: speech always has a dip that long, a room's
/// noise never does.
const WINDOW: usize = 200;

/// The minimum is kept per block of 20 frames, so a new frame costs one comparison.
const BLOCK: usize = 20;
const BLOCKS: usize = WINDOW / BLOCK;

/// A frame this far above the floor is speech.
const SPEECH_ABOVE_FLOOR_DB: f32 = 10.0;

/// And never quieter than this, however quiet the room.
const SPEECH_MIN_DB: f32 = -72.0;

/// A frame's level in dBFS (16-bit full scale), from its samples.
pub fn level_db(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return -100.0;
    }
    let sum: f32 = samples.iter().map(|s| s * s).sum();
    let rms = (sum / samples.len() as f32).sqrt();
    if rms <= 0.0 {
        -100.0
    } else {
        (20.0 * (rms / 32768.0).log10()).max(-100.0)
    }
}

/// One frame as the leveling and the gate see it.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub level_db: f32,
    /// The room's noise floor, or none yet (nothing but silence so far).
    pub floor_db: Option<f32>,
}

impl Frame {
    /// Clearly above the room: someone is talking.
    pub fn is_speech(&self) -> bool {
        self.is_above(SPEECH_ABOVE_FLOOR_DB) && self.level_db > SPEECH_MIN_DB
    }

    /// At least `margin_db` above the floor.
    pub fn is_above(&self, margin_db: f32) -> bool {
        self.floor_db
            .is_some_and(|floor| self.level_db > floor + margin_db)
    }
}

/// The noise floor tracker: the minimum of the frame levels over the last two seconds.
pub struct Floor {
    /// The minimum of each finished block, oldest first; `None` before it holds a frame.
    blocks: [Option<f32>; BLOCKS],
    at: usize,
    current: Option<f32>,
    in_block: usize,
}

impl Floor {
    pub fn new() -> Self {
        Self {
            blocks: [None; BLOCKS],
            at: 0,
            current: None,
            in_block: 0,
        }
    }

    /// Measures `samples` (one frame) and returns it with the floor under it.
    pub fn measure(&mut self, samples: &[f32]) -> Frame {
        let level_db = level_db(samples);
        if level_db > NO_SIGNAL_DB {
            self.current = Some(self.current.map_or(level_db, |m| m.min(level_db)));
            self.in_block += 1;
            if self.in_block == BLOCK {
                self.blocks[self.at] = self.current.take();
                self.at = (self.at + 1) % BLOCKS;
                self.in_block = 0;
            }
        }
        let floor_db = self
            .blocks
            .iter()
            .chain(std::iter::once(&self.current))
            .flatten()
            .copied()
            .reduce(f32::min);
        Frame { level_db, floor_db }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for Floor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(amplitude: f32) -> Vec<f32> {
        (0..480)
            .map(|i| if i % 2 == 0 { amplitude } else { -amplitude })
            .collect()
    }

    #[test]
    fn the_level_is_the_frame_rms_in_dbfs() {
        assert!((level_db(&frame(3276.8)) + 20.0).abs() < 0.01);
        assert_eq!(level_db(&[0.0; 480]), -100.0);
    }

    #[test]
    fn the_floor_is_the_quietest_frame_and_speech_stands_above_it() {
        let mut floor = Floor::new();
        for _ in 0..50 {
            floor.measure(&frame(30.0));
        }
        let speech = floor.measure(&frame(3000.0));
        assert!((speech.floor_db.unwrap() - level_db(&frame(30.0))).abs() < 0.01);
        assert!(speech.is_speech());
        assert!(!floor.measure(&frame(40.0)).is_speech());
    }

    #[test]
    fn the_floor_follows_a_louder_room_within_two_seconds() {
        let mut floor = Floor::new();
        for _ in 0..300 {
            floor.measure(&frame(30.0));
        }
        let mut last = floor.measure(&frame(300.0));
        for _ in 0..WINDOW + BLOCK {
            last = floor.measure(&frame(300.0));
        }
        assert!((last.floor_db.unwrap() - level_db(&frame(300.0))).abs() < 0.01);
        assert!(!last.is_speech(), "the louder room is not speech");
    }

    #[test]
    fn digital_silence_teaches_nothing() {
        let mut floor = Floor::new();
        for _ in 0..100 {
            assert!(floor.measure(&[0.0; 480]).floor_db.is_none());
        }
        assert!(!floor.measure(&frame(3000.0)).is_above(0.0));
    }
}
