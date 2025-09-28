//! # winshift
//!
//! A cross-platform library for monitoring window focus changes.
//!
//! ## Features
//!
//! - Native window focus tracking on Linux via X11
//! - macOS support via Accessibility API (requires app tracking workaround)
//! - Windows support planned (not yet implemented)
//! - Event-driven callback system
//! - Minimal overhead window monitoring
//! - Thread-safe design
//!
//! Note: The app tracking functionality on macOS exists solely to work around
//! platform limitations for reliable window focus detection.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use winshift::{FocusChangeHandler, WindowFocusHook};
//!
//! struct MyHandler;
//!
//! impl FocusChangeHandler for MyHandler {
//!     fn on_app_change(&self, pid: i32, app_name: String) {
//!         println!("App changed: {} (PID: {})", app_name, pid);
//!     }
//!
//!     fn on_window_change(&self, window_title: String) {
//!         println!("Window changed to: {}", window_title);
//!     }
//! }
//!
//! fn main() -> Result<(), winshift::WinshiftError> {
//!     let handler = MyHandler;
//!     let hook = WindowFocusHook::new(handler);
//!     hook.run()
//! }
//! ```

mod error;
mod hook;

// Platform-specific implementations
// TODO: Windows implementation (planned to use SetWinEventHook)

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
mod macos;

pub use error::WinshiftError;
pub use hook::{FocusChangeHandler, MonitoringMode, WindowFocusHook, WindowHookConfig};

// Re-export standard log macros for convenience
pub use log::{debug, error, info, trace, warn};

// Re-export env_logger for examples and users
pub use env_logger;

#[cfg(target_os = "macos")]
#[deprecated(note = "Use WindowFocusHook::stop on the hook instance instead")]
pub use macos::stop_hook;
#[cfg(target_os = "macos")]
pub use macos::{
    get_active_window_info, match_active_window, ActiveWindowInfo, MatchOptions, WindowBounds,
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
