//! The source's one stream. Each request gets the newest picture from owlmic.exe, the last one
//! while the next is being written, or the placeholder; samples come from a fixed pool.

use crate::exports::LIVE;
use crate::shared::{FRAME_BYTES, HEIGHT, Mapping, Picker, WIDTH};
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::E_POINTER;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::core::{GUID, HRESULT, IUnknown, Interface, Ref, Result, implement};

/// Enough for the camera service and two apps to hold a few frames each.
const POOL: u32 = 10;

#[implement(IMFMediaStream2)]
pub struct Stream {
    queue: IMFMediaEventQueue,
    descriptor: IMFStreamDescriptor,
    inner: Mutex<Inner>,
}

struct Inner {
    /// Dropped at shutdown, which breaks the source and stream reference cycle.
    source: Option<IMFMediaSource>,
    running: bool,
    provided_allocator: Option<IMFVideoSampleAllocator>,
    allocator: Option<IMFVideoSampleAllocator>,
    media_type: Option<IMFMediaType>,
    /// Frame duration in 100 ns units.
    duration: i64,
    next_frame_at: Option<Instant>,
    mapping: Option<Mapping>,
    picker: Option<Picker>,
}

impl Stream {
    pub fn new(source: IMFMediaSource, descriptor: IMFStreamDescriptor) -> Result<Self> {
        LIVE.fetch_add(1, Ordering::AcqRel);
        Ok(Self {
            queue: unsafe { MFCreateEventQueue()? },
            descriptor,
            inner: Mutex::new(Inner {
                source: Some(source),
                running: false,
                provided_allocator: None,
                allocator: None,
                media_type: None,
                duration: 333_333,
                next_frame_at: None,
                mapping: None,
                picker: None,
            }),
        })
    }

    fn inner(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn is_running(&self) -> bool {
        self.inner().running
    }

    pub fn set_allocator(&self, allocator: IMFVideoSampleAllocator) -> Result<()> {
        let mut inner = self.inner();
        if inner.source.is_none() {
            return Err(MF_E_SHUTDOWN.into());
        }
        if inner.running {
            return Err(MF_E_INVALIDREQUEST.into());
        }
        inner.provided_allocator = Some(allocator);
        Ok(())
    }

    /// Starts with the media type the app chose on `descriptor`.
    pub fn start(&self, descriptor: &IMFStreamDescriptor, now: &PROPVARIANT) -> Result<()> {
        self.start_internal(Some(descriptor), now, true)
    }

    fn start_internal(
        &self,
        descriptor: Option<&IMFStreamDescriptor>,
        now: &PROPVARIANT,
        send_event: bool,
    ) -> Result<()> {
        let mut inner = self.inner();
        if inner.source.is_none() {
            return Err(MF_E_SHUTDOWN.into());
        }
        unsafe {
            let media_type = match descriptor {
                Some(d) => {
                    let mt = d.GetMediaTypeHandler()?.GetCurrentMediaType()?;
                    inner.media_type = Some(mt.clone());
                    mt
                }
                None => match inner.media_type.clone() {
                    Some(mt) => mt,
                    None => self
                        .descriptor
                        .GetMediaTypeHandler()?
                        .GetCurrentMediaType()?,
                },
            };
            let rate = media_type.GetUINT64(&MF_MT_FRAME_RATE)?;
            let (num, den) = (
                (rate >> 32).max(1) as i64,
                (rate & 0xFFFF_FFFF).max(1) as i64,
            );
            inner.duration = 10_000_000 * den / num;
            if !inner.running || inner.allocator.is_none() {
                let allocator = match inner.provided_allocator.clone() {
                    Some(alloc) => alloc,
                    None => {
                        let mut raw = std::ptr::null_mut();
                        MFCreateVideoSampleAllocatorEx(&IMFVideoSampleAllocatorEx::IID, &mut raw)?;
                        IMFVideoSampleAllocator::from_raw(raw)
                    }
                };
                let _ = allocator.UninitializeSampleAllocator();
                allocator.InitializeSampleAllocator(POOL, &media_type)?;
                inner.allocator = Some(allocator);
            }
        }
        if inner.picker.is_none() {
            inner.picker = Some(Picker::new(crate::placeholder::render(WIDTH, HEIGHT).data));
        }
        if inner.mapping.is_none() {
            inner.mapping = Mapping::create();
        }
        inner.next_frame_at = None;
        inner.running = true;
        drop(inner);
        if send_event {
            unsafe {
                self.queue.QueueEventParamVar(
                    MEStreamStarted.0 as u32,
                    &GUID::zeroed(),
                    HRESULT(0),
                    now,
                )?;
            }
        }
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        self.stop_internal(true)
    }

    fn stop_internal(&self, send_event: bool) -> Result<()> {
        let mut inner = self.inner();
        inner.running = false;
        inner.next_frame_at = None;
        if let Some(alloc) = inner.allocator.take() {
            unsafe {
                let _ = alloc.UninitializeSampleAllocator();
            }
        }
        drop(inner);
        if send_event {
            let now = PROPVARIANT::default();
            unsafe {
                self.queue.QueueEventParamVar(
                    MEStreamStopped.0 as u32,
                    &GUID::zeroed(),
                    HRESULT(0),
                    &now,
                )?;
            }
        }
        Ok(())
    }

    pub fn shutdown(&self) {
        let mut inner = self.inner();
        inner.running = false;
        inner.next_frame_at = None;
        inner.source = None;
        inner.allocator = None;
        inner.provided_allocator = None;
        inner.media_type = None;
        inner.mapping = None;
        drop(inner);
        unsafe {
            let _ = self.queue.Shutdown();
        }
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        LIVE.fetch_sub(1, Ordering::AcqRel);
    }
}

impl IMFMediaEventGenerator_Impl for Stream_Impl {
    fn GetEvent(&self, flags: MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS) -> Result<IMFMediaEvent> {
        unsafe { self.queue.GetEvent(flags.0) }
    }
    fn BeginGetEvent(&self, callback: Ref<IMFAsyncCallback>, state: Ref<IUnknown>) -> Result<()> {
        unsafe { self.queue.BeginGetEvent(callback.as_ref(), state.as_ref()) }
    }
    fn EndGetEvent(&self, result: Ref<IMFAsyncResult>) -> Result<IMFMediaEvent> {
        unsafe { self.queue.EndGetEvent(result.as_ref()) }
    }
    fn QueueEvent(
        &self,
        kind: u32,
        extended: *const GUID,
        status: HRESULT,
        value: *const PROPVARIANT,
    ) -> Result<()> {
        unsafe { self.queue.QueueEventParamVar(kind, extended, status, value) }
    }
}

impl IMFMediaStream_Impl for Stream_Impl {
    fn GetMediaSource(&self) -> Result<IMFMediaSource> {
        self.inner()
            .source
            .clone()
            .ok_or_else(|| MF_E_SHUTDOWN.into())
    }

    fn GetStreamDescriptor(&self) -> Result<IMFStreamDescriptor> {
        if self.inner().source.is_none() {
            return Err(MF_E_SHUTDOWN.into());
        }
        Ok(self.descriptor.clone())
    }

    fn RequestSample(&self, token: Ref<IUnknown>) -> Result<()> {
        let wait_until = {
            let inner = self.inner();
            if inner.source.is_none() {
                return Err(MF_E_SHUTDOWN.into());
            }
            if !inner.running {
                return Err(MF_E_INVALIDREQUEST.into());
            }
            inner.next_frame_at
        };
        if let Some(deadline) = wait_until {
            let now = Instant::now();
            if now < deadline {
                std::thread::sleep(deadline - now);
            }
        }
        let mut inner = self.inner();
        if inner.source.is_none() {
            return Err(MF_E_SHUTDOWN.into());
        }
        let Inner {
            running: true,
            allocator: Some(allocator),
            mapping,
            picker: Some(picker),
            duration,
            next_frame_at,
            ..
        } = &mut *inner
        else {
            return Err(MF_E_INVALIDREQUEST.into());
        };
        let step = Duration::from_nanos((*duration).max(1) as u64 * 100);
        let now = Instant::now();
        *next_frame_at = Some(match *next_frame_at {
            Some(prev) if prev + step > now => prev + step,
            _ => now + step,
        });
        unsafe {
            let sample = allocator.AllocateSample()?;
            let buffer = sample.GetBufferByIndex(0)?;
            let picture = picker.next(mapping.as_ref().map(|m| &m.ring), GetTickCount64());
            write_nv12(&buffer, picture)?;
            let _ = buffer.SetCurrentLength(FRAME_BYTES as u32);
            let pts = MFGetSystemTime();
            sample.SetSampleTime(pts)?;
            sample.SetSampleDuration(*duration)?;
            sample.SetUINT32(&MFSampleExtension_CleanPoint, 1)?;
            let _ = sample.SetUINT64(&MFSampleExtension_DeviceReferenceSystemTime, pts as u64);
            if let Some(token) = token.as_ref() {
                sample.SetUnknown(&MFSampleExtension_Token, token)?;
            }
            drop(inner);
            self.queue.QueueEventParamUnk(
                MEMediaSample.0 as u32,
                &GUID::zeroed(),
                HRESULT(0),
                &sample,
            )
        }
    }
}

unsafe fn write_nv12(buffer: &IMFMediaBuffer, picture: &[u8]) -> Result<()> {
    if let Ok(b2) = buffer.cast::<IMF2DBuffer2>() {
        let mut scanline0 = std::ptr::null_mut();
        let mut pitch = 0i32;
        let mut start = std::ptr::null_mut();
        let mut len = 0u32;
        unsafe {
            b2.Lock2DSize(
                MF2DBuffer_LockFlags_Write,
                &mut scanline0,
                &mut pitch,
                &mut start,
                &mut len,
            )?;
        }
        if scanline0.is_null() || pitch.unsigned_abs() < WIDTH as u32 {
            let _ = unsafe { b2.Unlock2D() };
            return Err(E_POINTER.into());
        }
        let rows = HEIGHT + HEIGHT / 2;
        for row in 0..rows {
            unsafe {
                let dst = scanline0.offset(row as isize * pitch as isize);
                let src = picture.as_ptr().add(row * WIDTH);
                std::ptr::copy_nonoverlapping(src, dst, WIDTH);
            }
        }
        return unsafe { b2.Unlock2D() };
    }
    if let Ok(planar) = buffer.cast::<IMF2DBuffer>() {
        return unsafe { planar.ContiguousCopyFrom(picture) };
    }
    let mut data = std::ptr::null_mut();
    let mut max = 0;
    unsafe {
        buffer.Lock(&mut data, Some(&mut max), None)?;
        if data.is_null() || (max as usize) < FRAME_BYTES {
            let _ = buffer.Unlock();
            return Err(E_POINTER.into());
        }
        std::ptr::copy_nonoverlapping(picture.as_ptr(), data, FRAME_BYTES);
        buffer.Unlock()
    }
}

impl IMFMediaStream2_Impl for Stream_Impl {
    fn SetStreamState(&self, state: MF_STREAM_STATE) -> Result<()> {
        if state == MF_STREAM_STATE_RUNNING {
            if self.is_running() {
                return Ok(());
            }
            let now = PROPVARIANT::default();
            return self.start_internal(None, &now, false);
        }
        if state == MF_STREAM_STATE_STOPPED {
            return self.stop_internal(false);
        }
        Err(MF_E_INVALID_STATE_TRANSITION.into())
    }

    fn GetStreamState(&self) -> Result<MF_STREAM_STATE> {
        Ok(if self.is_running() {
            MF_STREAM_STATE_RUNNING
        } else {
            MF_STREAM_STATE_STOPPED
        })
    }
}
