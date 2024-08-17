use crate::error::WinshiftError;
use crate::{log_debug, log_error, log_trace};
use std::sync::{Arc, RwLock};

pub trait FocusChangeHandler: Send + Sync {
    fn on_focus_change(&self, window_title: String);
}

pub struct WindowFocusHook {
    handler: Arc<RwLock<dyn FocusChangeHandler>>,
}

impl WindowFocusHook {
    pub fn new<H: FocusChangeHandler + 'static>(handler: H) -> Self {
        log_debug!("Creating new WindowFocusHook");
        Self {
            handler: Arc::new(RwLock::new(handler)),
        }
    }

    pub fn run(&self) -> Result<(), WinshiftError> {
        log_debug!("Running WindowFocusHook");
        #[cfg(target_os = "windows")]
        {
            log_trace!("Running on Windows platform");
            crate::windows::run_hook(self.handler.clone())
        }

        #[cfg(target_os = "linux")]
        {
            log_trace!("Running on Linux platform");
            crate::linux::run_hook(self.handler.clone())
        }

        #[cfg(target_os = "macos")]
        {
            log_trace!("Running on macOS platform");
            crate::macos::run_hook(self.handler.clone())
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            log_error!("Unsupported platform");
            Err(WinshiftError::PlatformError(
                "Unsupported platform".to_string(),
            ))
        }
    }

    pub fn stop(&self) -> Result<(), WinshiftError> {
        log_debug!("Stopping WindowFocusHook");
        #[cfg(target_os = "windows")]
        {
            log_trace!("Stopping on Windows platform");
            crate::windows::stop_hook()
        }

        #[cfg(target_os = "linux")]
        {
            log_trace!("Stopping on Linux platform");
            crate::linux::stop_hook()
        }

        #[cfg(target_os = "macos")]
        {
            log_trace!("Stopping on macOS platform");
            crate::macos::stop_hook()
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            log_error!("Unsupported platform");
            Err(WinshiftError::PlatformError(
                "Unsupported platform".to_string(),
            ))
        }
    }
}
