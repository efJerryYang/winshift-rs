use crate::error::WinshiftError;
use crate::FocusChangeHandler;
use libc::{c_char, c_int, c_uchar, c_ulong, c_void};
use std::ffi::CStr;
use std::os::unix::io::RawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use libc::{fd_set, timeval, FD_ZERO, FD_SET, select};
use std::sync::{Arc, Mutex};
use x11::xlib;

static RUNNING: AtomicBool = AtomicBool::new(true);

pub(crate) fn run_hook(handler: Arc<Mutex<dyn FocusChangeHandler>>) -> Result<(), WinshiftError> {
    unsafe {
        let display = xlib::XOpenDisplay(std::ptr::null());
        if display.is_null() {
            return Err(WinshiftError::InitializationError);
        }

        let root = xlib::XDefaultRootWindow(display);
        xlib::XSelectInput(
            display,
            root,
            xlib::PropertyChangeMask | xlib::SubstructureNotifyMask,
        );

        let active_window_atom =
            xlib::XInternAtom(display, "_NET_ACTIVE_WINDOW\0".as_ptr() as *const i8, 0);
        let wm_name_atom = xlib::XInternAtom(display, "WM_NAME\0".as_ptr() as *const i8, 0);
        let net_wm_name_atom =
            xlib::XInternAtom(display, "_NET_WM_NAME\0".as_ptr() as *const i8, 0);

        let mut active_window: xlib::Window = 0;
        let mut last_title = String::new();

        // Set up error handler
        let old_handler = xlib::XSetErrorHandler(Some(x_error_handler));

        let x11_fd = xlib::XConnectionNumber(display) as RawFd;

        while RUNNING.load(Ordering::Relaxed) {
            let mut in_fds: fd_set = std::mem::zeroed();
            FD_ZERO(&mut in_fds);
            FD_SET(x11_fd, &mut in_fds);

            // Set up a timeout of 100ms
            let mut timeout = timeval {
                tv_sec: 0,
                tv_usec: 100000, // 100ms
            };

            if select(
                x11_fd + 1,
                &mut in_fds,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut timeout,
            ) > 0
            {
                while xlib::XPending(display) > 0 {
                    let mut event: xlib::XEvent = std::mem::zeroed();
                    xlib::XNextEvent(display, &mut event);

                    match event.get_type() {
                        xlib::PropertyNotify => {
                            let xproperty = event.property;
                            if xproperty.atom == active_window_atom {
                                let new_active_window =
                                    get_active_window(display, root, active_window_atom);
                                if new_active_window != active_window {
                                    active_window = new_active_window;
                                    if let Some(window_title) = get_window_title(
                                        display,
                                        active_window,
                                        wm_name_atom,
                                        net_wm_name_atom,
                                    ) {
                                        if window_title != last_title {
                                            last_title = window_title.clone();
                                            if let Ok(guard) = handler.lock() {
                                                guard.on_focus_change(window_title);
                                            }
                                        }
                                    }
                                }
                            } else if (xproperty.atom == wm_name_atom
                                || xproperty.atom == net_wm_name_atom)
                                && xproperty.window == active_window
                            {
                                if let Some(window_title) = get_window_title(
                                    display,
                                    active_window,
                                    wm_name_atom,
                                    net_wm_name_atom,
                                ) {
                                    if window_title != last_title {
                                        last_title = window_title.clone();
                                        if let Ok(guard) = handler.lock() {
                                            guard.on_focus_change(window_title);
                                        }
                                    }
                                }
                            }
                        }
                        xlib::CreateNotify | xlib::DestroyNotify => {
                            // Re-check active window and title on window creation or destruction
                            active_window = get_active_window(display, root, active_window_atom);
                            if let Some(window_title) = get_window_title(
                                display,
                                active_window,
                                wm_name_atom,
                                net_wm_name_atom,
                            ) {
                                if window_title != last_title {
                                    last_title = window_title.clone();
                                    if let Ok(guard) = handler.lock() {
                                        guard.on_focus_change(window_title);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // Reset error handler
        xlib::XSetErrorHandler(old_handler);
        xlib::XCloseDisplay(display);
    }

    Ok(())
}

pub fn stop_hook() {
    RUNNING.store(false, Ordering::Relaxed);
}

unsafe fn get_active_window(
    display: *mut xlib::Display,
    root: xlib::Window,
    active_window_atom: xlib::Atom,
) -> xlib::Window {
    let mut actual_type: xlib::Atom = 0;
    let mut actual_format: c_int = 0;
    let mut nitems: c_ulong = 0;
    let mut bytes_after: c_ulong = 0;
    let mut prop: *mut c_char = std::ptr::null_mut();

    if xlib::XGetWindowProperty(
        display,
        root,
        active_window_atom,
        0,
        1,
        xlib::False as i32,
        xlib::XA_WINDOW,
        &mut actual_type,
        &mut actual_format,
        &mut nitems,
        &mut bytes_after,
        &mut prop as *mut *mut c_char as *mut *mut c_uchar,
    ) == 0
        && !prop.is_null()
    {
        let window = *(prop as *const xlib::Window);
        xlib::XFree(prop as *mut c_void);
        window
    } else {
        0
    }
}

unsafe fn get_window_title(
    display: *mut xlib::Display,
    window: xlib::Window,
    wm_name_atom: xlib::Atom,
    net_wm_name_atom: xlib::Atom,
) -> Option<String> {
    let mut actual_type: xlib::Atom = 0;
    let mut actual_format: c_int = 0;
    let mut nitems: c_ulong = 0;
    let mut bytes_after: c_ulong = 0;
    let mut prop: *mut c_char = std::ptr::null_mut();

    // Try _NET_WM_NAME first
    if xlib::XGetWindowProperty(
        display,
        window,
        net_wm_name_atom,
        0,
        1024,
        xlib::False as i32,
        xlib::XInternAtom(display, "UTF8_STRING\0".as_ptr() as *const i8, 0),
        &mut actual_type,
        &mut actual_format,
        &mut nitems,
        &mut bytes_after,
        &mut prop as *mut *mut c_char as *mut *mut c_uchar,
    ) == 0
        && !prop.is_null()
    {
        let title = CStr::from_ptr(prop).to_string_lossy().into_owned();
        xlib::XFree(prop as *mut c_void);
        return Some(title);
    }

    // Fallback to WM_NAME
    if xlib::XGetWindowProperty(
        display,
        window,
        wm_name_atom,
        0,
        1024,
        xlib::False as i32,
        xlib::XA_STRING,
        &mut actual_type,
        &mut actual_format,
        &mut nitems,
        &mut bytes_after,
        &mut prop as *mut *mut c_char as *mut *mut c_uchar,
    ) == 0
        && !prop.is_null()
    {
        let title = CStr::from_ptr(prop).to_string_lossy().into_owned();
        xlib::XFree(prop as *mut c_void);
        return Some(title);
    }

    None // Return None if unable to get window title
}

unsafe extern "C" fn x_error_handler(
    _: *mut xlib::Display,
    error: *mut xlib::XErrorEvent,
) -> c_int {
    // Log the error or handle it as needed
    // For now, we'll just ignore BadWindow errors
    if (*error).error_code == xlib::BadWindow {
        return 0;
    }
    // For other errors, print a warning
    eprintln!("X11 error occurred: {}", (*error).error_code);
    0
}
