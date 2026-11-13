//! Quiet PC speakers (SYSTEM_DESIGN section 17.3): while Speaker runs, the default output is
//! muted so the sound comes only from the phone. The device is recorded before muting, so a
//! crash is undone at the next start. On devices whose mute also silences the loopback, Owlmic
//! unmutes and remembers not to mute that device again.

use owlmic_settings::store::Store;
use std::sync::Arc;
use std::time::{Duration, Instant};
use windows::Win32::Media::Audio::Endpoints::{IAudioEndpointVolume, IAudioMeterInformation};
use windows::Win32::Media::Audio::{IMMDevice, eConsole, eRender};
use windows::Win32::System::Com::{CLSCTX_ALL, CoTaskMemFree};
use windows::core::{GUID, HSTRING};

/// Sound playing that the loopback doesn't hear for this long means muting broke the loopback.
const SILENT_FOR: Duration = Duration::from_secs(1);
/// Only the first seconds after muting are checked.
const CHECK_WINDOW: Duration = Duration::from_secs(10);
const PLAYING: f32 = 0.01;
const HEARD: f32 = 0.0005;

struct Muted {
    id: String,
    volume: IAudioEndpointVolume,
    meter: Option<IAudioMeterInformation>,
    since: Instant,
    unheard_since: Option<Instant>,
}

pub struct Quiet {
    store: Arc<Store>,
    muted: Option<Muted>,
}

fn device_id(device: &IMMDevice) -> Option<String> {
    unsafe {
        let id = device.GetId().ok()?;
        let text = id.to_string().ok();
        CoTaskMemFree(Some(id.0 as _));
        text
    }
}

fn set_mute(volume: &IAudioEndpointVolume, mute: bool) -> bool {
    unsafe { volume.SetMute(mute, &GUID::zeroed()).is_ok() }
}

impl Quiet {
    /// Undoes a mute a crashed Owlmic left behind.
    pub fn new(store: Arc<Store>) -> Self {
        if let Some(id) = store.read(|d| d.speaker.muted_by_owlmic.clone()) {
            let restored = unsafe {
                crate::audio::enumerator()
                    .and_then(|e| e.GetDevice(&HSTRING::from(&id)))
                    .and_then(|d| d.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None))
                    .map(|v| set_mute(&v, false))
            };
            // A device that is gone can't be unmuted; forget it either way.
            let _ = restored;
            store.update(|d| d.speaker.muted_by_owlmic = None);
        }
        Self { store, muted: None }
    }

    /// Mutes the default output unless it is already muted or known to break the loopback.
    pub fn engage(&mut self) {
        if self.muted.is_some() {
            return;
        }
        let Some(device) = crate::audio::enumerator()
            .ok()
            .and_then(|e| unsafe { e.GetDefaultAudioEndpoint(eRender, eConsole).ok() })
        else {
            return;
        };
        let Some(id) = device_id(&device) else { return };
        if self
            .store
            .read(|d| d.speaker.unmutable_devices.contains(&id))
        {
            return;
        }
        let Ok(volume) = (unsafe { device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) })
        else {
            return;
        };
        if unsafe { volume.GetMute() }.is_ok_and(|m| m.as_bool()) {
            return;
        }
        self.store
            .update(|d| d.speaker.muted_by_owlmic = Some(id.clone()));
        if !set_mute(&volume, true) {
            self.store.update(|d| d.speaker.muted_by_owlmic = None);
            return;
        }
        let meter = unsafe {
            device
                .Activate::<IAudioMeterInformation>(CLSCTX_ALL, None)
                .ok()
        };
        self.muted = Some(Muted {
            id,
            volume,
            meter,
            since: Instant::now(),
            unheard_since: None,
        });
    }

    /// The default output moved while Owlmic had muted the old one: the old one is unmuted
    /// and the new one muted instead.
    pub fn follow_default(&mut self) {
        let Some(m) = self.muted.as_ref() else { return };
        let new = crate::audio::enumerator()
            .ok()
            .and_then(|e| unsafe { e.GetDefaultAudioEndpoint(eRender, eConsole).ok() })
            .and_then(|d| device_id(&d));
        if new.is_some_and(|id| id != m.id) {
            self.release();
            self.engage();
        }
    }

    /// Unmutes what Owlmic muted.
    pub fn release(&mut self) {
        if let Some(m) = self.muted.take() {
            set_mute(&m.volume, false);
            self.store.update(|d| d.speaker.muted_by_owlmic = None);
        }
    }

    /// The self-check, called a few times a second while muted with the loopback's last peak.
    pub fn check(&mut self, loopback_peak: f32, now: Instant) {
        let Some(m) = self.muted.as_mut() else { return };
        if now.duration_since(m.since) > CHECK_WINDOW {
            return;
        }
        let playing = m
            .meter
            .as_ref()
            .and_then(|meter| unsafe { meter.GetPeakValue().ok() })
            .unwrap_or(0.0)
            > PLAYING;
        if !playing || loopback_peak > HEARD {
            m.unheard_since = None;
            return;
        }
        let since = *m.unheard_since.get_or_insert(now);
        if now.duration_since(since) >= SILENT_FOR {
            let id = m.id.clone();
            self.release();
            self.store.update(|d| d.speaker.unmutable_devices.push(id));
        }
    }

    pub fn is_checking(&self, now: Instant) -> bool {
        self.muted
            .as_ref()
            .is_some_and(|m| now.duration_since(m.since) <= CHECK_WINDOW)
    }
}
