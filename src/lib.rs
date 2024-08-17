mod error;
mod hook;
mod logger;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
mod macos;

pub use error::WinshiftError;
pub use hook::{FocusChangeHandler, WindowFocusHook};

pub fn init_logger() {
    logger::init();
}
