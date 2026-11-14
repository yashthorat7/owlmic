//! Owlmic Mic (SYSTEM_DESIGN section 17.1): the phone's voice rendered into Owlmic Bridge, whose
//! other end is the microphone meeting apps record. Runs only while the phone's mic is on; with
//! nothing rendered the driver carries silence by itself.

use crate::audio::{Com, Format, Handle, enumerator, find};
use owlmic_media::audio::pipeline::JitterBuffer;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::Media::Audio::{
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_NOPERSIST,
    IAudioClient, IAudioRenderClient, eRender,
};
use windows::Win32::System::Com::{CLSCTX_ALL, CoTaskMemFree};
use windows::Win32::System::Threading::{
    AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW, CreateEventW,
    WaitForSingleObject,
};
use windows::core::{Result, w};

/// The renamed endpoint first, then the driver's own name on a PC where renaming failed.
pub const ENDPOINTS: [&str; 2] = [owlmic_ui::names::BRIDGE_DEVICE, "CABLE Input"];
/// 10 ms, in 100 ns units.
const PERIOD: i64 = 100_000;

pub struct MicOutput {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl MicOutput {
    pub fn start(jitter: Arc<JitterBuffer>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        let thread = std::thread::Builder::new()
            .name("mic render".into())
            .spawn(move || run(&jitter, &s))
            .ok();
        Self { stop, thread }
    }
}

impl Drop for MicOutput {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(jitter: &JitterBuffer, stop: &AtomicBool) {
    let _com = Com::init();
    let mut task = 0;
    let mmcss = unsafe { AvSetMmThreadCharacteristicsW(w!("Pro Audio"), &mut task) };
    while !stop.load(Ordering::Acquire) {
        // A missing or reset device: try again shortly (the hub's health check reports it).
        if render(jitter, stop).is_err() {
            for _ in 0..10 {
                if stop.load(Ordering::Acquire) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
    if let Ok(h) = mmcss {
        unsafe {
            let _ = AvRevertMmThreadCharacteristics(h);
        }
    }
}

fn render(jitter: &JitterBuffer, stop: &AtomicBool) -> Result<()> {
    unsafe {
        let device = find(&enumerator()?, eRender, &ENDPOINTS).ok_or(E_FAIL)?;
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        let mix = client.GetMixFormat()?;
        let format = Format::of(mix);
        let init = client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK | AUDCLNT_STREAMFLAGS_NOPERSIST,
            PERIOD,
            0,
            mix,
            None,
        );
        CoTaskMemFree(Some(mix as _));
        init?;
        let format = format.ok_or(E_FAIL)?;
        let event = Handle(CreateEventW(None, false, false, None)?);
        client.SetEventHandle(event.0)?;
        let out: IAudioRenderClient = client.GetService()?;
        let frames = client.GetBufferSize()?;
        jitter.set_output_rate(format.rate);
        let mut scratch = vec![0f32; frames as usize * format.channels];
        client.Start()?;
        let result = loop {
            if stop.load(Ordering::Acquire) {
                break Ok(());
            }
            WaitForSingleObject(event.0, 200);
            let padding = match client.GetCurrentPadding() {
                Ok(p) => p,
                Err(e) => break Err(e),
            };
            let n = frames.saturating_sub(padding);
            if n == 0 {
                continue;
            }
            let data = match out.GetBuffer(n) {
                Ok(d) => d,
                Err(e) => break Err(e),
            };
            let samples = &mut scratch[..n as usize * format.channels];
            jitter.pop_samples(samples, format.channels as u16);
            format.write(samples, data);
            if let Err(e) = out.ReleaseBuffer(n, 0) {
                break Err(e);
            }
        };
        let _ = client.Stop();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "needs the Owlmic Mic driver installed"]
    fn owlmic_bridge_is_found() {
        let _com = Com::init();
        assert!(find(&enumerator().unwrap(), eRender, &ENDPOINTS).is_some());
    }
}
