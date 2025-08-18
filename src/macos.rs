//! # Thread Safety Warning
//!
//! This implementation uses `static mut` variables which are not thread-safe.
//! It assumes single-threaded usage on the main thread only.
//!
//! TODO: Replace `static mut` with thread-safe alternatives (Mutex/RwLock)
//! TODO: Implement hook-specific stop methods instead of global stop

use std::sync::{Arc, RwLock};

#[link(name = "AppKit", kind = "framework")]
extern "C" {}
use crate::error::WinshiftError;
use crate::FocusChangeHandler;

use log::{debug, error, info, trace, warn};
use core_foundation::runloop::{kCFRunLoopDefaultMode, CFRunLoop};

// TODO: Make these thread-safe
static mut CURRENT_RUN_LOOP: Option<CFRunLoop> = None;

pub(crate) fn run_hook_with_config(
    handler: Arc<RwLock<dyn FocusChangeHandler>>,
    config: &crate::hook::WindowHookConfig,
) -> Result<(), WinshiftError> {
    trace!(
        "Starting macOS hook with monitoring mode: {:?}",
        config.monitoring_mode
    );
    run_accessibility_hook_with_mode(handler, config.monitoring_mode)
}

unsafe extern "C" fn window_focus_callback(
    observer: accessibility_sys::AXObserverRef,
    element: accessibility_sys::AXUIElementRef,
    _notification: core_foundation::string::CFStringRef,
    user_info: *mut std::ffi::c_void,
) {
    use accessibility_sys::{kAXTitleAttribute, AXUIElementCopyAttributeValue};
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::string::CFString;
    use std::ptr;

    trace!(
        "Window focus callback entry - observer: {:p}, element: {:p}, user_info: {:p}",
        observer,
        element,
        user_info
    );

    if user_info.is_null() {
        error!("FATAL: user_info is null in window_focus_callback!");
        return;
    }

    if element.is_null() {
        error!("FATAL: element is null in window_focus_callback!");
        return;
    }

    trace!("Pointers validated, dereferencing handler...");
    let handler = &*(user_info as *const Arc<RwLock<dyn FocusChangeHandler>>);
    trace!("Handler dereferenced successfully");

    let mut title_ptr: *mut std::ffi::c_void = ptr::null_mut();
    let title_attr = CFString::from_static_string(kAXTitleAttribute);

    let result = AXUIElementCopyAttributeValue(
        element,
        title_attr.as_concrete_TypeRef(),
        std::ptr::from_mut::<*mut std::ffi::c_void>(&mut title_ptr)
            .cast::<*const std::ffi::c_void>(),
    );

    if result == 0 && !title_ptr.is_null() {
        let cf_title = CFType::wrap_under_create_rule(title_ptr);

        if let Some(cf_string) = cf_title.downcast::<CFString>() {
            let window_title = cf_string.to_string();

            if !window_title.is_empty() {
                debug!("Window focus changed to: '{}'", window_title);

                trace!("Acquiring handler read lock...");
                if let Ok(guard) = handler.read() {
                    trace!("Handler lock acquired, calling on_window_change...");
                    guard.on_window_change(window_title);
                    trace!("on_window_change completed");
                } else {
                    error!("Failed to acquire handler read lock");
                }
            }
        } else {
            warn!("Failed to downcast CFType to CFString");
        }
    } else {
        warn!(
            "Failed to get window title, result: {}, ptr null: {}",
            result,
            title_ptr.is_null()
        );
    }

    trace!("Window focus callback exit");
}

struct ObserverInfo {
    run_loop_source: core_foundation::runloop::CFRunLoopSource,
}

fn run_accessibility_hook_with_mode(
    handler: Arc<RwLock<dyn FocusChangeHandler>>,
    mode: crate::hook::MonitoringMode,
) -> Result<(), WinshiftError> {
    use crate::hook::MonitoringMode;

    match mode {
        MonitoringMode::Combined => run_accessibility_hook(handler),
        MonitoringMode::AppOnly => run_app_only_hook(handler),
        MonitoringMode::WindowOnly => run_window_only_hook(handler),
    }
}

fn run_accessibility_hook(
    handler: Arc<RwLock<dyn FocusChangeHandler>>,
) -> Result<(), WinshiftError> {
    use accessibility_sys::{
        kAXFocusedWindowChangedNotification, AXIsProcessTrusted, AXObserverAddNotification,
        AXObserverCallback, AXObserverCreate, AXObserverGetRunLoopSource,
        AXUIElementCreateApplication,
    };
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;
    use objc::declare::ClassDecl;
    use objc::runtime::{Object, Sel};
    use objc::{class, msg_send, sel};
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::ptr;

    info!("Using Accessibility API for event-driven window monitoring");

    if !unsafe { AXIsProcessTrusted() } {
        return Err(WinshiftError::PlatformError(
            "Accessibility permissions required. Please enable accessibility access in System Preferences > Security & Privacy > Privacy > Accessibility".to_string(),
        ));
    }

    info!("Accessibility permissions verified");

    let handler_ptr = Box::into_raw(Box::new(handler.clone()));

    static mut OBSERVERS: Option<HashMap<i32, ObserverInfo>> = None;
    unsafe {
        OBSERVERS = Some(HashMap::new());
    }

    unsafe fn create_observer_for_app(
        pid: i32,
        handler: &Arc<RwLock<dyn FocusChangeHandler>>,
    ) -> Result<ObserverInfo, WinshiftError> {
        trace!("Creating observer for PID: {}", pid);
        let mut observer = ptr::null_mut();
        let callback: AXObserverCallback = window_focus_callback;

        let result = AXObserverCreate(pid, callback, &mut observer);
        trace!("AXObserverCreate result for PID {}: {}", pid, result);
        if result != 0 {
            return Err(WinshiftError::PlatformError(format!(
                "Failed to create AX observer for PID {pid}: {result}"
            )));
        }

        let app_element = AXUIElementCreateApplication(pid);
        trace!("Created AXUIElement for PID: {}", pid);

        let window_notification = CFString::from_static_string(kAXFocusedWindowChangedNotification);

        let handler_ptr = Box::into_raw(Box::new(handler.clone()));

        let result = AXObserverAddNotification(
            observer,
            app_element,
            window_notification.as_concrete_TypeRef(),
            handler_ptr.cast::<c_void>(),
        );

        trace!(
            "AXObserverAddNotification result for PID {}: {}",
            pid,
            result
        );
        if result != 0 {
            use accessibility_sys::error_string;
            warn!(
                "Failed to add window focus notification for PID {}: {} ({})",
                pid,
                result,
                error_string(result)
            );
            let _ = Box::from_raw(handler_ptr);
            return Err(WinshiftError::PlatformError(format!(
                "Failed to add notification for PID {}: {} ({})",
                pid,
                result,
                error_string(result)
            )));
        }

        let run_loop_source = AXObserverGetRunLoopSource(observer);
        let run_loop = CFRunLoop::get_current();
        use core_foundation::runloop::CFRunLoopSource;
        let cf_source = CFRunLoopSource::wrap_under_get_rule(run_loop_source);
        run_loop.add_source(&cf_source, kCFRunLoopDefaultMode);

        info!(
            "Successfully created accessibility observer for application PID: {}",
            pid
        );
        Ok(ObserverInfo {
            run_loop_source: cf_source,
        })
    }

    static mut GLOBAL_HANDLER: Option<Arc<RwLock<dyn FocusChangeHandler>>> = None;

    unsafe fn setup_nsworkspace_notifications(
        handler_ptr: *mut Arc<RwLock<dyn FocusChangeHandler>>,
    ) -> Result<(), WinshiftError> {
        trace!("Setting up NSWorkspace notifications");

        GLOBAL_HANDLER = Some((*handler_ptr).clone());

        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("WindowMonitorObserver", superclass).ok_or_else(|| {
            WinshiftError::PlatformError("Failed to create observer class".to_string())
        })?;

        extern "C" fn application_did_activate(
            this: &Object,
            _cmd: Sel,
            notification: *mut Object,
        ) {
            unsafe {
                trace!(
                    "NSWorkspace callback entry - this: {:p}, notification: {:p}",
                    this,
                    notification
                );

                if notification.is_null() {
                    error!("FATAL: notification is null!");
                    return;
                }

                trace!("Getting global handler...");
                if let Some(handler) = (&raw const GLOBAL_HANDLER).as_ref().unwrap() {
                    trace!("Global handler found, extracting application info...");

                    trace!("Getting userInfo from notification...");
                    let user_info: *mut Object = msg_send![notification, userInfo];
                    trace!("userInfo result: {:p}", user_info);

                    if !user_info.is_null() {
                        trace!("Creating app key...");
                        let app_key = CFString::from_static_string("NSWorkspaceApplicationKey");
                        trace!("Getting app object from userInfo...");
                        let app: *mut Object =
                            msg_send![user_info, objectForKey: app_key.as_concrete_TypeRef()];
                        trace!("App object result: {:p}", app);

                        if !app.is_null() {
                            trace!("Getting process identifier...");
                            let pid: i32 = msg_send![app, processIdentifier];
                            let app_name = get_app_name_by_pid(pid)
                                .unwrap_or_else(|| format!("Unknown (PID: {pid})"));
                            debug!("Application switched to PID {} ({})", pid, app_name);

                            trace!("Checking if observer already exists for PID {}...", pid);
                            if let Some(observers) = (&raw mut OBSERVERS).as_mut().unwrap() {
                                if !observers.contains_key(&pid) {
                                    trace!(
                                        "No existing observer, creating new one for PID {}",
                                        pid
                                    );

                                    trace!(
                                        "Calling create_observer_for_app with handler reference..."
                                    );

                                    match create_observer_for_app(pid, handler) {
                                        Ok(observer_info) => {
                                            trace!("Observer created successfully, cleaning up old observers...");
                                            let run_loop = CFRunLoop::get_current();
                                            for (old_pid, old_observer_info) in observers.drain() {
                                                trace!(
                                                    "Removing CFRunLoop source for old PID: {}",
                                                    old_pid
                                                );
                                                run_loop.remove_source(
                                                    &old_observer_info.run_loop_source,
                                                    kCFRunLoopDefaultMode,
                                                );
                                                trace!(
                                                    "Cleaned up observer for old PID: {}",
                                                    old_pid
                                                );
                                            }

                                            trace!("Inserting new observer for PID {}...", pid);
                                            observers.insert(pid, observer_info);
                                            info!("Created AX observer for app PID: {}", pid);

                                            trace!("Notifying app change...");
                                            if let Ok(guard) = handler.read() {
                                                trace!("Handler lock acquired, calling on_app_change...");
                                                guard.on_app_change(pid, app_name.clone());
                                                trace!("on_app_change completed");
                                            }

                                            trace!("Getting current window title...");
                                            if let Some(title) = get_current_window_title() {
                                                info!(
                                                    "Current window in activated app: '{}'",
                                                    title
                                                );
                                                trace!(
                                                    "Acquiring handler lock for initial window..."
                                                );
                                                if let Ok(guard) = handler.read() {
                                                    trace!("Handler lock acquired, calling on_window_change...");
                                                    guard.on_window_change(title);
                                                    trace!("on_window_change completed");
                                                }
                                            } else {
                                                trace!("No current window title available");
                                            }
                                        }
                                        Err(e) => {
                                            warn!("Failed to create AX observer for activated app PID {}: {}", pid, e);
                                        }
                                    }

                                    trace!("Observer creation completed successfully");
                                } else {
                                    trace!(
                                        "Observer already exists for PID {}, skipping creation",
                                        pid
                                    );
                                }
                            } else {
                                error!("OBSERVERS is None!");
                            }
                        } else {
                            warn!("App object is null from userInfo");
                        }
                    } else {
                        warn!("userInfo is null from notification");
                    }
                } else {
                    error!("GLOBAL_HANDLER is None!");
                }

                trace!("NSWorkspace callback exit");
            }
        }

        decl.add_method(
            sel!(applicationDidActivate:),
            application_did_activate as extern "C" fn(&Object, Sel, *mut Object),
        );

        let observer_class = decl.register();
        trace!("Created WindowMonitorObserver class");

        let observer_instance: *mut Object = msg_send![observer_class, alloc];
        let observer_instance: *mut Object = msg_send![observer_instance, init];

        let workspace_class = class!(NSWorkspace);
        let workspace: *mut Object = msg_send![workspace_class, sharedWorkspace];
        let notification_center: *mut Object = msg_send![workspace, notificationCenter];

        let notification_name =
            CFString::from_static_string("NSWorkspaceDidActivateApplicationNotification");
        let _: () = msg_send![
            notification_center,
            addObserver: observer_instance
            selector: sel!(applicationDidActivate:)
            name: notification_name.as_concrete_TypeRef()
            object: ptr::null_mut::<Object>()
        ];

        info!("Successfully registered for NSWorkspaceDidActivateApplicationNotification");
        Ok(())
    }

    unsafe {
        let workspace_class = class!(NSWorkspace);
        let workspace: *mut Object = msg_send![workspace_class, sharedWorkspace];
        let frontmost_app: *mut Object = msg_send![workspace, frontmostApplication];

        if !frontmost_app.is_null() {
            let initial_pid: i32 = msg_send![frontmost_app, processIdentifier];
            trace!("Initial frontmost app PID: {}", initial_pid);

            if initial_pid > 0 {
                match create_observer_for_app(initial_pid, &handler) {
                    Ok(observer_info) => {
                        if let Some(observers) = (&raw mut OBSERVERS).as_mut().unwrap() {
                            observers.insert(initial_pid, observer_info);
                            info!(
                                "Created initial observer for current app PID: {}",
                                initial_pid
                            );

                            let app_name = get_app_name_by_pid(initial_pid)
                                .unwrap_or_else(|| format!("Unknown (PID: {initial_pid})"));
                            info!("Initial app: {} (PID: {})", app_name, initial_pid);
                            if let Ok(guard) = handler.read() {
                                guard.on_app_change(initial_pid, app_name);
                            }

                            if let Some(title) = get_current_window_title() {
                                info!("Initial window: '{}'", title);
                                if let Ok(guard) = handler.read() {
                                    guard.on_window_change(title);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to create initial observer: {}", e);
                    }
                }
            }
        }

        info!("Accessibility observer started - event-driven window monitoring active");

        setup_nsworkspace_notifications(handler_ptr)?;

        info!("Event-driven NSWorkspace monitoring active");
        run_cfrunloop();

        if let Some(observers) = (&raw mut OBSERVERS).as_mut().unwrap() {
            let run_loop = CFRunLoop::get_current();
            for (pid, observer_info) in observers.drain() {
                trace!("Cleaning up observer for PID: {}", pid);
                run_loop.remove_source(&observer_info.run_loop_source, kCFRunLoopDefaultMode);
                trace!("Removed CFRunLoop source for PID: {}", pid);
            }
        }

        CURRENT_RUN_LOOP = None;
        let _ = Box::from_raw(handler_ptr);
        trace!("Accessibility hook stopped");
    }

    Ok(())
}

fn run_app_only_hook(handler: Arc<RwLock<dyn FocusChangeHandler>>) -> Result<(), WinshiftError> {
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;
    use objc::declare::ClassDecl;
    use objc::runtime::{Object, Sel};
    use objc::{class, msg_send, sel};
    use std::ptr;

    info!("Using NSWorkspace for app-only monitoring (no window observers)");

    if !unsafe { accessibility_sys::AXIsProcessTrusted() } {
        return Err(WinshiftError::PlatformError(
            "Accessibility permissions required. Please enable accessibility access in System Preferences > Security & Privacy > Privacy > Accessibility".to_string(),
        ));
    }

    info!("Accessibility permissions verified");

    static mut GLOBAL_HANDLER: Option<Arc<RwLock<dyn FocusChangeHandler>>> = None;
    unsafe fn setup_nsworkspace_notifications_only(
        handler_ptr: *mut Arc<RwLock<dyn FocusChangeHandler>>,
    ) -> Result<(), WinshiftError> {
        trace!("Setting up NSWorkspace notifications (app-only mode)");

        GLOBAL_HANDLER = Some((*handler_ptr).clone());

        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("AppOnlyObserver", superclass).ok_or_else(|| {
            WinshiftError::PlatformError("Failed to create observer class".to_string())
        })?;

        extern "C" fn application_did_activate(
            this: &Object,
            _cmd: Sel,
            notification: *mut Object,
        ) {
            unsafe {
                trace!(
                    "NSWorkspace callback entry - this: {:p}, notification: {:p}",
                    this,
                    notification
                );

                if notification.is_null() {
                    error!("FATAL: notification is null!");
                    return;
                }

                if let Some(handler) = (&raw const GLOBAL_HANDLER).as_ref().unwrap() {
                    let user_info: *mut Object = msg_send![notification, userInfo];

                    if !user_info.is_null() {
                        let app_key = CFString::from_static_string("NSWorkspaceApplicationKey");
                        let app: *mut Object =
                            msg_send![user_info, objectForKey: app_key.as_concrete_TypeRef()];

                        if !app.is_null() {
                            let pid: i32 = msg_send![app, processIdentifier];
                            let app_name = get_app_name_by_pid(pid)
                                .unwrap_or_else(|| format!("Unknown (PID: {pid})"));
                            debug!("Application switched to PID {} ({})", pid, app_name);

                            if let Ok(guard) = handler.read() {
                                guard.on_app_change(pid, app_name);
                            }
                        }
                    }
                }
            }
        }

        decl.add_method(
            sel!(applicationDidActivate:),
            application_did_activate as extern "C" fn(&Object, Sel, *mut Object),
        );

        let observer_class = decl.register();
        let observer_instance: *mut Object = msg_send![observer_class, alloc];
        let observer_instance: *mut Object = msg_send![observer_instance, init];

        let workspace_class = class!(NSWorkspace);
        let workspace: *mut Object = msg_send![workspace_class, sharedWorkspace];
        let notification_center: *mut Object = msg_send![workspace, notificationCenter];

        let notification_name =
            CFString::from_static_string("NSWorkspaceDidActivateApplicationNotification");
        let _: () = msg_send![
            notification_center,
            addObserver: observer_instance
            selector: sel!(applicationDidActivate:)
            name: notification_name.as_concrete_TypeRef()
            object: ptr::null_mut::<Object>()
        ];

        info!("App-only monitoring active - no window observers created");
        Ok(())
    }

    let handler_ptr = Box::into_raw(Box::new(handler.clone()));

    unsafe {
        // Get initial app state
        let workspace_class = class!(NSWorkspace);
        let workspace: *mut Object = msg_send![workspace_class, sharedWorkspace];
        let frontmost_app: *mut Object = msg_send![workspace, frontmostApplication];

        if !frontmost_app.is_null() {
            let initial_pid: i32 = msg_send![frontmost_app, processIdentifier];
            let app_name = get_app_name_by_pid(initial_pid)
                .unwrap_or_else(|| format!("Unknown (PID: {initial_pid})"));
            info!("Initial app: {} (PID: {})", app_name, initial_pid);
            if let Ok(guard) = handler.read() {
                guard.on_app_change(initial_pid, app_name);
            }
        }

        setup_nsworkspace_notifications_only(handler_ptr)?;
        run_cfrunloop();
        let _ = Box::from_raw(handler_ptr);
        trace!("App-only hook stopped");
    }

    Ok(())
}

fn run_window_only_hook(handler: Arc<RwLock<dyn FocusChangeHandler>>) -> Result<(), WinshiftError> {
    use accessibility_sys::{
        kAXFocusedWindowChangedNotification, AXIsProcessTrusted, AXObserverAddNotification,
        AXObserverCallback, AXObserverCreate, AXObserverGetRunLoopSource,
        AXUIElementCreateApplication,
    };
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;
    use std::ffi::c_void;
    use std::ptr;

    info!("Using Accessibility API for window-only monitoring (no app notifications)");

    if !unsafe { AXIsProcessTrusted() } {
        return Err(WinshiftError::PlatformError(
            "Accessibility permissions required. Please enable accessibility access in System Preferences > Security & Privacy > Privacy > Accessibility".to_string(),
        ));
    }

    info!("Accessibility permissions verified");

    let handler_ptr = Box::into_raw(Box::new(handler.clone()));

    unsafe fn create_observer_for_current_app(
        handler: &Arc<RwLock<dyn FocusChangeHandler>>,
    ) -> Result<ObserverInfo, WinshiftError> {
        use objc::{class, msg_send};

        let workspace_class = class!(NSWorkspace);
        let workspace: *mut objc::runtime::Object = msg_send![workspace_class, sharedWorkspace];
        let frontmost_app: *mut objc::runtime::Object = msg_send![workspace, frontmostApplication];

        if frontmost_app.is_null() {
            return Err(WinshiftError::PlatformError(
                "No frontmost application found".to_string(),
            ));
        }

        let pid: i32 = msg_send![frontmost_app, processIdentifier];
        trace!("Creating window observer for current app PID: {}", pid);

        let mut observer = ptr::null_mut();
        let callback: AXObserverCallback = window_focus_callback;

        let result = AXObserverCreate(pid, callback, &mut observer);
        if result != 0 {
            return Err(WinshiftError::PlatformError(format!(
                "Failed to create AX observer: {result}"
            )));
        }

        let app_element = AXUIElementCreateApplication(pid);
        let window_notification = CFString::from_static_string(kAXFocusedWindowChangedNotification);
        let handler_ptr = Box::into_raw(Box::new(handler.clone()));

        let result = AXObserverAddNotification(
            observer,
            app_element,
            window_notification.as_concrete_TypeRef(),
            handler_ptr.cast::<c_void>(),
        );

        if result != 0 {
            let _ = Box::from_raw(handler_ptr);
            return Err(WinshiftError::PlatformError(format!(
                "Failed to add notification: {result}"
            )));
        }

        let run_loop_source = AXObserverGetRunLoopSource(observer);
        let run_loop = CFRunLoop::get_current();
        use core_foundation::runloop::CFRunLoopSource;
        let cf_source = CFRunLoopSource::wrap_under_get_rule(run_loop_source);
        run_loop.add_source(&cf_source, kCFRunLoopDefaultMode);

        info!("Window-only monitoring active for current app PID: {}", pid);
        Ok(ObserverInfo {
            run_loop_source: cf_source,
        })
    }

    unsafe {
        let observer_info = create_observer_for_current_app(&handler)?;

        if let Some(title) = get_current_window_title() {
            info!("Initial window: '{}'", title);
            if let Ok(guard) = handler.read() {
                guard.on_window_change(title);
            }
        }

        run_cfrunloop();

        let run_loop = CFRunLoop::get_current();
        run_loop.remove_source(&observer_info.run_loop_source, kCFRunLoopDefaultMode);
        let _ = Box::from_raw(handler_ptr);
        trace!("Window-only hook stopped");
    }

    Ok(())
}

fn run_cfrunloop() {
    info!("Getting current CFRunLoop");

    let run_loop = CFRunLoop::get_current();
    unsafe {
        CURRENT_RUN_LOOP = Some(run_loop.clone());
    }

    info!("CFRunLoop starting");
    CFRunLoop::run_current();
    info!("CFRunLoop stopped");

    unsafe {
        CURRENT_RUN_LOOP = None;
    }
}

fn get_app_name_by_pid(pid: i32) -> Option<String> {
    use objc::{class, msg_send};

    unsafe {
        let workspace_class = class!(NSWorkspace);
        let workspace: *mut objc::runtime::Object = msg_send![workspace_class, sharedWorkspace];

        if workspace.is_null() {
            return None;
        }

        let running_apps: *mut objc::runtime::Object = msg_send![workspace, runningApplications];

        if running_apps.is_null() {
            return None;
        }

        let count: usize = msg_send![running_apps, count];
        for i in 0..count {
            let app: *mut objc::runtime::Object = msg_send![running_apps, objectAtIndex: i];
            if !app.is_null() {
                let app_pid: i32 = msg_send![app, processIdentifier];
                if app_pid == pid {
                    let localized_name: *mut objc::runtime::Object = msg_send![app, localizedName];
                    if !localized_name.is_null() {
                        let name_str: *const std::ffi::c_char =
                            msg_send![localized_name, UTF8String];
                        if !name_str.is_null() {
                            if let Ok(name) = std::ffi::CStr::from_ptr(name_str).to_str() {
                                return Some(name.to_string());
                            }
                        }
                    }
                    break;
                }
            }
        }
    }
    None
}

fn get_current_window_title() -> Option<String> {
    use accessibility_sys::{
        kAXFocusedApplicationAttribute, kAXFocusedWindowAttribute, kAXTitleAttribute,
        AXUIElementCopyAttributeValue, AXUIElementCreateSystemWide,
    };
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::string::CFString;
    use std::ptr;

    unsafe {
        let system_element = AXUIElementCreateSystemWide();

        let mut focused_app: *mut std::ffi::c_void = ptr::null_mut();
        let focused_app_attr = CFString::from_static_string(kAXFocusedApplicationAttribute);
        let result = AXUIElementCopyAttributeValue(
            system_element,
            focused_app_attr.as_concrete_TypeRef(),
            std::ptr::from_mut::<*mut std::ffi::c_void>(&mut focused_app)
                .cast::<*const std::ffi::c_void>(),
        );

        if result != 0 || focused_app.is_null() {
            return None;
        }

        let mut focused_window: *mut std::ffi::c_void = ptr::null_mut();
        let focused_window_attr = CFString::from_static_string(kAXFocusedWindowAttribute);
        let result = AXUIElementCopyAttributeValue(
            focused_app as accessibility_sys::AXUIElementRef,
            focused_window_attr.as_concrete_TypeRef(),
            std::ptr::from_mut::<*mut std::ffi::c_void>(&mut focused_window)
                .cast::<*const std::ffi::c_void>(),
        );

        if result != 0 || focused_window.is_null() {
            return None;
        }

        let mut title_ref: *mut std::ffi::c_void = ptr::null_mut();
        let title_attr = CFString::from_static_string(kAXTitleAttribute);
        let result = AXUIElementCopyAttributeValue(
            focused_window as accessibility_sys::AXUIElementRef,
            title_attr.as_concrete_TypeRef(),
            std::ptr::from_mut::<*mut std::ffi::c_void>(&mut title_ref)
                .cast::<*const std::ffi::c_void>(),
        );

        if result != 0 || title_ref.is_null() {
            return None;
        }

        let cf_title = CFType::wrap_under_create_rule(title_ref);
        cf_title
            .downcast::<CFString>()
            .map(|cf_string| cf_string.to_string())
    }
}

pub fn stop_hook() -> Result<(), WinshiftError> {
    info!("=== STOP_HOOK CALLED ====");
    trace!("Stopping macOS hook");

    unsafe {
        if let Some(ref run_loop) = CURRENT_RUN_LOOP {
            info!("Found CFRunLoop, calling stop()...");
            run_loop.stop();
            info!("CFRunLoop::stop() called successfully");
        } else {
            warn!("No current CFRunLoop stored termination");
        }
    }

    info!("=== STOP_HOOK COMPLETED ====");
    Ok(())
}
