//! The loopback capture comes in the output device's own format; the speaker path wants 48 kHz
//! stereo. Channels beyond the first two are dropped (they are surround copies of the same
//! sound), and other rates are resampled linearly, which is plenty for monitoring.

pub struct ToStereo48k {
    rate: u32,
    channels: usize,
    /// Position between the last input frame and the next, for resampling.
    phase: f64,
    last: [f32; 2],
    out: Vec<f32>,
}

impl ToStereo48k {
    pub fn new(rate: u32, channels: usize) -> Self {
        Self {
            rate: rate.max(1),
            channels: channels.max(1),
            phase: 0.0,
            last: [0.0; 2],
            out: Vec::with_capacity(4096),
        }
    }

    /// Interleaved input in the device format; returns interleaved 48 kHz stereo.
    pub fn convert(&mut self, input: &[f32]) -> &[f32] {
        self.out.clear();
        let frames = input.len() / self.channels;
        let frame = |i: usize| -> [f32; 2] {
            let f = &input[i * self.channels..];
            if self.channels == 1 {
                [f[0], f[0]]
            } else {
                [f[0], f[1]]
            }
        };
        if self.rate == 48_000 {
            for i in 0..frames {
                self.out.extend_from_slice(&frame(i));
            }
            return &self.out;
        }
        let step = self.rate as f64 / 48_000.0;
        // `phase` counts from the frame before this input (kept in `last`), so -1 is `last`.
        while self.phase < frames as f64 {
            let pos = self.phase - 1.0;
            let i = pos.floor();
            let t = (pos - i) as f32;
            let a = if i < 0.0 {
                self.last
            } else {
                frame(i as usize)
            };
            let b = frame((i + 1.0).max(0.0) as usize);
            self.out.push(a[0] + (b[0] - a[0]) * t);
            self.out.push(a[1] + (b[1] - a[1]) * t);
            self.phase += step;
        }
        if frames > 0 {
            self.last = frame(frames - 1);
            self.phase -= frames as f64;
        }
        &self.out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_48k_passes_through_and_mono_is_doubled() {
        let mut c = ToStereo48k::new(48_000, 2);
        assert_eq!(c.convert(&[0.1, 0.2, 0.3, 0.4]), &[0.1, 0.2, 0.3, 0.4]);
        let mut m = ToStereo48k::new(48_000, 1);
        assert_eq!(m.convert(&[0.5]), &[0.5, 0.5]);
    }

    #[test]
    fn surround_keeps_the_front_pair() {
        let mut c = ToStereo48k::new(48_000, 6);
        assert_eq!(c.convert(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]), &[1.0, 2.0]);
    }

    #[test]
    fn a_44_1k_stream_comes_out_at_48k_on_average() {
        let mut c = ToStereo48k::new(44_100, 2);
        let mut produced = 0;
        for _ in 0..100 {
            produced += c.convert(&[0.0; 441 * 2]).len() / 2;
        }
        assert!((produced as i64 - 48_000).abs() <= 2, "{produced}");
    }
}
