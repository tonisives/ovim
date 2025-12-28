use tauri::WebviewWindow;

/// Set up the indicator window with special properties
#[allow(unused_variables)]
pub fn setup_indicator_window(window: &WebviewWindow) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    #[allow(deprecated)]
    {
        use cocoa::appkit::NSWindowCollectionBehavior;
        use cocoa::base::id;

        let ns_window = window.ns_window().map_err(|e| e.to_string())? as id;

        unsafe {
            // Set window level to floating
            use objc::*;
            let _: () = msg_send![ns_window, setLevel: 3i64]; // NSFloatingWindowLevel

            // Set collection behavior to appear on all spaces
            use cocoa::appkit::NSWindow;
            ns_window.setCollectionBehavior_(
                NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
                    | NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary,
            );
        }
    }

    Ok(())
}

/// Set whether the indicator window ignores mouse events
#[allow(unused_variables)]
pub fn set_indicator_ignores_mouse(window: &WebviewWindow, ignore: bool) -> Result<(), String> {
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

/// Set up the click overlay window with special properties
#[allow(unused_variables)]
pub fn setup_click_overlay_window(window: &WebviewWindow) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    #[allow(deprecated)]
    {
        use cocoa::appkit::NSWindowCollectionBehavior;
        use cocoa::base::id;

        let ns_window = window.ns_window().map_err(|e| e.to_string())? as id;

        unsafe {
            use objc::*;

            // Set window level above everything (screen saver level)
            let _: () = msg_send![ns_window, setLevel: 1000i64];

            // Set collection behavior to appear on all spaces
            use cocoa::appkit::NSWindow;
            ns_window.setCollectionBehavior_(
                NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
                    | NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary
                    | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary,
            );

            // Make window ignore mouse events initially (hints will handle clicks)
            let _: () = msg_send![ns_window, setIgnoresMouseEvents: false];
        }
    }

    Ok(())
}

/// Position the click overlay to cover all screens
#[allow(unused_variables)]
#[allow(dead_code)]
pub fn position_click_overlay_fullscreen(window: &WebviewWindow) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use objc::{class, msg_send, sel, sel_impl};

        unsafe {
            // Get all screens and calculate bounding box
            let screens: *mut objc::runtime::Object = msg_send![class!(NSScreen), screens];
            if screens.is_null() {
                return Err("Could not get screens".to_string());
            }

            let count: usize = msg_send![screens, count];
            if count == 0 {
                return Err("No screens found".to_string());
            }

            let mut min_x: f64 = f64::MAX;
            let mut min_y: f64 = f64::MAX;
            let mut max_x: f64 = f64::MIN;
            let mut max_y: f64 = f64::MIN;

            // Get main screen height for coordinate conversion
            let main_screen: *mut objc::runtime::Object = msg_send![class!(NSScreen), mainScreen];
            let main_frame: core_graphics::geometry::CGRect = msg_send![main_screen, frame];
            let main_height = main_frame.size.height;

            for i in 0..count {
                let screen: *mut objc::runtime::Object = msg_send![screens, objectAtIndex: i];
                if screen.is_null() {
                    continue;
                }

                let frame: core_graphics::geometry::CGRect = msg_send![screen, frame];

                // Convert from Cocoa coordinates (origin at bottom-left) to screen coordinates
                let screen_x = frame.origin.x;
                let screen_y = main_height - frame.origin.y - frame.size.height;

                min_x = min_x.min(screen_x);
                min_y = min_y.min(screen_y);
                max_x = max_x.max(screen_x + frame.size.width);
                max_y = max_y.max(screen_y + frame.size.height);
            }

            let width = (max_x - min_x) as u32;
            let height = (max_y - min_y) as u32;

            log::info!(
                "Positioning click overlay: x={}, y={}, w={}, h={}",
                min_x,
                min_y,
                width,
                height
            );

            // Set window position and size
            let _ = window.set_position(tauri::Position::Physical(
                tauri::PhysicalPosition::new(min_x as i32, min_y as i32),
            ));
            let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
                width, height,
            )));
        }
    }

    Ok(())
}
