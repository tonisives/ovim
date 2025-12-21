//! Accessibility APIs for getting text from focused UI elements

use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::string::CFString;

#[allow(non_upper_case_globals)]
const kAXValueCGPointType: i32 = 1;
#[allow(non_upper_case_globals)]
const kAXValueCGSizeType: i32 = 2;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateSystemWide() -> CFTypeRef;
    fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
    fn AXUIElementCopyAttributeValue(
        element: CFTypeRef,
        attribute: CFTypeRef,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXValueGetValue(
        value: CFTypeRef,
        the_type: i32,
        value_ptr: *mut std::ffi::c_void,
    ) -> bool;
}

/// RAII wrapper for CFTypeRef that automatically releases the reference when dropped.
struct CFHandle(CFTypeRef);

impl CFHandle {
    /// Create a new handle from a CFTypeRef. Returns None if the pointer is null.
    fn new(ptr: CFTypeRef) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            Some(Self(ptr))
        }
    }

    /// Get an attribute value from this element
    fn get_attribute(&self, attr_name: &str) -> Option<CFHandle> {
        let attr = CFString::new(attr_name);
        let mut value: CFTypeRef = std::ptr::null();
        let result =
            unsafe { AXUIElementCopyAttributeValue(self.0, attr.as_CFTypeRef(), &mut value) };
        if result != 0 || value.is_null() {
            None
        } else {
            Some(CFHandle(value))
        }
    }

    /// Extract a CGPoint from an AXValue
    fn extract_point(&self) -> Option<core_graphics::geometry::CGPoint> {
        let mut point = core_graphics::geometry::CGPoint::new(0.0, 0.0);
        let extracted = unsafe {
            AXValueGetValue(
                self.0,
                kAXValueCGPointType,
                &mut point as *mut _ as *mut std::ffi::c_void,
            )
        };
        if extracted {
            Some(point)
        } else {
            None
        }
    }

    /// Extract a CGSize from an AXValue
    fn extract_size(&self) -> Option<core_graphics::geometry::CGSize> {
        let mut size = core_graphics::geometry::CGSize::new(0.0, 0.0);
        let extracted = unsafe {
            AXValueGetValue(
                self.0,
                kAXValueCGSizeType,
                &mut size as *mut _ as *mut std::ffi::c_void,
            )
        };
        if extracted {
            Some(size)
        } else {
            None
        }
    }

    /// Convert to CFString and get as Rust String.
    /// Note: This consumes the handle to avoid double-free.
    fn into_string(self) -> Option<String> {
        // wrap_under_create_rule takes ownership, so we need to prevent
        // our Drop from also releasing
        let cf_string: CFString = unsafe { CFString::wrap_under_create_rule(self.0 as _) };
        let result = cf_string.to_string();
        // Prevent double-free by forgetting self (CFString now owns the ref)
        std::mem::forget(self);
        Some(result)
    }
}

impl Drop for CFHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) };
        }
    }
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

        let bundle_id: *mut objc::runtime::Object = msg_send![app, bundleIdentifier];
        if bundle_id.is_null() {
            return None;
        }

        let utf8: *const std::os::raw::c_char = msg_send![bundle_id, UTF8String];
        if utf8.is_null() {
            return None;
        }

        let bundle_id_str = std::ffi::CStr::from_ptr(utf8)
            .to_string_lossy()
            .into_owned();

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

        let running_app_class = class!(NSRunningApplication);
        let app: *mut objc::runtime::Object = msg_send![
            running_app_class,
            runningApplicationWithProcessIdentifier: context.app_pid
        ];

        if app.is_null() {
            log::error!(
                "Could not find running application with PID {}",
                context.app_pid
            );
            return Err(format!(
                "Could not find running application with PID {}",
                context.app_pid
            ));
        }

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

/// Get the frame of the frontmost window of the focused application
pub fn get_focused_window_frame() -> Option<ElementFrame> {
    let context = capture_focus_context()?;
    get_window_frame_for_pid(context.app_pid)
}

/// Get the frame of the focused window for a specific application PID
pub fn get_window_frame_for_pid(pid: i32) -> Option<ElementFrame> {
    let app_element = CFHandle::new(unsafe { AXUIElementCreateApplication(pid) })?;

    let focused_window = app_element.get_attribute("AXFocusedWindow").or_else(|| {
        log::warn!(
            "get_window_frame_for_pid: Failed to get AXFocusedWindow for pid {}",
            pid
        );
        None
    })?;

    let position_value = focused_window.get_attribute("AXPosition").or_else(|| {
        log::warn!("get_window_frame_for_pid: Failed to get AXPosition");
        None
    })?;

    let size_value = focused_window.get_attribute("AXSize").or_else(|| {
        log::warn!("get_window_frame_for_pid: Failed to get AXSize");
        None
    })?;

    let point = position_value.extract_point().or_else(|| {
        log::warn!("get_window_frame_for_pid: Failed to extract CGPoint from AXPosition");
        None
    })?;

    let size = size_value.extract_size().or_else(|| {
        log::warn!("get_window_frame_for_pid: Failed to extract CGSize from AXSize");
        None
    })?;

    log::info!(
        "get_window_frame_for_pid: Got frame x={}, y={}, w={}, h={}",
        point.x,
        point.y,
        size.width,
        size.height
    );

    Some(ElementFrame {
        x: point.x,
        y: point.y,
        width: size.width,
        height: size.height,
    })
}

/// Get the position and size of the currently focused UI element
pub fn get_focused_element_frame() -> Option<ElementFrame> {
    let system_wide = CFHandle::new(unsafe { AXUIElementCreateSystemWide() })?;
    let focused_app = system_wide.get_attribute("AXFocusedApplication")?;
    let focused_element = focused_app.get_attribute("AXFocusedUIElement")?;

    let position_value = focused_element.get_attribute("AXPosition")?;
    let size_value = focused_element.get_attribute("AXSize")?;

    let point = position_value.extract_point()?;
    let size = size_value.extract_size()?;

    Some(ElementFrame {
        x: point.x,
        y: point.y,
        width: size.width,
        height: size.height,
    })
}

/// Get the full text value from the currently focused UI element
pub fn get_focused_element_text() -> Option<String> {
    let system_wide = CFHandle::new(unsafe { AXUIElementCreateSystemWide() })?;
    let focused_app = system_wide.get_attribute("AXFocusedApplication")?;
    let focused_element = focused_app.get_attribute("AXFocusedUIElement")?;
    let value = focused_element.get_attribute("AXValue")?;
    value.into_string()
}
