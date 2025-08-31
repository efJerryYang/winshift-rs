//! Example to test Core Graphics window identification for stable window IDs
//! using the kCGWindowNumber attribute.

use accessibility_sys::*;
use core_foundation::array::CFArray;
use core_foundation::base::{CFType, ItemRef, TCFType};
use core_foundation::boolean::{kCFBooleanTrue, CFBooleanRef};
use core_foundation::dictionary::CFDictionary;
use core_foundation::dictionary::__CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::number::__CFNumber;
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::display::*;
use objc2::runtime;
use objc2::{class, msg_send, sel, sel_impl};
use std::ffi;

#[link(name = "AppKit", kind = "framework")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGWindowListCreate(option: u32, relativeToWindow: u32) -> *mut ffi::c_void;

    fn CGWindowListCreateDescriptionFromArray(windowArray: *mut ffi::c_void) -> *mut ffi::c_void;

    fn CFRelease(cf: *const ffi::c_void);
}

// const kCGWindowListOptionOnScreenOnly: u32 = 1 << 0;
// const kCGWindowListExcludeDesktopElements: u32 = 1 << 4;
// const kCGNullWindowID: u32 = 0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing Core Graphics Window Identification");
    println!("===========================================");
    println!();

    let frontmost_pid = get_frontmost_app_pid()?;
    println!("Frontmost app PID: {}", frontmost_pid);

    let window_id = find_active_window_id(frontmost_pid)?;
    println!("Active window ID: {}", window_id);

    Ok(())
}

fn get_frontmost_app_pid() -> Result<i32, Box<dyn std::error::Error>> {
    unsafe {
        let workspace_class = class!(NSWorkspace);
        let workspace: *mut runtime::Object = msg_send![workspace_class, sharedWorkspace];
        let frontmost_app: *mut runtime::Object = msg_send![workspace, frontmostApplication];

        if frontmost_app.is_null() {
            return Err("No frontmost application found".into());
        }

        let pid: i32 = msg_send![frontmost_app, processIdentifier];
        Ok(pid)
    }
}

fn find_active_window_id(target_pid: i32) -> Result<u32, Box<dyn std::error::Error>> {
    // Create the on-screen window list
    let window_descriptions = unsafe {
        let window_list_option =
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements;
        let window_ids = CGWindowListCreate(window_list_option, kCGNullWindowID);
        if window_ids.is_null() {
            return Err("Failed to create window list".into());
        }
        let desc = CGWindowListCreateDescriptionFromArray(window_ids);
        CFRelease(window_ids);
        if desc.is_null() {
            return Err("Failed to get window descriptions".into());
        }
        desc
    };

    // Wrap CFArray under get rule
    let descriptions_array = unsafe {
        CFArray::<CFDictionary>::wrap_under_get_rule(
            window_descriptions as *const core_foundation::array::__CFArray,
        )
    };

    let count = descriptions_array.len();
    println!("Found {} windows on screen", count);

    // Debug filtering counters
    let mut examined: usize = 0;
    let mut filtered_pid: usize = 0;
    let mut filtered_layer: usize = 0;
    let mut filtered_offscreen: usize = 0;
    let mut filtered_alpha: usize = 0;
    let mut filtered_missing_winid: usize = 0;

    for i in 0..count {
        if let Some(window_dict) = descriptions_array.get(i) {
            examined += 1;
            let fields = extract_window_fields(&window_dict);

            // Verbose per-window debug line
            println!(
                "[{:03}] CG pid={:?} layer={:?} onscreen={:?} alpha={:?} id={:?} owner={:?} title={:?} bounds={:?}",
                i,
                fields.pid,
                fields.layer,
                fields.onscreen,
                fields.alpha.map(|a| (a * 1000.0).round() / 1000.0),
                fields.window_number,
                fields.owner_name.as_deref().unwrap_or("<none>"),
                fields.window_name.as_deref().unwrap_or("<none>"),
                fields.bounds,
            );

            let pid_ok = fields.pid == Some(target_pid);
            let layer_ok = fields.layer == Some(0);
            let onscreen_ok = fields.onscreen == Some(true);
            let alpha_ok = fields.alpha.map(|a| a > 0.0).unwrap_or(false);

            if pid_ok && layer_ok && onscreen_ok && alpha_ok {
                if let Some(win_id) = fields.window_number.map(|v| v as u32) {
                    println!(
                        "Selected window: id={}, pid={}, layer={}, onscreen={}, alpha={:.3}",
                        win_id,
                        fields.pid.unwrap_or_default(),
                        fields.layer.unwrap_or_default(),
                        fields.onscreen.unwrap_or(false),
                        fields.alpha.unwrap_or(0.0)
                    );
                    // Additional debug: compare CG window info with AX focused window
                    debug_compare_with_ax(&window_dict, target_pid, win_id);
                    println!("Checked {} windows before selecting.", examined);
                    unsafe { CFRelease(window_descriptions) };
                    return Ok(win_id);
                } else {
                    filtered_missing_winid += 1;
                    println!("  -> excluded: missing kCGWindowNumber");
                }
            } else {
                if !pid_ok {
                    filtered_pid += 1;
                    println!(
                        "  -> excluded: PID mismatch (expected {}, got {:?})",
                        target_pid, fields.pid
                    );
                }
                if !layer_ok {
                    filtered_layer += 1;
                    println!("  -> excluded: non-zero layer ({:?})", fields.layer);
                }
                if !onscreen_ok {
                    filtered_offscreen += 1;
                    println!("  -> excluded: not onscreen");
                }
                if !alpha_ok {
                    filtered_alpha += 1;
                    println!("  -> excluded: alpha <= 0 ({:?})", fields.alpha);
                }
            }
        }
    }

    println!("Filtered summary (before selection):");
    println!("  Examined: {}", examined);
    println!("  PID mismatch: {}", filtered_pid);
    println!("  Non-zero layer: {}", filtered_layer);
    println!("  Offscreen: {}", filtered_offscreen);
    println!("  Alpha <= 0: {}", filtered_alpha);
    println!("  Missing window ID: {}", filtered_missing_winid);

    unsafe { CFRelease(window_descriptions) };
    Err("No qualifying window found for frontmost app".into())
}

#[derive(Debug, Clone)]
struct WindowFields {
    pid: Option<i32>,
    layer: Option<i32>,
    onscreen: Option<bool>,
    alpha: Option<f64>,
    window_number: Option<i32>,
    // Extra verbose fields
    bounds: Option<String>,
    owner_name: Option<String>,
    window_name: Option<String>,
}

fn extract_window_fields(window_dict: &CFDictionary) -> WindowFields {
    unsafe {
        let pid_key = CFString::from_static_string("kCGWindowOwnerPID");
        let layer_key = CFString::from_static_string("kCGWindowLayer");
        let onscreen_key = CFString::from_static_string("kCGWindowIsOnscreen");
        let alpha_key = CFString::from_static_string("kCGWindowAlpha");
        let window_id_key = CFString::from_static_string("kCGWindowNumber");
        let owner_name_key = CFString::from_static_string("kCGWindowOwnerName");
        let window_name_key = CFString::from_static_string("kCGWindowName");
        let bounds_key = CFString::from_static_string("kCGWindowBounds");

        let pid_ptr = *window_dict.get(pid_key.as_concrete_TypeRef() as *const _);
        let layer_ptr =
            *window_dict.get(layer_key.as_concrete_TypeRef() as *const _) as *const __CFNumber;
        let onscreen_ref =
            *window_dict.get(onscreen_key.as_concrete_TypeRef() as *const _) as CFBooleanRef;
        let alpha_ptr =
            *window_dict.get(alpha_key.as_concrete_TypeRef() as *const _) as *const __CFNumber;
        let window_id_ptr =
            *window_dict.get(window_id_key.as_concrete_TypeRef() as *const _) as *const __CFNumber;
        let owner_name_ptr = *window_dict.get(owner_name_key.as_concrete_TypeRef() as *const _);
        let window_name_ptr = *window_dict.get(window_name_key.as_concrete_TypeRef() as *const _);
        let bounds_ptr = *window_dict.get(bounds_key.as_concrete_TypeRef() as *const _);

        let pid = if !pid_ptr.is_null() {
            CFNumber::wrap_under_get_rule(pid_ptr as *const __CFNumber).to_i32()
        } else {
            None
        };

        let layer = if !layer_ptr.is_null() {
            CFNumber::wrap_under_get_rule(layer_ptr).to_i32()
        } else {
            None
        };

        let onscreen = if !onscreen_ref.is_null() {
            Some(onscreen_ref == kCFBooleanTrue)
        } else {
            None
        };

        let alpha = if !alpha_ptr.is_null() {
            CFNumber::wrap_under_get_rule(alpha_ptr).to_f64()
        } else {
            None
        };

        let window_number = if !window_id_ptr.is_null() {
            CFNumber::wrap_under_get_rule(window_id_ptr).to_i32()
        } else {
            None
        };

        let owner_name = if !owner_name_ptr.is_null() {
            let s = CFString::wrap_under_get_rule(owner_name_ptr as CFStringRef);
            Some(s.to_string())
        } else {
            None
        };

        let window_name = if !window_name_ptr.is_null() {
            let s = CFString::wrap_under_get_rule(window_name_ptr as CFStringRef);
            Some(s.to_string())
        } else {
            None
        };

        let bounds = if !bounds_ptr.is_null() {
            let cf_value = CFType::wrap_under_get_rule(bounds_ptr);
            Some(format!("{:#?}", cf_value))
        } else {
            None
        };

        WindowFields {
            pid,
            layer,
            onscreen,
            alpha,
            window_number,
            bounds,
            owner_name,
            window_name,
        }
    }
}

fn debug_compare_with_ax(window_dict: &CFDictionary, target_pid: i32, win_id: u32) {
    println!("CG/AX Comparison for selected window ({}):", win_id);
    // Print CG-side details (name/title and bounds)
    unsafe {
        let name_key = CFString::from_static_string("kCGWindowName");
        let bounds_key = CFString::from_static_string("kCGWindowBounds");

        let name_ptr = *window_dict.get(name_key.as_concrete_TypeRef() as *const _);
        if !name_ptr.is_null() {
            let cf_value = CFType::wrap_under_get_rule(name_ptr);
            println!("  CG Title: {:#?}", cf_value);
        } else {
            println!("  CG Title: <none>");
        }

        let bounds_ptr = *window_dict.get(bounds_key.as_concrete_TypeRef() as *const _);
        if !bounds_ptr.is_null() {
            let cf_value = CFType::wrap_under_get_rule(bounds_ptr);
            println!("  CG Bounds: {:#?}", cf_value);
        } else {
            println!("  CG Bounds: <none>");
        }
    }

    // AX-side details: title, position, size for the focused window of this PID
    unsafe {
        if !AXIsProcessTrusted() {
            println!("  AX: Not trusted; cannot fetch AX attributes.");
            return;
        }

        let app_element = AXUIElementCreateApplication(target_pid);

        let mut focused_window: *mut std::ffi::c_void = std::ptr::null_mut();
        let focused_attr = CFString::from_static_string(kAXFocusedWindowAttribute);
        let res = AXUIElementCopyAttributeValue(
            app_element,
            focused_attr.as_concrete_TypeRef(),
            std::ptr::from_mut::<*mut std::ffi::c_void>(&mut focused_window)
                .cast::<*const std::ffi::c_void>(),
        );
        if res != 0 || focused_window.is_null() {
            println!("  AX: Failed to obtain focused window element.");
            return;
        }

        // AX Title
        let mut title_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
        let title_attr = CFString::from_static_string(kAXTitleAttribute);
        let _ = AXUIElementCopyAttributeValue(
            focused_window as _,
            title_attr.as_concrete_TypeRef(),
            std::ptr::from_mut::<*mut std::ffi::c_void>(&mut title_ptr)
                .cast::<*const std::ffi::c_void>(),
        );
        if !title_ptr.is_null() {
            let cf_value = CFType::wrap_under_create_rule(title_ptr);
            println!("  AX Title: {:#?}", cf_value);
        } else {
            println!("  AX Title: <none>");
        }

        // AX Position
        let mut pos_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
        let pos_attr = CFString::from_static_string(kAXPositionAttribute);
        let _ = AXUIElementCopyAttributeValue(
            focused_window as _,
            pos_attr.as_concrete_TypeRef(),
            std::ptr::from_mut::<*mut std::ffi::c_void>(&mut pos_ptr)
                .cast::<*const std::ffi::c_void>(),
        );
        if !pos_ptr.is_null() {
            let cf_value = CFType::wrap_under_create_rule(pos_ptr);
            println!("  AX Position: {:#?}", cf_value);
        } else {
            println!("  AX Position: <none>");
        }

        // AX Size
        let mut size_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
        let size_attr = CFString::from_static_string(kAXSizeAttribute);
        let _ = AXUIElementCopyAttributeValue(
            focused_window as _,
            size_attr.as_concrete_TypeRef(),
            std::ptr::from_mut::<*mut std::ffi::c_void>(&mut size_ptr)
                .cast::<*const std::ffi::c_void>(),
        );
        if !size_ptr.is_null() {
            let cf_value = CFType::wrap_under_create_rule(size_ptr);
            println!("  AX Size: {:#?}", cf_value);
        } else {
            println!("  AX Size: <none>");
        }
    }
}
