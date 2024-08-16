use std::sync::{Arc, Mutex};
use crate::error::WinshiftError;

pub trait FocusChangeHandler: Send + Sync {
    fn on_focus_change(&self, window_title: String);
}

pub struct WindowFocusHook {
    handler: Arc<Mutex<dyn FocusChangeHandler>>,
}

impl WindowFocusHook {
    pub fn new<H: FocusChangeHandler + 'static>(handler: H) -> Self {
        Self {
            handler: Arc::new(Mutex::new(handler)),
        }
    }

    pub fn run(&self) -> Result<(), WinshiftError> {
        #[cfg(target_os = "windows")]
        {
            crate::windows::run_hook(self.handler.clone())
        }

        #[cfg(target_os = "linux")]
        {
            crate::linux::run_hook(self.handler.clone())
        }

        #[cfg(target_os = "macos")]
        {
            crate::macos::run_hook(self.handler.clone())
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            Err(WinshiftError::PlatformError("Unsupported platform".to_string()))
        }
    }

    pub fn stop(&self) {
        #[cfg(target_os = "windows")]
        {
            crate::windows::stop_hook()
        }

        #[cfg(target_os = "linux")]
        {
            crate::linux::stop_hook()
        }

        #[cfg(target_os = "macos")]
        {
            crate::macos::stop_hook()
        }
    }
}