//! H.264 to NV12 with the Media Foundation decoder in low-latency mode (SYSTEM_DESIGN section
//! 17.2). Software decoding: no Direct3D device is handed to it.

use owlmic_media::video::Nv12;
use std::mem::ManuallyDrop;
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::core::{Interface, Result};

pub struct Decoder {
    mft: IMFTransform,
    /// Reused output sample when the decoder doesn't provide its own.
    out: Option<IMFSample>,
    /// Coded (w, h) and visible (w, h) (macroblock rows/cols are padded to 16).
    size: (usize, usize, usize, usize),
    picture: Nv12,
}

impl Decoder {
    pub fn new() -> Result<Self> {
        unsafe {
            let mft: IMFTransform =
                CoCreateInstance(&CLSID_MSH264DecoderMFT, None, CLSCTX_INPROC_SERVER)?;
            let _ = mft.GetAttributes().map(|a| a.SetUINT32(&MF_LOW_LATENCY, 1));
            if let Ok(codec) = mft.cast::<ICodecAPI>() {
                let mut var = windows::Win32::System::Variant::VARIANT::default();
                (*var.Anonymous.Anonymous).vt = windows::Win32::System::Variant::VT_UI4;
                (*var.Anonymous.Anonymous).Anonymous.ulVal = 1;
                let _ = codec.SetValue(&CODECAPI_AVLowLatencyMode, &var);
            }
            let input = MFCreateMediaType()?;
            input.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            input.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
            mft.SetInputType(0, &input, 0)?;
            let mut d = Self {
                mft,
                out: None,
                size: (0, 0, 0, 0),
                picture: Nv12::black(2, 2),
            };
            d.choose_output()?;
            d.mft
                .ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)?;
            d.mft
                .ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)?;
            Ok(d)
        }
    }

    /// Picks NV12 out, and sizes the output sample for it.
    fn choose_output(&mut self) -> Result<()> {
        unsafe {
            let mut i = 0;
            let chosen = loop {
                let t = self.mft.GetOutputAvailableType(0, i)?;
                if t.GetGUID(&MF_MT_SUBTYPE)? == MFVideoFormat_NV12 {
                    break t;
                }
                i += 1;
            };
            self.mft.SetOutputType(0, &chosen, 0)?;
            let current = self.mft.GetOutputCurrentType(0).unwrap_or(chosen.clone());
            let packed = current
                .GetUINT64(&MF_MT_FRAME_SIZE)
                .or_else(|_| chosen.GetUINT64(&MF_MT_FRAME_SIZE))
                .unwrap_or(0);
            let (coded_w, coded_h) = ((packed >> 32) as usize, (packed & 0xFFFF_FFFF) as usize);
            let mut visible_w = coded_w;
            let mut visible_h = coded_h;
            let mut area = MFVideoArea::default();
            let has_aperture = current
                .GetBlob(
                    &MF_MT_MINIMUM_DISPLAY_APERTURE,
                    std::slice::from_raw_parts_mut(
                        &mut area as *mut _ as *mut u8,
                        size_of::<MFVideoArea>(),
                    ),
                    None,
                )
                .is_ok()
                || chosen
                    .GetBlob(
                        &MF_MT_MINIMUM_DISPLAY_APERTURE,
                        std::slice::from_raw_parts_mut(
                            &mut area as *mut _ as *mut u8,
                            size_of::<MFVideoArea>(),
                        ),
                        None,
                    )
                    .is_ok();
            if has_aperture {
                if area.Area.cx > 0 {
                    visible_w = (area.Area.cx as usize).min(coded_w);
                }
                if area.Area.cy > 0 {
                    visible_h = (area.Area.cy as usize).min(coded_h);
                }
            }
            self.size = (coded_w, coded_h, visible_w & !1, visible_h & !1);
            let info = self.mft.GetOutputStreamInfo(0)?;
            self.out = if info.dwFlags & MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32 != 0 {
                None
            } else {
                let sample = MFCreateSample()?;
                sample.AddBuffer(&MFCreateMemoryBuffer(
                    info.cbSize.max((coded_w * coded_h * 3 / 2) as u32).max(1),
                )?)?;
                Some(sample)
            };
            Ok(())
        }
    }

    /// One whole frame in; the decoded picture when the decoder has one ready.
    pub fn decode(&mut self, frame: &[u8], timestamp_us: u32) -> Result<Option<&Nv12>> {
        unsafe {
            let buffer = MFCreateMemoryBuffer(frame.len() as u32)?;
            let mut data = std::ptr::null_mut();
            buffer.Lock(&mut data, None, None)?;
            std::ptr::copy_nonoverlapping(frame.as_ptr(), data, frame.len());
            buffer.Unlock()?;
            buffer.SetCurrentLength(frame.len() as u32)?;
            let sample = MFCreateSample()?;
            sample.AddBuffer(&buffer)?;
            let _ = sample.SetSampleTime(timestamp_us as i64 * 10);
            if let Err(e) = self.mft.ProcessInput(0, &sample, 0) {
                if e.code() != MF_E_NOTACCEPTING {
                    return Err(e);
                }
                // A picture is waiting; take it, then the input fits.
                let got = self.drain()?;
                self.mft.ProcessInput(0, &sample, 0)?;
                if got {
                    return Ok(Some(&self.picture));
                }
            }
            Ok(self.drain()?.then_some(&self.picture))
        }
    }

    /// Pulls every ready picture, keeping the newest. True if there was one.
    fn drain(&mut self) -> Result<bool> {
        let mut got = false;
        loop {
            if let Some(s) = &self.out {
                unsafe {
                    let b = s.GetBufferByIndex(0)?;
                    b.SetCurrentLength(0)?;
                }
            }
            let mut output = [MFT_OUTPUT_DATA_BUFFER {
                dwStreamID: 0,
                pSample: ManuallyDrop::new(self.out.clone()),
                dwStatus: 0,
                pEvents: ManuallyDrop::new(None),
            }];
            let mut status = 0;
            let result = unsafe { self.mft.ProcessOutput(0, &mut output, &mut status) };
            let sample = unsafe { ManuallyDrop::take(&mut output[0].pSample) };
            unsafe { drop(ManuallyDrop::take(&mut output[0].pEvents)) };
            match result {
                Ok(()) => {
                    if let Some(s) = sample {
                        self.copy_out(&s)?;
                        got = true;
                    }
                }
                Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => return Ok(got),
                Err(e) if e.code() == MF_E_TRANSFORM_STREAM_CHANGE => self.choose_output()?,
                Err(e) => return Err(e),
            }
        }
    }

    fn copy_out(&mut self, sample: &IMFSample) -> Result<()> {
        let (coded_w, coded_h, visible_w, visible_h) = self.size;
        if visible_w == 0 || visible_h == 0 {
            return Err(E_FAIL.into());
        }
        if (self.picture.width, self.picture.height) != (visible_w, visible_h) {
            self.picture = Nv12::black(visible_w, visible_h);
        }
        unsafe {
            let buffer = sample.GetBufferByIndex(0)?;
            let (mut base, mut pitch) = (std::ptr::null_mut(), 0i32);
            let planar = buffer.cast::<IMF2DBuffer>().ok();
            let locked_2d = planar
                .as_ref()
                .is_some_and(|p| p.Lock2D(&mut base, &mut pitch).is_ok());
            if !locked_2d {
                buffer.Lock(&mut base, None, None)?;
                pitch = coded_w as i32;
            }
            let pitch = pitch.unsigned_abs() as usize;
            let dst = &mut self.picture.data;
            for row in 0..visible_h {
                std::ptr::copy_nonoverlapping(
                    base.add(row * pitch),
                    dst.as_mut_ptr().add(row * visible_w),
                    visible_w,
                );
            }
            let uv = base.add(pitch * coded_h);
            for row in 0..visible_h / 2 {
                std::ptr::copy_nonoverlapping(
                    uv.add(row * pitch),
                    dst.as_mut_ptr()
                        .add(visible_w * visible_h + row * visible_w),
                    visible_w,
                );
            }
            if locked_2d {
                planar.unwrap().Unlock2D()?;
            } else {
                buffer.Unlock()?;
            }
            let _ = buffer.SetCurrentLength(0);
        }
        Ok(())
    }

    /// Drops everything queued in the decoder, after lost data.
    pub fn flush(&mut self) {
        unsafe {
            let _ = self.mft.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0);
            if let Some(s) = &self.out
                && let Ok(b) = s.GetBufferByIndex(0)
            {
                let _ = b.SetCurrentLength(0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "needs Media Foundation, which Windows N and Server editions lack"]
    fn the_decoder_opens_with_nv12_out() {
        let _com = crate::audio::Com::init();
        let mut d = Decoder::new().unwrap();
        assert!(
            d.decode(&[0, 0, 0, 1, 0x09, 0xF0], 0)
                .is_ok_and(|p| p.is_none())
        );
        assert!(
            d.decode(&[0, 0, 0, 1, 0x09, 0xF0], 33_000)
                .is_ok_and(|p| p.is_none())
        );
        assert!(
            d.decode(&[0, 0, 0, 1, 0x09, 0xF0], 66_000)
                .is_ok_and(|p| p.is_none())
        );
    }

    #[test]
    #[ignore = "needs Media Foundation, which Windows N and Server editions lack"]
    fn consecutive_frames_decode_without_stalling() {
        let _com = crate::audio::Com::init();
        unsafe {
            let enc: IMFTransform =
                CoCreateInstance(&CLSID_MSH264EncoderMFT, None, CLSCTX_INPROC_SERVER).unwrap();
            let out_type = MFCreateMediaType().unwrap();
            out_type
                .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
                .unwrap();
            out_type
                .SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)
                .unwrap();
            out_type.SetUINT32(&MF_MT_AVG_BITRATE, 1_000_000).unwrap();
            let (w, h) = (320u32, 240u32);
            out_type
                .SetUINT64(&MF_MT_FRAME_SIZE, ((w as u64) << 32) | (h as u64))
                .unwrap();
            out_type
                .SetUINT64(&MF_MT_FRAME_RATE, (30u64 << 32) | 1)
                .unwrap();
            out_type
                .SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
                .unwrap();
            enc.SetOutputType(0, &out_type, 0).unwrap();

            let in_type = MFCreateMediaType().unwrap();
            in_type
                .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
                .unwrap();
            in_type
                .SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)
                .unwrap();
            in_type
                .SetUINT64(&MF_MT_FRAME_SIZE, ((w as u64) << 32) | (h as u64))
                .unwrap();
            in_type
                .SetUINT64(&MF_MT_FRAME_RATE, (30u64 << 32) | 1)
                .unwrap();
            in_type
                .SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
                .unwrap();
            enc.SetInputType(0, &in_type, 0).unwrap();

            enc.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)
                .unwrap();
            enc.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)
                .unwrap();

            let mut encoded_frames = Vec::new();
            for frame_idx in 0..5 {
                let sample = MFCreateSample().unwrap();
                let buf_size = (w * h * 3 / 2) as usize;
                let buf = MFCreateMemoryBuffer(buf_size as u32).unwrap();
                let mut ptr = std::ptr::null_mut();
                buf.Lock(&mut ptr, None, None).unwrap();
                std::ptr::write_bytes(ptr, (frame_idx * 40) as u8, buf_size);
                buf.Unlock().unwrap();
                buf.SetCurrentLength(buf_size as u32).unwrap();
                sample.AddBuffer(&buf).unwrap();
                sample.SetSampleTime(frame_idx as i64 * 333_333).unwrap();
                sample.SetSampleDuration(333_333).unwrap();

                enc.ProcessInput(0, &sample, 0).unwrap();

                loop {
                    let info = enc.GetOutputStreamInfo(0).unwrap();
                    let out_sample = MFCreateSample().unwrap();
                    let out_buf = MFCreateMemoryBuffer(info.cbSize.max(65536)).unwrap();
                    out_sample.AddBuffer(&out_buf).unwrap();

                    let mut output = [MFT_OUTPUT_DATA_BUFFER {
                        dwStreamID: 0,
                        pSample: ManuallyDrop::new(Some(out_sample)),
                        dwStatus: 0,
                        pEvents: ManuallyDrop::new(None),
                    }];
                    let mut status = 0;
                    match enc.ProcessOutput(0, &mut output, &mut status) {
                        Ok(()) => {
                            let s = ManuallyDrop::take(&mut output[0].pSample).unwrap();
                            let _ = ManuallyDrop::take(&mut output[0].pEvents);
                            let b = s.GetBufferByIndex(0).unwrap();
                            let mut data_ptr = std::ptr::null_mut();
                            let mut len = 0;
                            b.Lock(&mut data_ptr, None, Some(&mut len)).unwrap();
                            let slice = std::slice::from_raw_parts(data_ptr, len as usize);
                            encoded_frames.push(slice.to_vec());
                            b.Unlock().unwrap();
                        }
                        Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => {
                            let _ = ManuallyDrop::take(&mut output[0].pSample);
                            let _ = ManuallyDrop::take(&mut output[0].pEvents);
                            break;
                        }
                        Err(e) => panic!("encoder error: {:?}", e),
                    }
                }
            }

            let _ = enc.ProcessMessage(MFT_MESSAGE_COMMAND_DRAIN, 0);
            loop {
                let info = enc.GetOutputStreamInfo(0).unwrap();
                let out_sample = MFCreateSample().unwrap();
                let out_buf = MFCreateMemoryBuffer(info.cbSize.max(65536)).unwrap();
                out_sample.AddBuffer(&out_buf).unwrap();

                let mut output = [MFT_OUTPUT_DATA_BUFFER {
                    dwStreamID: 0,
                    pSample: ManuallyDrop::new(Some(out_sample)),
                    dwStatus: 0,
                    pEvents: ManuallyDrop::new(None),
                }];
                let mut status = 0;
                match enc.ProcessOutput(0, &mut output, &mut status) {
                    Ok(()) => {
                        let s = ManuallyDrop::take(&mut output[0].pSample).unwrap();
                        let _ = ManuallyDrop::take(&mut output[0].pEvents);
                        let b = s.GetBufferByIndex(0).unwrap();
                        let mut data_ptr = std::ptr::null_mut();
                        let mut len = 0;
                        b.Lock(&mut data_ptr, None, Some(&mut len)).unwrap();
                        let slice = std::slice::from_raw_parts(data_ptr, len as usize);
                        encoded_frames.push(slice.to_vec());
                        b.Unlock().unwrap();
                    }
                    Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => {
                        let _ = ManuallyDrop::take(&mut output[0].pSample);
                        let _ = ManuallyDrop::take(&mut output[0].pEvents);
                        break;
                    }
                    Err(e) => panic!("encoder error: {:?}", e),
                }
            }

            assert!(!encoded_frames.is_empty());
            let mut dec = Decoder::new().unwrap();
            let mut decoded_count = 0;
            for (idx, packet) in encoded_frames.iter().enumerate() {
                if let Ok(Some(pic)) = dec.decode(packet, (idx as u32) * 33_333) {
                    decoded_count += 1;
                    assert_eq!((pic.width, pic.height), (w as usize, h as usize));
                }
            }
            assert!(decoded_count >= 2, "decoded multiple frames successfully");
        }
    }
}
