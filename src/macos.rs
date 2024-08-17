use std::sync::{Arc, RwLock};
use std::ffi::c_void;
use core_foundation::{
    base::*,
    runloop::*,
    string::*,
};
use cocoa::appkit::{NSWorkspace, NSWorkspaceNotification};
use cocoa::foundation::{NSString, NSArray};
use objc::{runtime::Object, *};
use crate::error::WinshiftError;
use crate::FocusChangeHandler;
use std::sync::atomic::{AtomicBool, Ordering};

static RUNNING: AtomicBool = AtomicBool::new(true);

pub(crate) fn run_hook(handler: Arc<RwLock<dyn FocusChangeHandler>>) -> Result<(), WinshiftError> {
    unsafe {
        let workspace = NSWorkspace::sharedWorkspace(nil);
        let notification_center: id = msg_send![workspace, notificationCenter];

        let observer = WindowObserver::new(handler);
        let observer_ptr = Box::into_raw(Box::new(observer));

        let active_app_notif = NSWorkspaceNotification::ActiveSpaceDidChange;
        let _: () = msg_send![
            notification_center,
            addObserver:observer_ptr
            selector:sel!(activeAppDidChange:)
            name:active_app_notif.into_cocoa_value()
            object:nil
        ];

        let active_window_notif = NSWorkspaceNotification::ActiveWindowDidChange;
        let _: () = msg_send![
            notification_center,
            addObserver:observer_ptr
            selector:sel!(activeWindowDidChange:)
            name:active_window_notif.into_cocoa_value()
            object:nil
        ];

        let run_loop = CFRunLoopGetCurrent();
        while RUNNING.load(Ordering::Relaxed) {
            autoreleasepool(|| {
                let _ = CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.1, false);
            });
        }

        // Clean up
        let _: () = msg_send![
            notification_center,
            removeObserver:observer_ptr
            name:active_app_notif.into_cocoa_value()
            object:nil
        ];
        let _: () = msg_send![
            notification_center,
            removeObserver:observer_ptr
            name:active_window_notif.into_cocoa_value()
            object:nil
        ];

        drop(Box::from_raw(observer_ptr));
    }

    Ok(())
}

struct WindowObserver {
    handler: Arc<RwLock<dyn FocusChangeHandler>>,
    last_title: String,
}

impl WindowObserver {
    fn new(handler: Arc<RwLock<dyn FocusChangeHandler>>) -> Self {
        WindowObserver {
            handler,
            last_title: String::new(),
        }
    }

    fn handle_change(&mut self) {
        if let Some(window_title) = get_active_window_title() {
            if window_title != self.last_title {
                self.last_title = window_title.clone();
                if let Ok(guard) = self.handler.read() {
                    guard.on_focus_change(window_title);
                }
            }
        }
    }
}

#[allow(non_snake_case)]
impl WindowObserver {
    extern "C" fn activeAppDidChange(_: &Object, _: Sel, _: id) {
        unsafe {
            let this: *mut WindowObserver = *(*_).get_ivar("rustObject");
            (*this).handle_change();
        }
    }

    extern "C" fn activeWindowDidChange(_: &Object, _: Sel, _: id) {
        unsafe {
            let this: *mut WindowObserver = *(*_).get_ivar("rustObject");
            (*this).handle_change();
        }
    }
}

fn get_active_window_title() -> Option<String> {
    unsafe {
        let workspace = NSWorkspace::sharedWorkspace(nil);
        let app: id = msg_send![workspace, frontmostApplication];
        let windows: id = msg_send![app, windows];
        let main_window: id = msg_send![windows, firstObject];
        if main_window != nil {
            let title: id = msg_send![main_window, title];
            let nsstring = NSString::alloc(nil).init_str(&CFString::new(title).to_string());
            Some(nsstring.to_string())
        } else {
            None
        }
    }
}


pub fn stop_hook() -> Result<(), WinshiftError> {
    if RUNNING.swap(false, Ordering::Relaxed) {
        Ok(())
    } else {
        Err(WinshiftError::StopError)
    }
}