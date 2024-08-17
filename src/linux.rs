use crate::error::WinshiftError;
use crate::FocusChangeHandler;
use crate::{log_debug, log_error, log_info, log_trace, log_warn};
use libc::{c_char, c_int, c_uchar, c_ulong, c_void, close, pipe, read, write, EINTR};
use libc::{fd_set, select, FD_SET, FD_ZERO};
use std::ffi::CStr;
use std::os::unix::io::RawFd;
use std::sync::{Arc, RwLock};
use x11::xlib;

static mut INTERRUPT_PIPE: [RawFd; 2] = [-1, -1];

pub(crate) fn run_hook(handler: Arc<RwLock<dyn FocusChangeHandler>>) -> Result<(), WinshiftError> {
    log_debug!("Starting Linux hook");
    unsafe {
        // Create the self-pipe
        if pipe(INTERRUPT_PIPE.as_mut_ptr()) != 0 {
            log_error!("Failed to create interrupt pipe");
            return Err(WinshiftError::InitializationError);
        }
        log_trace!("Interrupt pipe created");

        let display = xlib::XOpenDisplay(std::ptr::null());
        if display.is_null() {
            log_error!("Failed to open X11 display");
            close(INTERRUPT_PIPE[0]);
            close(INTERRUPT_PIPE[1]);
            return Err(WinshiftError::InitializationError);
        }
        log_debug!("X11 display opened successfully");

        let root = xlib::XDefaultRootWindow(display);
        xlib::XSelectInput(
            display,
            root,
            xlib::PropertyChangeMask | xlib::SubstructureNotifyMask,
        );
        log_trace!("Input selection set on root window");

        let active_window_atom =
            xlib::XInternAtom(display, "_NET_ACTIVE_WINDOW\0".as_ptr() as *const i8, 0);
        let wm_name_atom = xlib::XInternAtom(display, "WM_NAME\0".as_ptr() as *const i8, 0);
        let net_wm_name_atom =
            xlib::XInternAtom(display, "_NET_WM_NAME\0".as_ptr() as *const i8, 0);
        log_trace!("X11 atoms initialized");

        let mut active_window: xlib::Window = 0;
        let mut last_title = String::new();

        // Set up error handler
        let old_handler = xlib::XSetErrorHandler(Some(x_error_handler));
        log_trace!("X11 error handler set");

        let x11_fd = xlib::XConnectionNumber(display) as RawFd;
        log_debug!("X11 connection file descriptor: {}", x11_fd);

        let mut in_fds: fd_set = std::mem::zeroed();
        FD_ZERO(&mut in_fds);
        FD_SET(x11_fd, &mut in_fds);
        FD_SET(INTERRUPT_PIPE[0], &mut in_fds);

        let max_fd = x11_fd.max(INTERRUPT_PIPE[0]) + 1;

        loop {
            log_trace!("Waiting for X11 events or interrupt signal");
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
                    log_debug!("Received interrupt signal");
                    let mut buf = [0u8; 1];
                    read(INTERRUPT_PIPE[0], buf.as_mut_ptr() as *mut c_void, 1);
                    break;
                }

                if libc::FD_ISSET(x11_fd, &read_fds) {
                    while xlib::XPending(display) > 0 {
                        let mut event: xlib::XEvent = std::mem::zeroed();
                        xlib::XNextEvent(display, &mut event);
                        log_trace!("Received X11 event type: {}", event.get_type());

                        match event.get_type() {
                            xlib::PropertyNotify => {
                                let xproperty = event.property;
                                if xproperty.atom == active_window_atom {
                                    log_debug!("Active window property changed");
                                    let new_active_window =
                                        get_active_window(display, root, active_window_atom);
                                    if new_active_window != active_window {
                                        log_debug!("New active window: {}", new_active_window);
                                        active_window = new_active_window;
                                        if let Some(window_title) = get_window_title(
                                            display,
                                            active_window,
                                            wm_name_atom,
                                            net_wm_name_atom,
                                        ) {
                                            if window_title != last_title {
                                                log_info!(
                                                    "Window focus changed: '{}' -> '{}'",
                                                    last_title,
                                                    window_title
                                                );
                                                last_title = window_title.clone();
                                                if let Ok(guard) = handler.read() {
                                                    guard.on_focus_change(window_title);
                                                }
                                            }
                                        }
                                    }
                                } else if (xproperty.atom == wm_name_atom
                                    || xproperty.atom == net_wm_name_atom)
                                    && xproperty.window == active_window
                                {
                                    log_debug!("Window title property changed");
                                    if let Some(window_title) = get_window_title(
                                        display,
                                        active_window,
                                        wm_name_atom,
                                        net_wm_name_atom,
                                    ) {
                                        if window_title != last_title {
                                            log_info!(
                                                "Window title changed: '{}' -> '{}'",
                                                last_title,
                                                window_title
                                            );
                                            last_title = window_title.clone();
                                            if let Ok(guard) = handler.read() {
                                                guard.on_focus_change(window_title);
                                            }
                                        }
                                    }
                                }
                            }
                            xlib::CreateNotify | xlib::DestroyNotify => {
                                log_debug!("Window created or destroyed");
                                active_window =
                                    get_active_window(display, root, active_window_atom);
                                if let Some(window_title) = get_window_title(
                                    display,
                                    active_window,
                                    wm_name_atom,
                                    net_wm_name_atom,
                                ) {
                                    if window_title != last_title {
                                        log_info!(
                                            "Window changed: '{}' -> '{}'",
                                            last_title,
                                            window_title
                                        );
                                        last_title = window_title.clone();
                                        if let Ok(guard) = handler.read() {
                                            guard.on_focus_change(window_title);
                                        }
                                    }
                                }
                            }
                            _ => {
                                log_trace!("Ignoring event type: {}", event.get_type());
                            }
                        }
                    }
                }
            } else if *libc::__errno_location() != EINTR {
                log_warn!("select() failed");
            }
        }

        // Reset error handler
        log_trace!("Resetting X11 error handler");
        xlib::XSetErrorHandler(old_handler);
        xlib::XCloseDisplay(display);
        log_debug!("X11 display closed");

        // Close the self-pipe
        close(INTERRUPT_PIPE[0]);
        close(INTERRUPT_PIPE[1]);
        log_trace!("Interrupt pipe closed");
    }

    log_debug!("Linux hook stopped");
    Ok(())
}

pub fn stop_hook() -> Result<(), WinshiftError> {
    log_debug!("Attempting to stop Linux hook");
    unsafe {
        // Send interrupt signal through the pipe
        let buf = [0u8; 1];
        if write(INTERRUPT_PIPE[1], buf.as_ptr() as *const c_void, 1) != 1 {
            log_error!("Failed to send interrupt signal");
            return Err(WinshiftError::StopError);
        }
    }
    log_debug!("Linux hook stop signal sent");
    Ok(())
}

unsafe fn get_active_window(
    display: *mut xlib::Display,
    root: xlib::Window,
    active_window_atom: xlib::Atom,
) -> xlib::Window {
    log_trace!("Getting active window");
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
        log_trace!("Active window: {}", window);
        window
    } else {
        log_warn!("Failed to get active window");
        0
    }
}

unsafe fn get_window_title(
    display: *mut xlib::Display,
    window: xlib::Window,
    wm_name_atom: xlib::Atom,
    net_wm_name_atom: xlib::Atom,
) -> Option<String> {
    log_trace!("Getting window title for window: {}", window);
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
        log_trace!("Window title (_NET_WM_NAME): {}", title);
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
        log_trace!("Window title (WM_NAME): {}", title);
        return Some(title);
    }

    log_warn!("Failed to get window title for window: {}", window);
    None // Return None if unable to get window title
}

unsafe extern "C" fn x_error_handler(
    _: *mut xlib::Display,
    error: *mut xlib::XErrorEvent,
) -> c_int {
    // Log the error or handle it as needed
    // For now, we'll just ignore BadWindow errors
    if (*error).error_code == xlib::BadWindow {
        log_trace!("Ignoring BadWindow error");
        return 0;
    }
    // For other errors, print a warning
    log_warn!("X11 error occurred: {}", (*error).error_code);
    0
}
