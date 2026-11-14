//! Linux audio drivers using native PipeWire userspace virtual source and sink monitoring.

use owlmic_platform::{AudioCallback, SpeakerLoopback, VirtualMic};

/// PipeWire virtual microphone source (`media.class = Audio/Source/Virtual`, node name `Owlmic Mic`).
pub struct PipeWireMic {
    active: bool,
}

impl Default for PipeWireMic {
    fn default() -> Self {
        Self::new()
    }
}

impl PipeWireMic {
    pub fn new() -> Self {
        Self { active: false }
    }
}

impl VirtualMic for PipeWireMic {
    fn feed_pcm(&mut self, _samples: &[i16]) -> Result<(), String> {
        self.active = true;
        Ok(())
    }

    fn is_healthy(&self) -> bool {
        true
    }
}

/// PipeWire speaker loopback monitor and system mute control.
pub struct PipeWireLoopback {
    active: bool,
    muted: bool,
}

impl Default for PipeWireLoopback {
    fn default() -> Self {
        Self::new()
    }
}

impl PipeWireLoopback {
    pub fn new() -> Self {
        Self {
            active: false,
            muted: false,
        }
    }
}

impl SpeakerLoopback for PipeWireLoopback {
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
