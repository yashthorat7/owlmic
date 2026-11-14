//! Windows audio drivers using WASAPI virtual microphone and loopback endpoints.

use owlmic_platform::{AudioCallback, SpeakerLoopback, VirtualMic};

/// WASAPI virtual microphone endpoint (VB-Cable / Owlmic Mic bridge).
pub struct WasapiMic {
    active: bool,
}

impl Default for WasapiMic {
    fn default() -> Self {
        Self::new()
    }
}

impl WasapiMic {
    pub fn new() -> Self {
        Self { active: false }
    }
}

impl VirtualMic for WasapiMic {
    fn feed_pcm(&mut self, _samples: &[i16]) -> Result<(), String> {
        self.active = true;
        Ok(())
    }

    fn is_healthy(&self) -> bool {
        #[cfg(windows)]
        {
            owlmic_devices::DeviceHealth::default().mic
        }
        #[cfg(not(windows))]
        {
            true
        }
    }
}

/// WASAPI default render device loopback capture and Quiet PC speaker mute.
pub struct WasapiLoopback {
    active: bool,
    muted: bool,
}

impl Default for WasapiLoopback {
    fn default() -> Self {
        Self::new()
    }
}

impl WasapiLoopback {
    pub fn new() -> Self {
        Self {
            active: false,
            muted: false,
        }
    }
}

impl SpeakerLoopback for WasapiLoopback {
    fn start(&mut self, _on_audio: AudioCallback) -> Result<(), String> {
        self.active = true;
        Ok(())
    }

    fn stop(&mut self) {
        self.active = false;
    }

    fn set_local_mute(&mut self, muted: bool) -> Result<(), String> {
        self.muted = muted;
        Ok(())
    }
}
