//! owlmic.exe: the App Hub, startup, and the command-line jobs the installer runs.

pub mod app;
pub mod sinks;

#[cfg(windows)]
pub mod autostart;
#[cfg(windows)]
pub mod camera;
#[cfg(windows)]
pub mod instance;
#[cfg(windows)]
pub mod start;
#[cfg(windows)]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;
