//! Owlmic for Linux: PipeWire virtual microphone and loopback, v4l2loopback virtual camera,
//! and StatusNotifierItem system tray coordinator.

pub use owlmic_platform::{SpeakerLoopback, TrayUi, VirtualCam, VirtualMic};

pub mod audio;
pub mod camera;
pub mod tray;

pub use audio::{PipeWireLoopback, PipeWireMic};
pub use camera::V4l2Cam;
pub use tray::LinuxTray;
