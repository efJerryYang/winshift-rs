use thiserror::Error;

#[derive(Error, Debug)]
pub enum WinshiftError {
    #[error("Failed to initialize hook")]
    InitializationError,

    #[error("Failed to set event hook")]
    HookError,

    #[error("Failed to stop hook")]
    StopError,

    #[error("Platform-specific error: {0}")]
    PlatformError(String),

    #[cfg(target_os = "windows")]
    #[error("Windows API error: {0}")]
    WindowsError(#[from] windows::core::Error),

    #[cfg(target_os = "linux")]
    #[error("X11 error: {0}")]
    X11Error(String),

    #[cfg(target_os = "macos")]
    #[error("macOS error: {0}")]
    MacOSError(String),
}