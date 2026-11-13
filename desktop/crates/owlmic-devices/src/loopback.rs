//! Speaker (SYSTEM_DESIGN section 17.3): whatever the PC plays, captured from the default output
//! with WASAPI loopback every 10 ms, made 48 kHz stereo and handed to the speaker sender. Follows
//! changes of the default output.

use crate::audio::{Com, Format, Handle, enumerator};
use owlmic_media::audio::convert::ToStereo48k;
use owlmic_media::audio::speaker::SpeakerSender;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{E_FAIL, HANDLE, PROPERTYKEY};
use windows::Win32::Media::Audio::{
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
    DEVICE_STATE, EDataFlow, ERole, IAudioCaptureClient, IAudioClient, IMMNotificationClient,
    IMMNotificationClient_Impl, eConsole, eRender,
};
use windows::Win32::System::Com::{CLSCTX_ALL, CoTaskMemFree};
use windows::Win32::System::Threading::{
    AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW, CreateEventW, SetEvent,
    WaitForSingleObject,
};
use windows::core::{PCWSTR, Result, implement, w};

/// 200 ms of device buffer: room for a late tick.
const BUFFER: i64 = 2_000_000;
const TICK_MS: u32 = 10;
/// Loopback goes quiet when nothing plays; after this gap silence is sent so the phone's
/// playback keeps its pace.
const GAP: Duration = Duration::from_millis(30);
static SILENCE: [f32; 960] = [0.0; 960];

pub struct SpeakerCapture {
    stop: Arc<AtomicBool>,
    wake: Arc<Handle>,
    thread: Option<JoinHandle<()>>,
}

impl SpeakerCapture {
    /// `level` receives the peak of the last tick as f32 bits, for the Quiet PC speakers check.
    pub fn start(sender: Arc<SpeakerSender>, level: Arc<AtomicU32>) -> Option<Self> {
        let wake = Arc::new(Handle(unsafe {
            CreateEventW(None, false, false, None).ok()?
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let (s, w) = (stop.clone(), wake.clone());
        let thread = std::thread::Builder::new()
            .name("speaker capture".into())
            .spawn(move || run(&sender, &level, &s, &w))
            .ok();
        Some(Self { stop, wake, thread })
    }
}

impl Drop for SpeakerCapture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        unsafe {
            let _ = SetEvent(self.wake.0);
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[implement(IMMNotificationClient)]
struct DefaultWatch {
    changed: Arc<AtomicBool>,
    wake: Arc<Handle>,
}

impl IMMNotificationClient_Impl for DefaultWatch_Impl {
    fn OnDeviceStateChanged(&self, _: &PCWSTR, _: DEVICE_STATE) -> Result<()> {
        Ok(())
    }
    fn OnDeviceAdded(&self, _: &PCWSTR) -> Result<()> {
        Ok(())
    }
    fn OnDeviceRemoved(&self, _: &PCWSTR) -> Result<()> {
        Ok(())
    }
    fn OnDefaultDeviceChanged(&self, flow: EDataFlow, role: ERole, _: &PCWSTR) -> Result<()> {
        if flow == eRender && role == eConsole {
            self.changed.store(true, Ordering::Release);
            unsafe { SetEvent(self.wake.0)? };
        }
        Ok(())
    }
    fn OnPropertyValueChanged(&self, _: &PCWSTR, _: &PROPERTYKEY) -> Result<()> {
        Ok(())
    }
}

fn run(sender: &SpeakerSender, level: &AtomicU32, stop: &AtomicBool, wake: &Arc<Handle>) {
    let _com = Com::init();
    let mut task = 0;
    let mmcss = unsafe { AvSetMmThreadCharacteristicsW(w!("Pro Audio"), &mut task) };
    let changed = Arc::new(AtomicBool::new(false));
    let watch: IMMNotificationClient = DefaultWatch {
        changed: changed.clone(),
        wake: wake.clone(),
    }
    .into();
    let devices = enumerator().ok();
    if let Some(e) = &devices {
        unsafe {
            let _ = e.RegisterEndpointNotificationCallback(&watch);
        }
    }
    while !stop.load(Ordering::Acquire) {
        changed.store(false, Ordering::Release);
        if capture(sender, level, stop, &changed, wake.0).is_err() {
            // No output device at all, or it went away: wait for a change or half a second.
            unsafe { WaitForSingleObject(wake.0, 500) };
        }
    }
    if let Some(e) = &devices {
        unsafe {
            let _ = e.UnregisterEndpointNotificationCallback(&watch);
        }
    }
    if let Ok(h) = mmcss {
        unsafe {
            let _ = AvRevertMmThreadCharacteristics(h);
        }
    }
}

fn capture(
    sender: &SpeakerSender,
    level: &AtomicU32,
    stop: &AtomicBool,
    changed: &AtomicBool,
    wake: HANDLE,
) -> Result<()> {
    unsafe {
        let device = enumerator()?.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        let mix = client.GetMixFormat()?;
        let format = Format::of(mix);
        let init = client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK,
            BUFFER,
            0,
            mix,
            None,
        );
        CoTaskMemFree(Some(mix as _));
        init?;
        let format = format.ok_or(E_FAIL)?;
        let input: IAudioCaptureClient = client.GetService()?;
        let mut scratch = vec![0f32; client.GetBufferSize()? as usize * format.channels];
        let mut convert = ToStereo48k::new(format.rate, format.channels);
        let mut last_data = Instant::now();
        client.Start()?;
        let result = loop {
            WaitForSingleObject(wake, TICK_MS);
            if stop.load(Ordering::Acquire) || changed.load(Ordering::Acquire) {
                break Ok(());
            }
            let mut peak = 0f32;
            loop {
                match input.GetNextPacketSize() {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(e) => return finish(&client, Err(e)),
                }
                let (mut data, mut frames, mut flags) = (std::ptr::null_mut(), 0u32, 0u32);
                if let Err(e) = input.GetBuffer(&mut data, &mut frames, &mut flags, None, None) {
                    return finish(&client, Err(e));
                }
                let n = (frames as usize * format.channels).min(scratch.len());
                let samples = &mut scratch[..n];
                if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
                    samples.fill(0.0);
                } else {
                    format.read(data, samples);
                }
                let _ = input.ReleaseBuffer(frames);
                let stereo = convert.convert(samples);
                peak = stereo.iter().fold(peak, |p, s| p.max(s.abs()));
                sender.push(stereo);
                last_data = Instant::now();
            }
            if last_data.elapsed() > GAP {
                sender.push(&SILENCE);
            }
            level.store(peak.to_bits(), Ordering::Relaxed);
        };
        finish(&client, result)
    }
}

fn finish(client: &IAudioClient, result: Result<()>) -> Result<()> {
    unsafe {
        let _ = client.Stop();
    }
    result
}
