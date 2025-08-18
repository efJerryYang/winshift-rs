use std::sync::{Arc, RwLock};
use crate::FocusChangeHandler;
use crate::error::WinshiftError;

pub(crate) fn run_hook_with_config(
    _handler: Arc<RwLock<dyn FocusChangeHandler>>,
    _config: &crate::hook::WindowHookConfig,
) -> Result<(), WinshiftError> {
    Err(WinshiftError::UnsupportedPlatform {
        platform: "Windows".to_string(),
        message: "Windows support not yet implemented. Potential approach: SetWinEventHook with EVENT_SYSTEM_FOREGROUND and EVENT_OBJECT_NAMECHANGE".to_string(),
    })
}
