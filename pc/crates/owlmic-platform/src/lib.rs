//! Cross-platform driver and UI abstraction traits for Owlmic PC companion.
//!
//! Windows implementations live in `owlmic-devices` / `owlmic-ui` (WASAPI, MF, Direct2D/GDI).
//! Linux implementations live in `owlmic-linux` (PipeWire, v4l2loopback, ksni tray).

/// Virtual microphone driver abstraction: feeds 48 kHz mono PCM from the phone into the OS audio subsystem.
pub trait VirtualMic: Send + Sync {
    /// Feeds a chunk of 48 kHz 16-bit mono PCM samples to the virtual microphone endpoint.
    fn feed_pcm(&mut self, samples: &[i16]) -> Result<(), String>;

    /// Returns whether the virtual microphone endpoint is currently registered and healthy.
    fn is_healthy(&self) -> bool;
}

/// Virtual camera driver abstraction: feeds decoded NV12/YUYV video frames into the OS camera subsystem.
pub trait VirtualCam: Send + Sync {
    /// Delivers an NV12 frame to the virtual camera device.
    fn feed_frame(&mut self, width: u32, height: u32, nv12: &[u8]) -> Result<(), String>;

    /// Returns whether the virtual camera driver is registered and available to user applications.
    fn is_healthy(&self) -> bool;
}

/// Audio callback type for receiving captured PCM floating-point chunks.
pub type AudioCallback = Box<dyn FnMut(&[f32]) + Send>;

/// Speaker loopback capture abstraction: captures PC audio output and routes it to the phone.
pub trait SpeakerLoopback: Send + Sync {
    /// Starts capturing default audio output as 48 kHz stereo floating-point PCM.
    fn start(&mut self, on_audio: AudioCallback) -> Result<(), String>;

    /// Stops audio capture.
    fn stop(&mut self);

    /// Mutes or unmutes the local PC speakers so only the phone plays audio.
    fn set_local_mute(&mut self, muted: bool) -> Result<(), String>;
}

/// Desktop UI and tray coordinator abstraction.
pub trait TrayUi: Send {
    /// Runs the system tray event loop.
    fn run(&mut self) -> Result<(), String>;

    /// Updates the tray icon and status display.
    fn update_status(&mut self, link_kind: u8, mic_active: bool, cam_active: bool);
}
