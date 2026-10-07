//! The source's one stream. Each request gets the newest picture from owlmic.exe, the last one
//! while the next is being written, or the placeholder; samples come from a fixed pool.

use crate::exports::LIVE;
use crate::shared::{FRAME_BYTES, HEIGHT, Mapping, Picker, WIDTH};
use std::sync::Mutex;
use std::sync::atomic::Ordering;
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
    allocator: Option<IMFVideoSampleAllocatorEx>,
    /// Frame duration in 100 ns units.
    duration: i64,
    frame_count: u64,
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
                allocator: None,
                duration: 333_333,
                frame_count: 0,
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

    /// Starts with the media type the app chose on `descriptor`.
    pub fn start(&self, descriptor: &IMFStreamDescriptor, now: &PROPVARIANT) -> Result<()> {
        let mut inner = self.inner();
        if inner.source.is_none() {
            return Err(MF_E_SHUTDOWN.into());
        }
        unsafe {
            let media_type = descriptor.GetMediaTypeHandler()?.GetCurrentMediaType()?;
            let rate = media_type.GetUINT64(&MF_MT_FRAME_RATE)?;
            let (num, den) = (
                (rate >> 32).max(1) as i64,
                (rate & 0xFFFF_FFFF).max(1) as i64,
            );
            inner.duration = 10_000_000 * den / num;
            let mut raw = std::ptr::null_mut();
            MFCreateVideoSampleAllocatorEx(&IMFVideoSampleAllocatorEx::IID, &mut raw)?;
            let allocator = IMFVideoSampleAllocatorEx::from_raw(raw);
            allocator.InitializeSampleAllocator(POOL, &media_type)?;
            inner.allocator = Some(allocator);
        }
        if inner.picker.is_none() {
            inner.picker = Some(Picker::new(crate::placeholder::render(WIDTH, HEIGHT).data));
        }
        if inner.mapping.is_none() {
            inner.mapping = Mapping::create();
        }
        inner.frame_count = 0;
        inner.running = true;
        unsafe {
            self.queue.QueueEventParamVar(
                MEStreamStarted.0 as u32,
                &GUID::zeroed(),
                HRESULT(0),
                now,
            )
        }
    }

    pub fn stop(&self) -> Result<()> {
        let mut inner = self.inner();
        inner.running = false;
        inner.allocator = None;
        let now = PROPVARIANT::default();
        unsafe {
            self.queue.QueueEventParamVar(
                MEStreamStopped.0 as u32,
                &GUID::zeroed(),
                HRESULT(0),
                &now,
            )
        }
    }

    pub fn shutdown(&self) {
        let mut inner = self.inner();
        inner.running = false;
        inner.source = None;
        inner.allocator = None;
        inner.mapping = None;
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
            frame_count,
            ..
        } = &mut *inner
        else {
            return Err(MF_E_INVALIDREQUEST.into());
        };
        unsafe {
            let sample = allocator.AllocateSample()?;
            let buffer = sample.GetBufferByIndex(0)?;
            let picture = picker.next(mapping.as_ref().map(|m| &m.ring), GetTickCount64());
            if let Ok(planar) = buffer.cast::<IMF2DBuffer>() {
                planar.ContiguousCopyFrom(picture)?;
            } else {
                let mut data = std::ptr::null_mut();
                let mut max = 0;
                buffer.Lock(&mut data, Some(&mut max), None)?;
                if data.is_null() || (max as usize) < FRAME_BYTES {
                    let _ = buffer.Unlock();
                    return Err(E_POINTER.into());
                }
                std::ptr::copy_nonoverlapping(picture.as_ptr(), data, FRAME_BYTES);
                buffer.Unlock()?;
            }
            let _ = buffer.SetCurrentLength(FRAME_BYTES as u32);
            let sample_time = (*frame_count as i64) * *duration;
            *frame_count += 1;
            sample.SetSampleTime(sample_time)?;
            sample.SetSampleDuration(*duration)?;
            sample.SetUINT32(&MFSampleExtension_CleanPoint, 1)?;
            let _ = sample.SetUINT64(
                &MFSampleExtension_DeviceReferenceSystemTime,
                MFGetSystemTime() as u64,
            );
            if let Some(token) = token.as_ref() {
                sample.SetUnknown(&MFSampleExtension_Token, token)?;
            }
            self.queue.QueueEventParamUnk(
                MEMediaSample.0 as u32,
                &GUID::zeroed(),
                HRESULT(0),
                &sample,
            )
        }
    }
}

impl IMFMediaStream2_Impl for Stream_Impl {
    fn SetStreamState(&self, state: MF_STREAM_STATE) -> Result<()> {
        if state == MF_STREAM_STATE_RUNNING {
            if self.is_running() {
                return Ok(());
            }
            let now = PROPVARIANT::default();
            return self.start(&self.descriptor, &now);
        }
        if state == MF_STREAM_STATE_STOPPED {
            return self.stop();
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
