pub const SAMPLE_RATE: u32 = 48_000;
pub const SAMPLES_PER_MS: usize = (SAMPLE_RATE / 1000) as usize; // 48 samples/ms
pub const MAX_LATENCY_MS: usize = 200; // 200 ms hard cap
pub const MAX_SAMPLES: usize = MAX_LATENCY_MS * SAMPLES_PER_MS; // 9600 samples

pub const USB_TARGET_MS: usize = 20; // 960 samples
pub const WIFI_TARGET_MS: usize = 40; // 1920 samples
pub const BT_TARGET_MS: usize = 80; // 3840 samples
pub const MAX_ADAPTIVE_TARGET_MS: usize = 120; // 5760 samples max

pub const MAX_DRIFT_RATIO: f32 = 0.002; // ±0.2% max drift correction
pub const DRIFT_GAIN: f32 = 0.004; // speed change per unit of depth error; max at half a target off
pub const DEPTH_AVG_FRAMES: f32 = 24_000.0; // ~0.5 s average of buffer depth for drift correction
pub const FADE_FRAMES: usize = 120; // 2.5 ms fade into and out of silence, so gaps don't click
pub const SOFT_CLIP_KNEE: f32 = 0.95; // peaks above this are rounded off, not cut flat
/// The lock-free ring's size: a power of two, room for about 680 ms, well past MAX_SAMPLES.
pub(crate) const RING: usize = 1 << 15;
