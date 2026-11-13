//! The Windows shell: the panel above the tray (SYSTEM_DESIGN sections 33 to 36) and the tray
//! owl (section 34). One window does both; it runs on the UI thread's message loop.

mod draw;
mod panel;
mod tray;

pub use panel::{Ui, Waker, open_running, quit_running, run};
