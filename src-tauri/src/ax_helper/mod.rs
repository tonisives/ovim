//! Accessibility helper subprocess
//!
//! This binary queries the accessibility tree and outputs element data as JSON.
//! It's run as a subprocess so that if the accessibility API throws an
//! Objective-C exception, it crashes this process instead of the main app.
//!
//! Note: We don't use objc_exception here because it's incompatible with Rust's
//! stack unwinding. Instead, we just let the subprocess crash if an exception
//! occurs - that's the whole point of the subprocess design.

#![allow(unexpected_cfgs)]

mod bindings;
mod cf_handle;
mod collect;
mod element;
mod menu;
mod types;

use core_foundation::base::CFRetain;
use std::env;

use bindings::AXUIElementCreateApplication;
use cf_handle::CFHandle;
use collect::collect_elements_inner;
use menu::collect_menu_elements;
use types::{HelperOutput, RawElement, WindowBounds};

fn get_frontmost_app_pid() -> Option<i32> {
    unsafe {
        use objc::{class, msg_send, sel, sel_impl};

        let workspace: *mut objc::runtime::Object =
            msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace.is_null() {
            return None;
        }

        let app: *mut objc::runtime::Object = msg_send![workspace, frontmostApplication];
        if app.is_null() {
            return None;
        }

        let pid: i32 = msg_send![app, processIdentifier];
        Some(pid)
    }
}

/// Get window bounds from a window element
fn get_window_bounds(window: &CFHandle) -> Option<WindowBounds> {
    let pos = window
        .get_attribute("AXPosition")
        .and_then(|p| p.extract_point())?;
    let size = window
        .get_attribute("AXSize")
        .and_then(|s| s.extract_size())?;

    Some(WindowBounds {
        x: pos.0,
        y: pos.1,
        width: size.0,
        height: size.1,
    })
}

/// Inner function that does all the work without try_objc wrappers
/// This is safe to call from within a try_objc block
fn query_elements_inner(pid: i32) -> Result<HelperOutput, String> {
    let mut elements: Vec<RawElement> = Vec::new();

    // First, check if there's an open menu (popup menu, context menu, etc.)
    // These take priority as they're the most likely target when visible
    if collect_menu_elements(&mut elements, pid) {
        return Ok(HelperOutput {
            elements,
            is_modal: true,
        });
    }

    let app_element = unsafe {
        let ptr = AXUIElementCreateApplication(pid);
        if ptr.is_null() {
            return Err("Could not create AX element for app".to_string());
        }
        CFHandle(ptr)
    };

    // Test if we can safely access the app's role
    // This is a simple operation that should fail fast if the app is in a bad state
    let _role = app_element.get_string_attribute("AXRole");

    // Try to get focused window first, fall back to app element
    let focused_window = app_element.get_attribute("AXFocusedWindow").and_then(|w| {
        let ptr = w.0;
        if ptr.is_null() {
            return None;
        }
        // Test if we can access the window's role before using it
        let role_test = w.get_string_attribute("AXRole");
        if role_test.is_none() {
            return None;
        }
        std::mem::forget(w);
        unsafe { CFRetain(ptr) };
        Some(CFHandle(ptr))
    });

    // Check for sheets or dialogs (file picker dialogs, save panels, etc.) on the focused window
    // These take priority as they are modal UI that needs interaction
    if let Some(ref window) = focused_window {
        // Try AXSheets first (attached sheet dialogs)
        if let Some(sheets) = window.get_attribute("AXSheets") {
            let sheets_ptr = sheets.0;
            let count = unsafe { core_foundation::array::CFArrayGetCount(sheets_ptr as _) };
            if count > 0 {
                // Get the first (topmost) sheet
                let sheet_ptr = unsafe {
                    core_foundation::array::CFArrayGetValueAtIndex(sheets_ptr as _, 0)
                };
                if !sheet_ptr.is_null() {
                    unsafe { CFRetain(sheet_ptr) };
                    let sheet = CFHandle(sheet_ptr);
                    let sheet_bounds = get_window_bounds(&sheet);
                    // Collect elements from the sheet instead of the window
                    collect_elements_inner(&sheet, &mut elements, 0, sheet_bounds, false);
                    return Ok(HelperOutput {
                        elements,
                        is_modal: true,
                    });
                }
            }
        }

        // Try AXDialogs (modal dialogs)
        if let Some(dialogs) = window.get_attribute("AXDialogs") {
            let dialogs_ptr = dialogs.0;
            let count = unsafe { core_foundation::array::CFArrayGetCount(dialogs_ptr as _) };
            if count > 0 {
                let dialog_ptr = unsafe {
                    core_foundation::array::CFArrayGetValueAtIndex(dialogs_ptr as _, 0)
                };
                if !dialog_ptr.is_null() {
                    unsafe { CFRetain(dialog_ptr) };
                    let dialog = CFHandle(dialog_ptr);
                    let dialog_bounds = get_window_bounds(&dialog);
                    collect_elements_inner(&dialog, &mut elements, 0, dialog_bounds, false);
                    return Ok(HelperOutput {
                        elements,
                        is_modal: true,
                    });
                }
            }
        }
    }

    // Also check app-level for sheets/dialogs (some apps report them at the app level)
    if let Some(sheets) = app_element.get_attribute("AXSheets") {
        let sheets_ptr = sheets.0;
        let count = unsafe { core_foundation::array::CFArrayGetCount(sheets_ptr as _) };
        if count > 0 {
            let sheet_ptr =
                unsafe { core_foundation::array::CFArrayGetValueAtIndex(sheets_ptr as _, 0) };
            if !sheet_ptr.is_null() {
                unsafe { CFRetain(sheet_ptr) };
                let sheet = CFHandle(sheet_ptr);
                let sheet_bounds = get_window_bounds(&sheet);
                collect_elements_inner(&sheet, &mut elements, 0, sheet_bounds, false);
                return Ok(HelperOutput {
                    elements,
                    is_modal: true,
                });
            }
        }
    }

    let (start_element, window_bounds) = focused_window
        .map(|w| {
            let bounds = get_window_bounds(&w);
            (w, bounds)
        })
        .or_else(|| {
            // Try AXWindows array as fallback
            app_element.get_attribute("AXWindows").and_then(|windows| {
                let windows_ptr = windows.0;
                let count = unsafe { core_foundation::array::CFArrayGetCount(windows_ptr as _) };
                if count > 0 {
                    let first = unsafe {
                        core_foundation::array::CFArrayGetValueAtIndex(windows_ptr as _, 0)
                    };
                    if !first.is_null() {
                        unsafe { CFRetain(first) };
                        let handle = CFHandle(first);
                        let bounds = get_window_bounds(&handle);
                        return Some((handle, bounds));
                    }
                }
                None
            })
        })
        .unwrap_or_else(|| {
            let app_ptr = app_element.0;
            unsafe { CFRetain(app_ptr) };
            (CFHandle(app_ptr), None)
        });

    collect_elements_inner(&start_element, &mut elements, 0, window_bounds, false);

    Ok(HelperOutput {
        elements,
        is_modal: false,
    })
}

fn query_elements(pid: i32) -> Result<HelperOutput, String> {
    // Since try_objc uses setjmp/longjmp which doesn't work well with Rust's
    // destructor-based cleanup (closures with captured state, Vec, etc.),
    // we just call the inner function directly.
    //
    // The whole point of this subprocess is that if it crashes, it only
    // crashes the subprocess, not the main app. So we let it crash if needed.
    query_elements_inner(pid)
}

pub fn main() {
    let args: Vec<String> = env::args().collect();

    // Usage: ovim-ax-helper <pid>
    // Or: ovim-ax-helper (uses frontmost app)
    let pid = if args.len() > 1 {
        args[1].parse::<i32>().ok()
    } else {
        get_frontmost_app_pid()
    };

    let pid = match pid {
        Some(p) => p,
        None => {
            eprintln!("{{\"error\": \"Could not determine PID\"}}");
            std::process::exit(1);
        }
    };

    // Reduced delay for faster click mode activation
    // Previously 300ms, reduced to 50ms as a balance between speed and stability
    // If crashes occur, the subprocess design ensures main app survives
    std::thread::sleep(std::time::Duration::from_millis(50));

    match query_elements(pid) {
        Ok(output) => {
            let json = serde_json::to_string(&output)
                .unwrap_or_else(|_| r#"{"elements":[],"is_modal":false}"#.to_string());
            println!("{}", json);
        }
        Err(e) => {
            eprintln!("{{\"error\": \"{}\"}}", e);
            std::process::exit(1);
        }
    }
}
