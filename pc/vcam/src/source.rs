//! The media source: one live colour stream, NV12 1920 x 1080 at 30 or 60 frames a second, with
//! the stream attributes the camera service needs to share it between apps.

use crate::exports::LIVE;
use crate::shared::{HEIGHT, WIDTH};
use crate::stream::Stream;
use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use windows::Win32::Foundation::{E_INVALIDARG, E_POINTER, ERROR_SET_NOT_FOUND};
use windows::Win32::Media::KernelStreaming::{
    IKsControl, IKsControl_Impl, KSIDENTIFIER, PINNAME_VIDEO_CAPTURE,
};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::core::{ComObject, GUID, HRESULT, IUnknown, Ref, Result, implement};

pub const RATES: [u32; 2] = [30, 60];

fn media_type(fps: u32) -> Result<IMFMediaType> {
    unsafe {
        let t = MFCreateMediaType()?;
        t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        t.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
        t.SetUINT64(&MF_MT_FRAME_SIZE, ((WIDTH as u64) << 32) | HEIGHT as u64)?;
        t.SetUINT64(&MF_MT_FRAME_RATE, ((fps as u64) << 32) | 1)?;
        t.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1 << 32) | 1)?;
        t.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        t.SetUINT32(&MF_MT_ALL_SAMPLES_INDEPENDENT, 1)?;
        t.SetUINT32(&MF_MT_DEFAULT_STRIDE, WIDTH as u32)?;
        t.SetUINT32(&MF_MT_SAMPLE_SIZE, crate::shared::FRAME_BYTES as u32)?;
        Ok(t)
    }
}

pub fn create() -> Result<IMFMediaSourceEx> {
    unsafe {
        let types = [Some(media_type(RATES[0])?), Some(media_type(RATES[1])?)];
        let descriptor = MFCreateStreamDescriptor(0, &types)?;
        descriptor
            .GetMediaTypeHandler()?
            .SetCurrentMediaType(types[0].as_ref())?;
        let mut stream_attributes = None;
        MFCreateAttributes(&mut stream_attributes, 4)?;
        let stream_attributes = stream_attributes.ok_or(E_POINTER)?;
        stream_attributes.SetGUID(&MF_DEVICESTREAM_STREAM_CATEGORY, &PINNAME_VIDEO_CAPTURE)?;
        stream_attributes.SetUINT32(&MF_DEVICESTREAM_STREAM_ID, 0)?;
        stream_attributes.SetUINT32(&MF_DEVICESTREAM_FRAMESERVER_SHARED, 1)?;
        stream_attributes.SetUINT32(
            &MF_DEVICESTREAM_ATTRIBUTE_FRAMESOURCE_TYPES,
            MFFrameSourceTypes_Color.0 as u32,
        )?;
        stream_attributes.CopyAllItems(&descriptor)?;
        let presentation = MFCreatePresentationDescriptor(Some(&[Some(descriptor.clone())]))?;
        presentation.SelectStream(0)?;
        let mut attributes = None;
        MFCreateAttributes(&mut attributes, 1)?;
        let source = ComObject::new(Source {
            queue: MFCreateEventQueue()?,
            attributes: attributes.ok_or(E_POINTER)?,
            presentation,
            stream_attributes: stream_attributes.clone(),
            stream: Mutex::new(None),
        });
        let stream = ComObject::new(Stream::new(source.cast()?, descriptor)?);
        *source.stream.lock().unwrap_or_else(|p| p.into_inner()) = Some(stream);
        LIVE.fetch_add(1, Ordering::AcqRel);
        Ok(source.into_interface())
    }
}

#[implement(IMFMediaSourceEx, IMFGetService, IKsControl)]
struct Source {
    queue: IMFMediaEventQueue,
    attributes: IMFAttributes,
    presentation: IMFPresentationDescriptor,
    stream_attributes: IMFAttributes,
    /// `None` after Shutdown.
    stream: Mutex<Option<ComObject<Stream>>>,
}

impl Drop for Source {
    fn drop(&mut self) {
        LIVE.fetch_sub(1, Ordering::AcqRel);
    }
}

impl Source {
    fn stream(&self) -> Result<ComObject<Stream>> {
        self.stream
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
            .ok_or_else(|| MF_E_SHUTDOWN.into())
    }
}

impl IMFMediaEventGenerator_Impl for Source_Impl {
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

impl IMFMediaSource_Impl for Source_Impl {
    fn GetCharacteristics(&self) -> Result<u32> {
        self.stream()?;
        Ok(MFMEDIASOURCE_IS_LIVE.0 as u32)
    }

    fn CreatePresentationDescriptor(&self) -> Result<IMFPresentationDescriptor> {
        self.stream()?;
        unsafe { self.presentation.Clone() }
    }

    fn Start(
        &self,
        presentation: Ref<IMFPresentationDescriptor>,
        time_format: *const GUID,
        _start: *const PROPVARIANT,
    ) -> Result<()> {
        let stream = self.stream()?;
        let presentation = presentation.ok()?;
        if !time_format.is_null() && unsafe { *time_format } != GUID::zeroed() {
            return Err(MF_E_UNSUPPORTED_TIME_FORMAT.into());
        }
        let start_pos = if _start.is_null() {
            PROPVARIANT::default()
        } else {
            unsafe { (*_start).clone() }
        };
        unsafe {
            for i in 0..presentation.GetStreamDescriptorCount()? {
                let mut selected = Default::default();
                let mut descriptor = None;
                presentation.GetStreamDescriptorByIndex(i, &mut selected, &mut descriptor)?;
                let descriptor = descriptor.ok_or(E_POINTER)?;
                if descriptor.GetStreamIdentifier()? != 0 {
                    return Err(E_INVALIDARG.into());
                }
                if selected.as_bool() {
                    let event = if stream.is_running() {
                        MEUpdatedStream
                    } else {
                        MENewStream
                    };
                    self.queue.QueueEventParamUnk(
                        event.0 as u32,
                        &GUID::zeroed(),
                        HRESULT(0),
                        &stream.to_interface::<IUnknown>(),
                    )?;
                    stream.start(&descriptor, &start_pos)?;
                } else if stream.is_running() {
                    stream.stop()?;
                }
            }
            self.queue.QueueEventParamVar(
                MESourceStarted.0 as u32,
                &GUID::zeroed(),
                HRESULT(0),
                &start_pos,
            )
        }
    }

    fn Stop(&self) -> Result<()> {
        let stream = self.stream()?;
        if stream.is_running() {
            stream.stop()?;
        }
        let empty = PROPVARIANT::default();
        unsafe {
            self.queue.QueueEventParamVar(
                MESourceStopped.0 as u32,
                &GUID::zeroed(),
                HRESULT(0),
                &empty,
            )
        }
    }

    fn Pause(&self) -> Result<()> {
        Err(MF_E_INVALID_STATE_TRANSITION.into())
    }

    fn Shutdown(&self) -> Result<()> {
        if let Some(stream) = self.stream.lock().unwrap_or_else(|p| p.into_inner()).take() {
            stream.shutdown();
        }
        unsafe { self.queue.Shutdown() }
    }
}

impl IMFMediaSourceEx_Impl for Source_Impl {
    fn GetSourceAttributes(&self) -> Result<IMFAttributes> {
        Ok(self.attributes.clone())
    }

    fn GetStreamAttributes(&self, id: u32) -> Result<IMFAttributes> {
        if id != 0 {
            return Err(E_INVALIDARG.into());
        }
        Ok(self.stream_attributes.clone())
    }

    fn SetD3DManager(&self, _manager: Ref<IUnknown>) -> Result<()> {
        Ok(())
    }
}

impl IMFGetService_Impl for Source_Impl {
    fn GetService(
        &self,
        _service: *const GUID,
        _iid: *const GUID,
        _out: *mut *mut c_void,
    ) -> Result<()> {
        Err(MF_E_UNSUPPORTED_SERVICE.into())
    }
}

/// The camera has no controls (focus, exposure and so on stay on the phone).
impl IKsControl_Impl for Source_Impl {
    fn KsProperty(
        &self,
        _: *const KSIDENTIFIER,
        _: u32,
        _: *mut c_void,
        _: u32,
        _: *mut u32,
    ) -> Result<()> {
        Err(HRESULT::from_win32(ERROR_SET_NOT_FOUND.0).into())
    }
    fn KsMethod(
        &self,
        _: *const KSIDENTIFIER,
        _: u32,
        _: *mut c_void,
        _: u32,
        _: *mut u32,
    ) -> Result<()> {
        Err(HRESULT::from_win32(ERROR_SET_NOT_FOUND.0).into())
    }
    fn KsEvent(
        &self,
        _: *const KSIDENTIFIER,
        _: u32,
        _: *mut c_void,
        _: u32,
        _: *mut u32,
    ) -> Result<()> {
        Err(HRESULT::from_win32(ERROR_SET_NOT_FOUND.0).into())
    }
}
