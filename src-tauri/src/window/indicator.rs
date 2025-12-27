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
