//! The frame ring owlmic.exe writes and Owlmic Cam reads (SYSTEM_DESIGN section 17.2): a header
//! and two NV12 slots in shared memory. The writer fills the slot the reader isn't on and then
//! publishes it; each slot has a sequence number that is odd while it is being written, so a
//! reader can tell a torn copy and keep the previous picture.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Created by the camera (it runs as a service, so it can make a global object), opened by the app.
pub const NAME: &str = "Global\\OwlmicCamFrames";
pub const WIDTH: usize = 1920;
pub const HEIGHT: usize = 1080;
pub const FRAME_BYTES: usize = WIDTH * HEIGHT * 3 / 2;
const MAGIC: u32 = u32::from_le_bytes(*b"OWCM");
const HEADER: usize = 64;
pub const TOTAL: usize = HEADER + 2 * FRAME_BYTES;
/// No new picture for this long and the camera shows its placeholder.
pub const STALE_MS: u64 = 2_000;
const NONE: u32 = u32::MAX;

// Header offsets.
const OFF_MAGIC: usize = 0;
const OFF_LATEST: usize = 4;
const OFF_SEQ: usize = 8; // two u32, one per slot
const OFF_FRAMES: usize = 16;
const OFF_WRITTEN_MS: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Read {
    Picture,
    /// The writer lapped the reader mid-copy; the previous picture still stands.
    Torn,
    /// No picture, or none for [`STALE_MS`].
    Nothing,
}

/// A view of the ring's memory. Every access goes through atomics or whole-slot copies.
pub struct Ring {
    base: *mut u8,
}

// The memory is shared between processes by design; the sequence numbers make access safe.
unsafe impl Send for Ring {}
unsafe impl Sync for Ring {}

impl Ring {
    /// # Safety
    /// `base` must point to at least [`TOTAL`] bytes, 8-byte aligned, valid while the ring lives.
    pub unsafe fn new(base: *mut u8) -> Self {
        Self { base }
    }

    fn u32_at(&self, off: usize) -> &AtomicU32 {
        unsafe { AtomicU32::from_ptr(self.base.add(off) as *mut u32) }
    }

    fn u64_at(&self, off: usize) -> &AtomicU64 {
        unsafe { AtomicU64::from_ptr(self.base.add(off) as *mut u64) }
    }

    fn slot(&self, i: usize) -> *mut u8 {
        unsafe { self.base.add(HEADER + i * FRAME_BYTES) }
    }

    /// Sets up a fresh ring: no picture yet.
    pub fn init(&self) {
        self.u32_at(OFF_LATEST).store(NONE, Ordering::Release);
        self.u32_at(OFF_SEQ).store(0, Ordering::Release);
        self.u32_at(OFF_SEQ + 4).store(0, Ordering::Release);
        self.u64_at(OFF_FRAMES).store(0, Ordering::Release);
        self.u64_at(OFF_WRITTEN_MS).store(0, Ordering::Release);
        self.u32_at(OFF_MAGIC).store(MAGIC, Ordering::Release);
    }

    pub fn is_ready(&self) -> bool {
        self.u32_at(OFF_MAGIC).load(Ordering::Acquire) == MAGIC
    }

    /// Publishes one 1920 x 1080 NV12 picture. `now_ms` is a clock both processes share.
    pub fn write(&self, frame: &[u8], now_ms: u64) {
        assert_eq!(frame.len(), FRAME_BYTES);
        let latest = self.u32_at(OFF_LATEST).load(Ordering::Acquire);
        let slot = if latest == 0 { 1 } else { 0 };
        let seq = self.u32_at(OFF_SEQ + slot * 4);
        seq.fetch_add(1, Ordering::AcqRel);
        unsafe { std::ptr::copy_nonoverlapping(frame.as_ptr(), self.slot(slot), FRAME_BYTES) };
        seq.fetch_add(1, Ordering::AcqRel);
        self.u32_at(OFF_LATEST)
            .store(slot as u32, Ordering::Release);
        self.u64_at(OFF_FRAMES).fetch_add(1, Ordering::AcqRel);
        self.u64_at(OFF_WRITTEN_MS).store(now_ms, Ordering::Release);
    }

    /// Copies the newest picture into `out` (at least [`FRAME_BYTES`] long).
    pub fn read(&self, out: &mut [u8], now_ms: u64) -> Read {
        let latest = self.u32_at(OFF_LATEST).load(Ordering::Acquire);
        if latest == NONE
            || now_ms.saturating_sub(self.u64_at(OFF_WRITTEN_MS).load(Ordering::Acquire)) > STALE_MS
        {
            return Read::Nothing;
        }
        let seq = self.u32_at(OFF_SEQ + latest as usize * 4);
        let before = seq.load(Ordering::Acquire);
        if before % 2 == 1 {
            return Read::Torn;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(self.slot(latest as usize), out.as_mut_ptr(), FRAME_BYTES)
        };
        // The copy's plain loads must complete before the sequence is read again; an acquire
        // load alone doesn't order loads before it (it matters on ARM64, not on x64).
        std::sync::atomic::fence(Ordering::Acquire);
        if seq.load(Ordering::Relaxed) == before {
            Read::Picture
        } else {
            Read::Torn
        }
    }

    pub fn frames_written(&self) -> u64 {
        self.u64_at(OFF_FRAMES).load(Ordering::Acquire)
    }
}

/// What the camera serves for each request: the newest picture, the last good one while the
/// writer is mid-frame, or the placeholder. Buffers are allocated once.
pub struct Picker {
    scratch: Vec<u8>,
    shown: Vec<u8>,
    placeholder: Vec<u8>,
    has_picture: bool,
}

impl Picker {
    pub fn new(placeholder: Vec<u8>) -> Self {
        assert_eq!(placeholder.len(), FRAME_BYTES);
        Self {
            scratch: vec![0; FRAME_BYTES],
            shown: vec![0; FRAME_BYTES],
            placeholder,
            has_picture: false,
        }
    }

    pub fn next(&mut self, ring: Option<&Ring>, now_ms: u64) -> &[u8] {
        match ring.map_or(Read::Nothing, |r| r.read(&mut self.scratch, now_ms)) {
            Read::Picture => {
                std::mem::swap(&mut self.scratch, &mut self.shown);
                self.has_picture = true;
            }
            Read::Torn => {}
            Read::Nothing => self.has_picture = false,
        }
        if self.has_picture {
            &self.shown
        } else {
            &self.placeholder
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory() -> Vec<u64> {
        vec![0u64; TOTAL.div_ceil(8)]
    }

    #[test]
    fn the_newest_picture_is_read_and_stale_ones_are_not() {
        let mut mem = memory();
        let ring = unsafe { Ring::new(mem.as_mut_ptr() as *mut u8) };
        ring.init();
        let mut out = vec![0u8; FRAME_BYTES];
        assert_eq!(ring.read(&mut out, 0), Read::Nothing);
        ring.write(&vec![1u8; FRAME_BYTES], 1_000);
        ring.write(&vec![2u8; FRAME_BYTES], 1_033);
        assert_eq!(ring.read(&mut out, 1_040), Read::Picture);
        assert!(out.iter().all(|b| *b == 2));
        assert_eq!(ring.frames_written(), 2);
        assert_eq!(
            ring.read(&mut out, 1_033 + STALE_MS + 1),
            Read::Nothing,
            "the phone went quiet"
        );
    }

    #[test]
    fn a_picture_being_written_is_not_read() {
        let mut mem = memory();
        let ring = unsafe { Ring::new(mem.as_mut_ptr() as *mut u8) };
        ring.init();
        ring.write(&vec![3u8; FRAME_BYTES], 10);
        ring.u32_at(OFF_SEQ + ring.u32_at(OFF_LATEST).load(Ordering::Relaxed) as usize * 4)
            .fetch_add(1, Ordering::Relaxed);
        let mut out = vec![0u8; FRAME_BYTES];
        assert_eq!(ring.read(&mut out, 10), Read::Torn);
    }

    #[test]
    fn the_picker_holds_the_last_picture_through_a_torn_read_and_falls_back_when_stale() {
        let mut mem = memory();
        let ring = unsafe { Ring::new(mem.as_mut_ptr() as *mut u8) };
        ring.init();
        let mut picker = Picker::new(vec![9u8; FRAME_BYTES]);
        assert_eq!(picker.next(None, 0)[0], 9);
        ring.write(&vec![4u8; FRAME_BYTES], 100);
        assert_eq!(picker.next(Some(&ring), 110)[0], 4);
        ring.u32_at(OFF_SEQ + ring.u32_at(OFF_LATEST).load(Ordering::Relaxed) as usize * 4)
            .fetch_add(1, Ordering::Relaxed);
        assert_eq!(picker.next(Some(&ring), 120)[0], 4);
        assert_eq!(picker.next(Some(&ring), 100 + STALE_MS + 1)[0], 9);
    }
}

#[cfg(windows)]
pub use mapping::Mapping;

#[cfg(windows)]
mod mapping {
    use super::{NAME, Ring, TOTAL};
    use windows::Win32::Foundation::{
        CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HLOCAL, INVALID_HANDLE_VALUE,
        LocalFree,
    };
    use windows::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
    use windows::Win32::System::Memory::{
        CreateFileMappingW, FILE_MAP_ALL_ACCESS, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile,
        OpenFileMappingW, PAGE_READWRITE, UnmapViewOfFile,
    };
    use windows::core::{HSTRING, w};

    /// The ring's shared memory, mapped into this process.
    pub struct Mapping {
        handle: HANDLE,
        view: MEMORY_MAPPED_VIEW_ADDRESS,
        pub ring: Ring,
    }

    unsafe impl Send for Mapping {}

    impl Mapping {
        /// For the camera: creates the memory, readable and writable by every signed-in user.
        pub fn create() -> Option<Self> {
            unsafe {
                let mut sd = PSECURITY_DESCRIPTOR::default();
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    w!("D:(A;;GA;;;SY)(A;;GA;;;LS)(A;;GA;;;AU)"),
                    SDDL_REVISION_1,
                    &mut sd,
                    None,
                )
                .ok()?;
                let sa = SECURITY_ATTRIBUTES {
                    nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                    lpSecurityDescriptor: sd.0,
                    bInheritHandle: false.into(),
                };
                let handle = CreateFileMappingW(
                    INVALID_HANDLE_VALUE,
                    Some(&sa),
                    PAGE_READWRITE,
                    0,
                    TOTAL as u32,
                    &HSTRING::from(NAME),
                );
                let existed = GetLastError() == ERROR_ALREADY_EXISTS;
                let _ = LocalFree(Some(HLOCAL(sd.0)));
                let mapping = Self::map(handle.ok()?)?;
                if !existed || !mapping.ring.is_ready() {
                    mapping.ring.init();
                }
                Some(mapping)
            }
        }

        /// For owlmic.exe: the camera's memory, present only while an app has Owlmic Cam open.
        pub fn open() -> Option<Self> {
            unsafe {
                Self::map(
                    OpenFileMappingW(FILE_MAP_ALL_ACCESS.0, false, &HSTRING::from(NAME)).ok()?,
                )
            }
        }

        unsafe fn map(handle: HANDLE) -> Option<Self> {
            let view = unsafe { MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, TOTAL) };
            if view.Value.is_null() {
                unsafe {
                    let _ = CloseHandle(handle);
                }
                return None;
            }
            let ring = unsafe { Ring::new(view.Value as *mut u8) };
            Some(Self { handle, view, ring })
        }
    }

    impl Drop for Mapping {
        fn drop(&mut self) {
            unsafe {
                let _ = UnmapViewOfFile(self.view);
                let _ = CloseHandle(self.handle);
            }
        }
    }
}
