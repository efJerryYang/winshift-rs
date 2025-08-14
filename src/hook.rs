use crate::error::WinshiftError;
use std::sync::{Arc, RwLock};

use log::{debug, trace};

/// Monitoring mode for selective event tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitoringMode {
    /// Monitor both application switches and window changes
    Comprehensive,
    /// Monitor only application switches, ignore window changes
    AppOnly,
    /// Monitor only window changes, ignore application switches
    WindowOnly,
}

impl Default for MonitoringMode {
    fn default() -> Self {
        Self::Comprehensive
    }
}

/// Trait for handling window focus change events
pub trait FocusChangeHandler: Send + Sync {
    /// Called when the active application changes
    ///
    /// # Arguments
    /// * `pid` - Process ID of the newly active application
    /// * `app_name` - Name of the application
    fn on_app_change(&self, pid: i32, app_name: String);

    /// Called when the focused window changes within the same application
    ///
    /// # Arguments
    /// * `window_title` - The title of the newly focused window
    fn on_window_change(&self, window_title: String);
}

/// Configuration for window monitoring behavior
#[derive(Debug, Clone, Default)]
pub struct WindowHookConfig {
    /// Monitoring mode to control which events are tracked
    /// Default: Comprehensive (both apps and windows)
    pub monitoring_mode: MonitoringMode,
}

/// Main window focus monitoring hook
///
/// ## Platform Implementation Notes:
/// - **macOS**: Event-driven using NSWorkspace notifications and Accessibility API observers
/// - **Linux**: Mixed approach depending on X11/Wayland availability  
/// - **Windows**: Event-driven using `SetWinEventHook` (currently not implemented)
///
/// ## Monitoring Modes:
/// - **Comprehensive**: Monitors both application switches and window changes (default)
/// - **`AppOnly`**: Monitors only application switches for optimal performance
/// - **`WindowOnly`**: Monitors only window changes within the current application
pub struct WindowFocusHook {
    handler: Arc<RwLock<dyn FocusChangeHandler>>,
    config: WindowHookConfig,
}

impl WindowFocusHook {
    /// Create a new window focus hook with default configuration
    pub fn new<H: FocusChangeHandler + 'static>(handler: H) -> Self {
        debug!("Creating new WindowFocusHook");

        Self {
            handler: Arc::new(RwLock::new(handler)),
            config: WindowHookConfig::default(),
        }
    }

    /// Create a new window focus hook with custom configuration
    pub fn with_config<H: FocusChangeHandler + 'static>(
        handler: H,
        config: WindowHookConfig,
    ) -> Self {
        debug!(
            "Creating new WindowFocusHook with custom config: {:?}",
            config
        );

        Self {
            handler: Arc::new(RwLock::new(handler)),
            config,
        }
    }

    /// Create a new window focus hook for app-only monitoring
    pub fn app_only<H: FocusChangeHandler + 'static>(handler: H) -> Self {
        let config = WindowHookConfig {
            monitoring_mode: MonitoringMode::AppOnly,
        };
        Self::with_config(handler, config)
    }

    /// Create a new window focus hook for window-only monitoring
    pub fn window_only<H: FocusChangeHandler + 'static>(handler: H) -> Self {
        let config = WindowHookConfig {
            monitoring_mode: MonitoringMode::WindowOnly,
        };
        Self::with_config(handler, config)
    }

    /// Runs the window focus monitoring hook
    /// This will block until `stop()` is called or an error occurs
    ///
    /// # Errors
    /// Returns `WinshiftError` if platform initialization fails
    pub fn run(&self) -> Result<(), WinshiftError> {
        debug!("Running WindowFocusHook");

        #[cfg(target_os = "windows")]
        {
            trace!("Running on Windows platform");
            crate::windows::run_hook_with_config(self.handler.clone(), &self.config)
        }

        #[cfg(target_os = "linux")]
        {
            trace!("Running on Linux platform");
            crate::linux::run_hook_with_config(self.handler.clone(), &self.config)
        }

        #[cfg(target_os = "macos")]
        {
            trace!("Running on macOS platform");
            crate::macos::run_hook_with_config(self.handler.clone(), &self.config)
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            error!("Unsupported platform");
            Err(WinshiftError::PlatformError(
                "Unsupported platform".to_string(),
            ))
        }
    }

    /// Stops the window focus monitoring hook
    ///
    /// # Errors
    /// Returns `WinshiftError` if platform stop operation fails
    pub fn stop() -> Result<(), WinshiftError> {
        debug!("Stopping WindowFocusHook");
        #[cfg(target_os = "windows")]
        {
            trace!("Stopping on Windows platform");
            crate::windows::stop_hook()
        }

        #[cfg(target_os = "linux")]
        {
            trace!("Stopping on Linux platform");
            crate::linux::stop_hook()
        }

        #[cfg(target_os = "macos")]
        {
            trace!("Stopping on macOS platform");
            crate::macos::stop_hook()
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            error!("Unsupported platform");
            Err(WinshiftError::PlatformError(
                "Unsupported platform".to_string(),
            ))
        }
    }
}
