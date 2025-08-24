//! # Thread Safety Warning
//!
//! This implementation uses `static mut` variables which are not thread-safe.
//! It assumes single-threaded usage on the main thread only.
//!
//! TODO: Replace `static mut INTERRUPT_PIPE` with thread-safe alternative
//! TODO: Consider thread-safe X11 event handling

use std::ffi::CStr;
use std::os::unix::io::RawFd;
use std::sync::{Arc, RwLock};

use libc::{c_char, c_int, c_uchar, c_ulong, c_void, close, pipe, read, write, EINTR};
use libc::{fd_set, select, FD_SET, FD_ZERO};
use log::{debug, error, info, trace, warn};
use x11::xlib;

use crate::error::WinshiftError;
use crate::FocusChangeHandler;

// TODO: Make this thread-safe (e.g. using lazy_static with Mutex)
static mut INTERRUPT_PIPE: [RawFd; 2] = [-1, -1];

pub(crate) fn run_hook_with_config(
    handler: Arc<RwLock<dyn FocusChangeHandler>>,
    _config: &crate::hook::WindowHookConfig,
) -> Result<(), WinshiftError> {
    run_hook(handler)
}

fn run_hook(handler: Arc<RwLock<dyn FocusChangeHandler>>) -> Result<(), WinshiftError> {
    debug!("Starting Linux hook");
    unsafe {
        // Create the self-pipe
        if pipe(INTERRUPT_PIPE.as_mut_ptr()) != 0 {
            error!("Failed to create interrupt pipe");
            return Err(WinshiftError::Initialization);
        }
        trace!("Interrupt pipe created");

        let display = xlib::XOpenDisplay(std::ptr::null());
        if display.is_null() {
            error!("Failed to open X11 display");
            close(INTERRUPT_PIPE[0]);
            close(INTERRUPT_PIPE[1]);
            return Err(WinshiftError::Initialization);
        }
        debug!("X11 display opened successfully");

        let root = xlib::XDefaultRootWindow(display);
        xlib::XSelectInput(
            display,
            root,
            xlib::PropertyChangeMask | xlib::SubstructureNotifyMask,
        );
        trace!("Input selection set on root window");

        let active_window_atom =
            xlib::XInternAtom(display, "_NET_ACTIVE_WINDOW\0".as_ptr() as *const i8, 0);
        let wm_name_atom = xlib::XInternAtom(display, "WM_NAME\0".as_ptr() as *const i8, 0);
        let net_wm_name_atom =
            xlib::XInternAtom(display, "_NET_WM_NAME\0".as_ptr() as *const i8, 0);
        trace!("X11 atoms initialized");

        let mut active_window: xlib::Window = 0;
        let mut last_title = String::new();

        // Set up error handler
        let old_handler = xlib::XSetErrorHandler(Some(x_error_handler));
        trace!("X11 error handler set");

        let x11_fd = xlib::XConnectionNumber(display) as RawFd;
        debug!("X11 connection file descriptor: {}", x11_fd);

        let mut in_fds: fd_set = std::mem::zeroed();
        FD_ZERO(&mut in_fds);
        FD_SET(x11_fd, &mut in_fds);
        FD_SET(INTERRUPT_PIPE[0], &mut in_fds);

        let max_fd = x11_fd.max(INTERRUPT_PIPE[0]) + 1;

        loop {
            trace!("Waiting for X11 events or interrupt signal");
            let mut read_fds = in_fds;

            if select(
                max_fd,
                &mut read_fds,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            ) > 0
            {
                if libc::FD_ISSET(INTERRUPT_PIPE[0], &read_fds) {
                    debug!("Received interrupt signal");
                    let mut buf = [0u8; 1];
                    read(INTERRUPT_PIPE[0], buf.as_mut_ptr() as *mut c_void, 1);
                    break;
                }

                if libc::FD_ISSET(x11_fd, &read_fds) {
                    while xlib::XPending(display) > 0 {
                        let mut event: xlib::XEvent = std::mem::zeroed();
                        xlib::XNextEvent(display, &mut event);
                        trace!("Received X11 event type: {}", event.get_type());

                        match event.get_type() {
                            xlib::PropertyNotify => {
                                let xproperty = event.property;
                                if xproperty.atom == active_window_atom {
                                    debug!("Active window property changed");
                                    let new_active_window =
                                        get_active_window(display, root, active_window_atom);
                                    if new_active_window != active_window {
                                        debug!("New active window: {}", new_active_window);
                                        active_window = new_active_window;
                                        if let Some(window_title) = get_window_title(
                                            display,
                                            active_window,
                                            wm_name_atom,
                                            net_wm_name_atom,
                                        ) {
                                            if window_title != last_title {
                                                info!(
                                                    "Window focus changed: '{}' -> '{}'",
                                                    last_title, window_title
                                                );
                                                last_title = window_title.clone();
                                                if let Ok(guard) = handler.read() {
                                                    guard.on_window_change(window_title);
                                                }
                                            }
                                        }
                                    }
                                } else if (xproperty.atom == wm_name_atom
                                    || xproperty.atom == net_wm_name_atom)
                                    && xproperty.window == active_window
                                {
                                    debug!("Window title property changed");
                                    if let Some(window_title) = get_window_title(
                                        display,
                                        active_window,
                                        wm_name_atom,
                                        net_wm_name_atom,
                                    ) {
                                        if window_title != last_title {
                                            info!(
                                                "Window title changed: '{}' -> '{}'",
                                                last_title, window_title
                                            );
                                            last_title = window_title.clone();
                                                if let Ok(guard) = handler.read() {
                                                    guard.on_window_change(window_title);
                                                }
                                        }
                                    }
                                }
                            }
                            xlib::CreateNotify | xlib::DestroyNotify => {
                                debug!("Window created or destroyed");
                                active_window =
                                    get_active_window(display, root, active_window_atom);
                                if let Some(window_title) = get_window_title(
                                    display,
                                    active_window,
                                    wm_name_atom,
                                    net_wm_name_atom,
                                ) {
                                    if window_title != last_title {
                                        info!(
                                            "Window changed: '{}' -> '{}'",
                                            last_title, window_title
                                        );
                                        last_title = window_title.clone();
                                            if let Ok(guard) = handler.read() {
                                                guard.on_window_change(window_title);
                                            }
                                    }
                                }
                            }
                            _ => {
                                trace!("Ignoring event type: {}", event.get_type());
                            }
                        }
                    }
                }
            } else if *libc::__errno_location() != EINTR {
                warn!("select() failed");
            }
        }

        // Reset error handler
        trace!("Resetting X11 error handler");
        xlib::XSetErrorHandler(old_handler);
        xlib::XCloseDisplay(display);
        debug!("X11 display closed");

        // Close the self-pipe
        close(INTERRUPT_PIPE[0]);
        close(INTERRUPT_PIPE[1]);
        trace!("Interrupt pipe closed");
    }

    debug!("Linux hook stopped");
    Ok(())
}

pub fn stop_hook() -> Result<(), WinshiftError> {
    debug!("Attempting to stop Linux hook");
    unsafe {
        // Send interrupt signal through the pipe
        let buf = [0u8; 1];
        if write(INTERRUPT_PIPE[1], buf.as_ptr() as *const c_void, 1) != 1 {
            error!("Failed to send interrupt signal");
            return Err(WinshiftError::Stop);
        }
    }
    debug!("Linux hook stop signal sent");
    Ok(())
}

unsafe fn get_active_window(
    display: *mut xlib::Display,
    root: xlib::Window,
    active_window_atom: xlib::Atom,
) -> xlib::Window {
    trace!("Getting active window");
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
        trace!("Active window: {}", window);
        window
    } else {
        warn!("Failed to get active window");
        0
    }
}

unsafe fn get_window_title(
    display: *mut xlib::Display,
    window: xlib::Window,
    wm_name_atom: xlib::Atom,
    net_wm_name_atom: xlib::Atom,
) -> Option<String> {
    trace!("Getting window title for window: {}", window);
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
        trace!("Window title (_NET_WM_NAME): {}", title);
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
        trace!("Window title (WM_NAME): {}", title);
        return Some(title);
    }

    warn!("Failed to get window title for window: {}", window);
    None // Return None if unable to get window title
}

unsafe extern "C" fn x_error_handler(
    _: *mut xlib::Display,
    error: *mut xlib::XErrorEvent,
) -> c_int {
    // Log the error or handle it as needed
    // For now, we'll just ignore BadWindow errors
    if (*error).error_code == xlib::BadWindow {
        trace!("Ignoring BadWindow error");
        return 0;
    }
    // For other errors, print a warning
    warn!("X11 error occurred: {}", (*error).error_code);
    0
}
