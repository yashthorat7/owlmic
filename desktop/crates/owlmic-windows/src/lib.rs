//! Owlmic for Windows: WASAPI virtual microphone and loopback, Media Foundation
//! virtual camera, and Direct2D system tray flyout.

pub use owlmic_platform::{SpeakerLoopback, TrayUi, VirtualCam, VirtualMic};

pub mod audio;
pub mod camera;
pub mod tray;

pub use audio::{WasapiLoopback, WasapiMic};
pub use camera::WindowsCam;
pub use tray::WindowsTray;
