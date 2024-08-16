use std::sync::{Arc, Mutex};
use windows::{
    Win32::UI::WindowsAndMessaging::*,
    Win32::Foundation::*,
    Win32::System::Threading::GetCurrentThreadId,
};
use crate::error::WinshiftError;
use crate::FocusChangeHandler;
use std::sync::atomic::{AtomicBool, Ordering};

static RUNNING: AtomicBool = AtomicBool::new(true);

pub(crate) fn run_hook(handler: Arc<Mutex<dyn FocusChangeHandler>>) -> Result<(), WinshiftError> {
    unsafe {
        let hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_OBJECT_NAMECHANGE,
            HINSTANCE::default(),
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );

        if hook.is_invalid() {
            return Err(WinshiftError::HookError);
        }

        GLOBAL_HANDLER = Some(handler);

        let mut msg = MSG::default();
        while RUNNING.load(Ordering::Relaxed) {
            let result = GetMessageA(&mut msg, HWND::default(), 0, 0);
            if result.0 <= 0 {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageA(&msg);
        }

        UnhookWinEvent(hook);
        Ok(())
    }
}

static mut GLOBAL_HANDLER: Option<Arc<Mutex<dyn FocusChangeHandler>>> = None;
static mut LAST_TITLE: Option<String> = None;

unsafe extern "system" fn win_event_proc(
    _h_win_event_hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _id_event_thread: u32,
    _dwms_event_time: u32,
) {
    if event == EVENT_SYSTEM_FOREGROUND as u32 || event == EVENT_OBJECT_NAMECHANGE as u32 {
        let mut title = [0u16; 512];
        GetWindowTextW(hwnd, &mut title);
        let title = String::from_utf16_lossy(&title).trim_end_matches('\0').to_string();
        
        if let Some(last_title) = LAST_TITLE.as_ref() {
            if *last_title != title {
                if let Some(handler) = &GLOBAL_HANDLER {
                    if let Ok(guard) = handler.lock() {
                        guard.on_focus_change(title.clone());
                    }
                }
                LAST_TITLE = Some(title);
            }
        } else {
            LAST_TITLE = Some(title);
        }
    }
}

pub fn stop_hook() {
    RUNNING.store(false, Ordering::Relaxed);
    unsafe {
        PostThreadMessageA(GetCurrentThreadId(), WM_QUIT, WPARAM(0), LPARAM(0));
    }
}