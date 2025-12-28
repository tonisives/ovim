//! Indicator window commands

use tauri::{Manager, WebviewWindow};

/// Set whether the indicator window ignores mouse events (click-through)
#[tauri::command]
pub fn set_indicator_ignores_mouse(app: tauri::AppHandle, ignore: bool) -> Result<(), String> {
    let window = app
        .get_webview_window("indicator")
        .ok_or("Indicator window not found")?;

    set_ignore_mouse_events(&window, ignore)
}

#[allow(unused_variables)]
fn set_ignore_mouse_events(window: &WebviewWindow, ignore: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    #[allow(deprecated)]
    {
        use cocoa::base::id;

        let ns_window = window.ns_window().map_err(|e| e.to_string())? as id;

        unsafe {
            use objc::*;
            let _: () = msg_send![ns_window, setIgnoresMouseEvents: ignore];
        }
    }

    Ok(())
}

/// Check if mouse is over the indicator window
#[tauri::command]
#[allow(deprecated)]
pub fn is_mouse_over_indicator(app: tauri::AppHandle) -> bool {
    #[cfg(target_os = "macos")]
    {
        use cocoa::base::id;
        use cocoa::foundation::NSRect;
        use tauri::Manager;

        if let Some(window) = app.get_webview_window("indicator") {
            // Get mouse position using CoreGraphics
            #[link(name = "CoreGraphics", kind = "framework")]
            extern "C" {
                fn CGEventCreate(source: *const std::ffi::c_void) -> *mut std::ffi::c_void;
                fn CGEventGetLocation(event: *const std::ffi::c_void) -> CGPoint;
                fn CFRelease(cf: *const std::ffi::c_void);
            }

            #[repr(C)]
            #[derive(Copy, Clone)]
            struct CGPoint {
                x: f64,
                y: f64,
            }

            let mouse_pos = unsafe {
                let event = CGEventCreate(std::ptr::null());
                if event.is_null() {
                    return false;
                }
                let pos = CGEventGetLocation(event);
                CFRelease(event);
                pos
            };

            // Get window frame using native macOS API
            let ns_window = match window.ns_window() {
                Ok(w) => w as id,
                Err(_) => return false,
            };

            let frame: NSRect = unsafe {
                use objc::*;
                msg_send![ns_window, frame]
            };

            // Get main screen height for coordinate conversion
            // macOS uses bottom-left origin, CoreGraphics uses top-left
            let screen_height: f64 = unsafe {
                use objc::*;
                let screens: id = msg_send![class!(NSScreen), screens];
                let main_screen: id = msg_send![screens, objectAtIndex: 0u64];
                let screen_frame: NSRect = msg_send![main_screen, frame];
                screen_frame.size.height
            };

            // Convert window frame from bottom-left to top-left coordinates
            let wx = frame.origin.x;
            let wy = screen_height - frame.origin.y - frame.size.height;
            let ww = frame.size.width;
            let wh = frame.size.height;

            let is_over = mouse_pos.x >= wx
                && mouse_pos.x <= wx + ww
                && mouse_pos.y >= wy
                && mouse_pos.y <= wy + wh;

            return is_over;
        }
        false
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// Check if the Command key is currently pressed
#[tauri::command]
pub fn is_command_key_pressed() -> bool {
    #[cfg(target_os = "macos")]
    {
        use core_graphics::event::CGEventFlags;

        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGEventSourceFlagsState(stateID: i32) -> u64;
        }

        const COMBINED_SESSION_STATE: i32 = 0;

        unsafe {
            let flags = CGEventSourceFlagsState(COMBINED_SESSION_STATE);
            (flags & CGEventFlags::CGEventFlagCommand.bits()) != 0
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}
