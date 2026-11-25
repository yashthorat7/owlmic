//! Speaker (SYSTEM_DESIGN section 17.3): whatever the PC plays, captured from the default output
//! with WASAPI loopback, paced cleanly at 10 ms cadence using a high-resolution timer, converted
//! to 48 kHz stereo and handed to the speaker sender. Follows changes of the default output.

use crate::audio::{Com, Format, Handle, enumerator};
use owlmic_media::audio::convert::ToStereo48k;
use owlmic_media::audio::speaker::SpeakerSender;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{E_FAIL, HANDLE, PROPERTYKEY, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::{
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
    DEVICE_STATE, EDataFlow, ERole, IAudioCaptureClient, IAudioClient, IMMNotificationClient,
    IMMNotificationClient_Impl, eConsole, eRender,
};
use windows::Win32::System::Com::{CLSCTX_ALL, CoTaskMemFree};
use windows::Win32::System::Threading::{
    AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW,
    CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CreateEventW, CreateWaitableTimerExW,
    CreateWaitableTimerW, SetEvent, SetWaitableTimer, TIMER_ALL_ACCESS, WaitForMultipleObjects,
    WaitForSingleObject,
};
use windows::core::{PCWSTR, Result, implement, w};

/// 200 ms of device buffer: room for a late tick.
const BUFFER: i64 = 2_000_000;
const TICK: Duration = Duration::from_millis(10);
const FRAME_FLOATS: usize = 48 * 10 * 2;
/// Cap staging queue at 60 ms so latency stays bounded across WASAPI bursts; trim excess back to 20 ms.
const MAX_QUEUED_FLOATS: usize = FRAME_FLOATS * 6;
const TRIM_TO_FLOATS: usize = FRAME_FLOATS * 2;
/// Two empty ticks (20 ms) before emitting paced silence, riding out 10 ms WASAPI delivery jitter.
const IDLE_SILENCE_TICKS: u32 = 2;
static SILENCE: [f32; FRAME_FLOATS] = [0.0; FRAME_FLOATS];

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

fn create_timer() -> Result<Handle> {
    unsafe {
        let t = CreateWaitableTimerExW(
            None,
            PCWSTR::null(),
            CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
            TIMER_ALL_ACCESS.0,
        )
        .or_else(|_| CreateWaitableTimerW(None, false, PCWSTR::null()))?;
        Ok(Handle(t))
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
    let timer = create_timer().ok();
    let mut backoff_ms = 50u32;
    while !stop.load(Ordering::Acquire) {
        changed.store(false, Ordering::Release);
        match capture(
            sender,
            level,
            stop,
            &changed,
            wake.0,
            timer.as_ref().map(|h| h.0),
        ) {
            Ok(()) => backoff_ms = 50,
            Err(_) => {
                unsafe { WaitForSingleObject(wake.0, backoff_ms) };
                backoff_ms = (backoff_ms * 2).min(400);
            }
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

fn wait_until(wake: HANDLE, timer: Option<HANDLE>, until: Instant) {
    let wait = until.saturating_duration_since(Instant::now());
    if wait.is_zero() {
        return;
    }
    unsafe {
        if let Some(t) = timer {
            let due_100ns = -((wait.as_nanos() / 100).max(1) as i64);
            if SetWaitableTimer(t, &due_100ns, 0, None, None, false).is_ok() {
                let handles = [wake, t];
                let _ = WaitForMultipleObjects(&handles, false, 50);
                return;
            }
        }
        let ms = (wait.as_millis() as u32).clamp(1, 20);
        let _ = WaitForSingleObject(wake, ms);
    }
}

fn capture(
    sender: &SpeakerSender,
    level: &AtomicU32,
    stop: &AtomicBool,
    changed: &AtomicBool,
    wake: HANDLE,
    timer: Option<HANDLE>,
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
        let mut queued = VecDeque::with_capacity(MAX_QUEUED_FLOATS);
        let mut frame_buf = [0f32; FRAME_FLOATS];
        let mut idle_ticks = 0u32;
        let mut next_tick = Instant::now() + TICK;
        client.Start()?;
        let result = loop {
            wait_until(wake, timer, next_tick);
            if stop.load(Ordering::Acquire) || changed.load(Ordering::Acquire) {
                break Ok(());
            }
            let now = Instant::now();
            next_tick += TICK;
            if now.saturating_duration_since(next_tick) > Duration::from_millis(40) {
                next_tick = now + TICK;
            }
            let mut tick_peak = 0f32;
            let mut got_frames = false;
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
                let needed = frames as usize * format.channels;
                if needed > scratch.len() {
                    scratch.resize(needed, 0.0);
                }
                let samples = &mut scratch[..needed];
                if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
                    samples.fill(0.0);
                } else {
                    format.read(data, samples);
                }
                let _ = input.ReleaseBuffer(frames);
                if frames > 0 {
                    got_frames = true;
                    let stereo = convert.convert(samples);
                    tick_peak = stereo.iter().fold(tick_peak, |p, s| p.max(s.abs()));
                    queued.extend(stereo.iter().copied());
                }
            }
            if got_frames {
                idle_ticks = 0;
                level.store(tick_peak.to_bits(), Ordering::Relaxed);
            } else {
                idle_ticks = idle_ticks.saturating_add(1);
                if idle_ticks >= IDLE_SILENCE_TICKS {
                    level.store(0f32.to_bits(), Ordering::Relaxed);
                }
            }
            if queued.len() > MAX_QUEUED_FLOATS {
                let drop_n = queued.len() - TRIM_TO_FLOATS;
                queued.drain(..drop_n);
            }
            if queued.len() >= FRAME_FLOATS {
                for dst in &mut frame_buf {
                    *dst = queued.pop_front().unwrap_or(0.0);
                }
                sender.push(&frame_buf);
            } else if idle_ticks >= IDLE_SILENCE_TICKS {
                if queued.is_empty() {
                    sender.push(&SILENCE);
                } else {
                    let n = queued.len();
                    for dst in &mut frame_buf[..n] {
                        *dst = queued.pop_front().unwrap_or(0.0);
                    }
                    frame_buf[n..].fill(0.0);
                    sender.push(&frame_buf);
                }
            }
        };
        let _ = WAIT_OBJECT_0;
        finish(&client, result)
    }
}

fn finish(client: &IAudioClient, result: Result<()>) -> Result<()> {
    unsafe {
        let _ = client.Stop();
    }
    result
}
