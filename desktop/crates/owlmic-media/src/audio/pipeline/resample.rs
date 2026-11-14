use super::JitterBuffer;
use super::constants::*;
use super::ring::Queued;
use std::sync::atomic::Ordering;
use std::time::Instant;

impl JitterBuffer {
    /// Output side: fills the device's buffer, resampled from the phone's 48 kHz to the device's
    /// rate. Takes no lock the receive side holds.
    pub fn pop_samples(&self, out: &mut [f32], channels: u16) {
        let ch = channels.max(1) as usize;
        let mut playout = self.playout.lock().unwrap_or_else(|p| p.into_inner());
        let (mut buf, flushed) = self.ring.read();
        if flushed {
            *playout = Playout::new();
        }
        // Past the hard cap the oldest audio goes.
        if buf.len() > MAX_SAMPLES {
            buf.skip(buf.len() - MAX_SAMPLES);
        }
        let target = self.adaptive_target_samples.load(Ordering::Relaxed);
        let step = SAMPLE_RATE as f32 / self.output_rate.load(Ordering::Relaxed) as f32;

        drift_resample_pop(&mut buf, out, ch, target, step, &mut playout);
    }
}

/// Where playback stands between output callbacks.
pub(crate) struct Playout {
    /// Playing, as opposed to silent while the buffer fills to its target.
    started: bool,
    /// How far between `buf[0]` and `buf[1]` the next output frame falls.
    phase: f32,
    /// The sample just before `buf[0]`, the fourth point the interpolation needs.
    prev: f32,
    /// Buffer depth averaged over about half a second: what drift correction steers by.
    depth_avg: f32,
    /// Frames left in the fade-in after playback (re)starts.
    fade_in: usize,
}

impl Playout {
    pub(crate) fn new() -> Self {
        Self {
            started: false,
            phase: 0.0,
            prev: 0.0,
            depth_avg: 0.0,
            fade_in: 0,
        }
    }
}

pub(crate) struct JitterStats {
    pub(crate) last_arrival: Option<Instant>,
    pub(crate) last_capture_ts: Option<u64>,
    pub(crate) jitter_estimate_us: f64,
}

impl JitterStats {
    pub(crate) fn new() -> Self {
        Self {
            last_arrival: None,
            last_capture_ts: None,
            jitter_estimate_us: 0.0,
        }
    }
}

/// `step` is how many 48 kHz samples one output frame advances: 48 kHz over the device's rate.
/// Drift correction then nudges it by up to MAX_DRIFT_RATIO to hold the buffer at `target`.
pub(crate) fn drift_resample_pop(
    buf: &mut Queued,
    out: &mut [f32],
    channels: usize,
    target: usize,
    step: f32,
    play: &mut Playout,
) {
    let frames_needed = out.len() / channels;

    if !play.started {
        if buf.len() >= target {
            play.started = true;
            play.fade_in = FADE_FRAMES;
            play.depth_avg = buf.len() as f32;
        } else {
            out.fill(0.0);
            return;
        }
    }

    // The depth jumps by a whole packet each time one lands. Steering by it directly slams the
    // speed between its limits every callback, which makes high sounds flutter; its average
    // only moves with real clock drift.
    let smoothing = (frames_needed as f32 / DEPTH_AVG_FRAMES).min(1.0);
    play.depth_avg += (buf.len() as f32 - play.depth_avg) * smoothing;
    let error = (play.depth_avg - target as f32) / target.max(1) as f32;
    let speed_adjust = (error * DRIFT_GAIN).clamp(-MAX_DRIFT_RATIO, MAX_DRIFT_RATIO);
    let effective_rate = step * (1.0 + speed_adjust);

    // If the buffer runs dry within this callback, fade the last frames out instead of
    // dropping to silence mid-wave, which clicks.
    let playable = ((buf.len() as f32 - play.phase) / effective_rate)
        .ceil()
        .max(0.0) as usize;
    let fade_out_from = if playable < frames_needed {
        playable.saturating_sub(FADE_FRAMES)
    } else {
        usize::MAX
    };

    for (i, frame) in out.chunks_exact_mut(channels).enumerate() {
        if buf.is_empty() {
            frame.fill(0.0);
            play.started = false;
            continue;
        }

        let s0 = buf.get(0).map_or(0.0, |s| s as f32);
        let s1 = buf.get(1).map_or(s0, |s| s as f32);
        let s2 = buf.get(2).map_or(s1, |s| s as f32);
        let mut level = 1.0 / 32768.0;
        if play.fade_in > 0 {
            level *= 1.0 - play.fade_in as f32 / FADE_FRAMES as f32;
            play.fade_in -= 1;
        }
        if i >= fade_out_from {
            level *= playable.saturating_sub(i) as f32 / FADE_FRAMES as f32;
        }
        frame.fill(soft_clip(cubic(play.prev, s0, s1, s2, play.phase) * level));

        play.phase += effective_rate;
        while play.phase >= 1.0 {
            if let Some(s) = buf.pop_front() {
                play.prev = s as f32;
            }
            play.phase -= 1.0;
        }
    }
}

/// 4-point cubic (Catmull-Rom) interpolation at `t` between `x0` and `x1`. Straight-line
/// interpolation dulls the highs by a varying amount as `t` moves, which is audible as a
/// flutter on "s" and "t" sounds.
fn cubic(xm1: f32, x0: f32, x1: f32, x2: f32, t: f32) -> f32 {
    let c1 = 0.5 * (x1 - xm1);
    let c2 = xm1 - 2.5 * x0 + 2.0 * x1 - 0.5 * x2;
    let c3 = 0.5 * (x2 - xm1) + 1.5 * (x0 - x1);
    ((c3 * t + c2) * t + c1) * t + x0
}

/// Leaves everything up to SOFT_CLIP_KNEE alone and bends peaks above it smoothly toward full
/// scale, where cutting them flat would crackle.
pub(crate) fn soft_clip(x: f32) -> f32 {
    let over = x.abs() - SOFT_CLIP_KNEE;
    if over <= 0.0 {
        return x;
    }
    let room = 1.0 - SOFT_CLIP_KNEE;
    x.signum() * (SOFT_CLIP_KNEE + room * (over / room).tanh())
}
