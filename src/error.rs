use thiserror::Error;

#[derive(Error, Debug)]
pub enum WinshiftError {
    #[error("Failed to initialize hook")]
    Initialization,

    #[error("Failed to set event hook")]
    Hook,

    #[error("Failed to stop hook")]
    Stop,

    #[error("Platform-specific error: {0}")]
    Platform(String),

    #[cfg(target_os = "windows")]
    #[error("Windows API error: {0}")]
    Windows(#[from] windows::core::Error),

    #[cfg(target_os = "linux")]
    #[error("X11 error: {0}")]
    X11(String),

    #[cfg(target_os = "macos")]
    #[error("macOS error: {0}")]
    MacOS(String),
}
