//! The camera tile's preview (SYSTEM_DESIGN section 33.1): what Owlmic Cam sends, at 10 frames a
//! second, only while the panel is open. The video thread fills it; the panel reads it.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub const WIDTH: usize = crate::panel::CAMERA_WIDTH_PX as usize;
pub const HEIGHT: usize = crate::panel::CAMERA_HEIGHT_PX as usize;
pub const EVERY_MS: u64 = 100;

pub struct Preview {
    wanted: AtomicBool,
    seq: AtomicU64,
    /// BGRA, top row first.
    pixels: Mutex<Vec<u8>>,
}

impl Default for Preview {
    fn default() -> Self {
        Self {
            wanted: AtomicBool::new(false),
            seq: AtomicU64::new(0),
            pixels: Mutex::new(vec![0; WIDTH * HEIGHT * 4]),
        }
    }
}

impl Preview {
    pub fn set_wanted(&self, wanted: bool) {
        self.wanted.store(wanted, Ordering::Relaxed);
    }

    pub fn wanted(&self) -> bool {
        self.wanted.load(Ordering::Relaxed)
    }

    /// For the video thread. Skipped rather than waited for while the panel is reading.
    pub fn put(&self, fill: impl FnOnce(&mut [u8])) {
        if let Ok(mut p) = self.pixels.try_lock() {
            fill(&mut p);
            self.seq.fetch_add(1, Ordering::Release);
        }
    }

    pub fn seq(&self) -> u64 {
        self.seq.load(Ordering::Acquire)
    }

    /// For the panel: reads the pixels.
    pub fn read<R>(&self, f: impl FnOnce(&[u8]) -> R) -> R {
        f(&self.pixels.lock().unwrap_or_else(|p| p.into_inner()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_picture_advances_the_sequence() {
        let p = Preview::default();
        p.put(|px| px[0] = 7);
        assert_eq!(p.seq(), 1);
        assert_eq!(p.read(|px| px[0]), 7);
    }
}
