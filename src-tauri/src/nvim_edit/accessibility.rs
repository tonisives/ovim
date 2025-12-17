//! Accessibility APIs for getting text from focused UI elements

use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::string::CFString;

// AXValue types for position and size
type AXValueRef = CFTypeRef;
const kAXValueCGPointType: i32 = 1;
const kAXValueCGSizeType: i32 = 2;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateSystemWide() -> CFTypeRef;
    fn AXUIElementCopyAttributeValue(
        element: CFTypeRef,
        attribute: CFTypeRef,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXValueGetValue(
        value: AXValueRef,
        the_type: i32,
        value_ptr: *mut std::ffi::c_void,
    ) -> bool;
}

/// Context about the focused application for later restoration
#[derive(Debug, Clone)]
pub struct FocusContext {
    pub app_pid: i32,
    #[allow(dead_code)]
    pub app_bundle_id: String,
}

/// Capture the current focus context (which app is focused)
pub fn capture_focus_context() -> Option<FocusContext> {
    unsafe {
        // Use NSWorkspace to get the frontmost app
        use objc::{class, msg_send, sel, sel_impl};

        let workspace: *mut objc::runtime::Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace.is_null() {
            return None;
        }

        let app: *mut objc::runtime::Object = msg_send![workspace, frontmostApplication];
        if app.is_null() {
            return None;
        }

        // Get PID
        let pid: i32 = msg_send![app, processIdentifier];

        // Get bundle ID
        let bundle_id: *mut objc::runtime::Object = msg_send![app, bundleIdentifier];
        if bundle_id.is_null() {
            return None;
        }

        let utf8: *const std::os::raw::c_char = msg_send![bundle_id, UTF8String];
        if utf8.is_null() {
            return None;
        }

        let bundle_id_str = std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned();

        Some(FocusContext {
            app_pid: pid,
            app_bundle_id: bundle_id_str,
        })
    }
}

/// Restore focus to a previously captured application
pub fn restore_focus(context: &FocusContext) -> Result<(), String> {
    log::info!("Attempting to restore focus to PID {}", context.app_pid);

    unsafe {
        use objc::{class, msg_send, sel, sel_impl};

        // Get NSRunningApplication for the PID
        let running_app_class = class!(NSRunningApplication);
        let app: *mut objc::runtime::Object = msg_send![
            running_app_class,
            runningApplicationWithProcessIdentifier: context.app_pid
        ];

        if app.is_null() {
            log::error!("Could not find running application with PID {}", context.app_pid);
            return Err(format!(
                "Could not find running application with PID {}",
                context.app_pid
            ));
        }

        // Activate the application
        // NSApplicationActivateIgnoringOtherApps = 1 << 1 = 2
        let options: u64 = 2;
        let success: bool = msg_send![app, activateWithOptions: options];

        if success {
            log::info!("Successfully activated application");
            Ok(())
        } else {
            log::error!("Failed to activate application");
            Err("Failed to activate application".to_string())
        }
    }
}

/// Position and size of a UI element
#[derive(Debug, Clone)]
pub struct ElementFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Get the position and size of the currently focused UI element
pub fn get_focused_element_frame() -> Option<ElementFrame> {
    unsafe {
        let system_wide = AXUIElementCreateSystemWide();
        if system_wide.is_null() {
            return None;
        }

        // Get focused application
        let focused_app_attr = CFString::new("AXFocusedApplication");
        let mut focused_app: CFTypeRef = std::ptr::null();
        let result = AXUIElementCopyAttributeValue(
            system_wide,
            focused_app_attr.as_CFTypeRef(),
            &mut focused_app,
        );

        if result != 0 || focused_app.is_null() {
            CFRelease(system_wide);
            return None;
        }

        // Get focused UI element from the application
        let focused_element_attr = CFString::new("AXFocusedUIElement");
        let mut focused_element: CFTypeRef = std::ptr::null();
        let result = AXUIElementCopyAttributeValue(
            focused_app,
            focused_element_attr.as_CFTypeRef(),
            &mut focused_element,
        );

        if result != 0 || focused_element.is_null() {
            CFRelease(focused_app);
            CFRelease(system_wide);
            return None;
        }

        // Get position (AXPosition)
        let position_attr = CFString::new("AXPosition");
        let mut position_value: CFTypeRef = std::ptr::null();
        let result = AXUIElementCopyAttributeValue(
            focused_element,
            position_attr.as_CFTypeRef(),
            &mut position_value,
        );

        if result != 0 || position_value.is_null() {
            CFRelease(focused_element);
            CFRelease(focused_app);
            CFRelease(system_wide);
            return None;
        }

        // Get size (AXSize)
        let size_attr = CFString::new("AXSize");
        let mut size_value: CFTypeRef = std::ptr::null();
        let result = AXUIElementCopyAttributeValue(
            focused_element,
            size_attr.as_CFTypeRef(),
            &mut size_value,
        );

        if result != 0 || size_value.is_null() {
            CFRelease(position_value);
            CFRelease(focused_element);
            CFRelease(focused_app);
            CFRelease(system_wide);
            return None;
        }

        // Extract CGPoint from AXValue (position)
        let mut point = core_graphics::geometry::CGPoint::new(0.0, 0.0);
        let extracted = AXValueGetValue(
            position_value as AXValueRef,
            kAXValueCGPointType,
            &mut point as *mut _ as *mut std::ffi::c_void,
        );

        if !extracted {
            CFRelease(size_value);
            CFRelease(position_value);
            CFRelease(focused_element);
            CFRelease(focused_app);
            CFRelease(system_wide);
            return None;
        }

        // Extract CGSize from AXValue (size)
        let mut size = core_graphics::geometry::CGSize::new(0.0, 0.0);
        let extracted = AXValueGetValue(
            size_value as AXValueRef,
            kAXValueCGSizeType,
            &mut size as *mut _ as *mut std::ffi::c_void,
        );

        CFRelease(size_value);
        CFRelease(position_value);
        CFRelease(focused_element);
        CFRelease(focused_app);
        CFRelease(system_wide);

        if !extracted {
            return None;
        }

        Some(ElementFrame {
            x: point.x,
            y: point.y,
            width: size.width,
            height: size.height,
        })
    }
}

/// Get the full text value from the currently focused UI element
pub fn get_focused_element_text() -> Option<String> {
    unsafe {
        let system_wide = AXUIElementCreateSystemWide();
        if system_wide.is_null() {
            return None;
        }

        // Get focused application
        let focused_app_attr = CFString::new("AXFocusedApplication");
        let mut focused_app: CFTypeRef = std::ptr::null();
        let result = AXUIElementCopyAttributeValue(
            system_wide,
            focused_app_attr.as_CFTypeRef(),
            &mut focused_app,
        );

        if result != 0 || focused_app.is_null() {
            CFRelease(system_wide);
            return None;
        }

        // Get focused UI element from the application
        let focused_element_attr = CFString::new("AXFocusedUIElement");
        let mut focused_element: CFTypeRef = std::ptr::null();
        let result = AXUIElementCopyAttributeValue(
            focused_app,
            focused_element_attr.as_CFTypeRef(),
            &mut focused_element,
        );

        if result != 0 || focused_element.is_null() {
            CFRelease(focused_app);
            CFRelease(system_wide);
            return None;
        }

        // Get the full text value (AXValue)
        let value_attr = CFString::new("AXValue");
        let mut value: CFTypeRef = std::ptr::null();
        let result = AXUIElementCopyAttributeValue(
            focused_element,
            value_attr.as_CFTypeRef(),
            &mut value,
        );

        CFRelease(focused_element);
        CFRelease(focused_app);
        CFRelease(system_wide);

        if result != 0 || value.is_null() {
            return None;
        }

        // Convert CFString to Rust String
        let cf_string: CFString = CFString::wrap_under_create_rule(value as _);
        Some(cf_string.to_string())
    }
}
