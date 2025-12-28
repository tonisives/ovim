//! Accessibility API integration for Click Mode
//!
//! Uses macOS Accessibility API to discover clickable UI elements
//! in the frontmost application.

#![allow(dead_code)]

use core_foundation::array::CFArray;
use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::string::CFString;

use crate::nvim_edit::accessibility::AXElementHandle;

use super::element::ClickableElementInternal;
use super::hints::generate_hints;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
    fn AXUIElementCopyAttributeValue(
        element: CFTypeRef,
        attribute: CFTypeRef,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXUIElementCopyAttributeNames(element: CFTypeRef, names: *mut CFTypeRef) -> i32;
    fn AXUIElementPerformAction(element: CFTypeRef, action: CFTypeRef) -> i32;
}

#[allow(non_upper_case_globals)]
const kAXValueCGPointType: i32 = 1;
#[allow(non_upper_case_globals)]
const kAXValueCGSizeType: i32 = 2;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXValueGetValue(
        value: CFTypeRef,
        the_type: i32,
        value_ptr: *mut std::ffi::c_void,
    ) -> bool;
}

/// Roles that are considered clickable
const CLICKABLE_ROLES: &[&str] = &[
    "AXButton",
    "AXLink",
    "AXMenuItem",
    "AXMenuBarItem",
    "AXCheckBox",
    "AXRadioButton",
    "AXPopUpButton",
    "AXComboBox",
    "AXTextField",
    "AXTextArea",
    "AXStaticText",
    "AXImage",
    "AXCell",
    "AXRow",
    "AXTab",
    "AXToolbarButton",
    "AXDisclosureTriangle",
    "AXIncrementor",
    "AXSlider",
];

/// Maximum depth to traverse the UI hierarchy
const MAX_DEPTH: usize = 10;

/// Maximum number of elements to collect (performance limit)
const MAX_ELEMENTS: usize = 200;

/// Get the frontmost application's PID
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

/// RAII wrapper for CFTypeRef
struct CFHandle(CFTypeRef);

impl CFHandle {
    fn new(ptr: CFTypeRef) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            Some(Self(ptr))
        }
    }

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

    fn get_string_attribute(&self, attr_name: &str) -> Option<String> {
        let handle = self.get_attribute(attr_name)?;
        handle.into_string()
    }

    fn into_string(self) -> Option<String> {
        let cf_string: CFString = unsafe { CFString::wrap_under_create_rule(self.0 as _) };
        let result = cf_string.to_string();
        std::mem::forget(self);
        Some(result)
    }

    fn extract_point(&self) -> Option<(f64, f64)> {
        let mut point = core_graphics::geometry::CGPoint::new(0.0, 0.0);
        let extracted = unsafe {
            AXValueGetValue(
                self.0,
                kAXValueCGPointType,
                &mut point as *mut _ as *mut std::ffi::c_void,
            )
        };
        if extracted {
            Some((point.x, point.y))
        } else {
            None
        }
    }

    fn extract_size(&self) -> Option<(f64, f64)> {
        let mut size = core_graphics::geometry::CGSize::new(0.0, 0.0);
        let extracted = unsafe {
            AXValueGetValue(
                self.0,
                kAXValueCGSizeType,
                &mut size as *mut _ as *mut std::ffi::c_void,
            )
        };
        if extracted {
            Some((size.width, size.height))
        } else {
            None
        }
    }

    fn as_ptr(&self) -> CFTypeRef {
        self.0
    }
}

impl Drop for CFHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) };
        }
    }
}

/// Check if an element has a clickable action
fn has_press_action(element: &CFHandle) -> bool {
    let mut actions: CFTypeRef = std::ptr::null();
    let result = unsafe { AXUIElementCopyAttributeNames(element.as_ptr(), &mut actions) };

    if result != 0 || actions.is_null() {
        return false;
    }

    // Check for AXActions attribute
    if let Some(actions_handle) = element.get_attribute("AXActions") {
        // Try to iterate actions array
        let actions_array: CFArray<CFString> =
            unsafe { CFArray::wrap_under_create_rule(actions_handle.0 as _) };
        std::mem::forget(actions_handle);

        for i in 0..actions_array.len() {
            if let Some(action) = actions_array.get(i) {
                let action_str = action.to_string();
                if action_str == "AXPress" || action_str == "AXShowMenu" {
                    return true;
                }
            }
        }
    }

    false
}

/// Check if an element is visible on screen
fn is_visible(element: &CFHandle) -> bool {
    // Check if position and size are valid
    let position = element.get_attribute("AXPosition");
    let size = element.get_attribute("AXSize");

    if position.is_none() || size.is_none() {
        return false;
    }

    let (x, y) = match position.and_then(|p| p.extract_point()) {
        Some(p) => p,
        None => return false,
    };

    let (w, h) = match size.and_then(|s| s.extract_size()) {
        Some(s) => s,
        None => return false,
    };

    // Element must have positive size and be on screen
    w > 0.0 && h > 0.0 && x >= -10000.0 && y >= -10000.0
}

/// Check if a role is clickable
fn is_clickable_role(role: &str) -> bool {
    CLICKABLE_ROLES.iter().any(|r| *r == role)
}

/// Recursively collect clickable elements
fn collect_elements(
    element: &CFHandle,
    elements: &mut Vec<(f64, f64, f64, f64, String, String, AXElementHandle)>,
    depth: usize,
) {
    if depth > MAX_DEPTH || elements.len() >= MAX_ELEMENTS {
        return;
    }

    // Get role
    let role = element.get_string_attribute("AXRole").unwrap_or_default();

    // Check if this element is clickable
    let is_clickable = is_clickable_role(&role) || has_press_action(element);

    if is_clickable && is_visible(element) {
        // Get position and size
        if let (Some(pos), Some(size)) = (
            element.get_attribute("AXPosition").and_then(|p| p.extract_point()),
            element.get_attribute("AXSize").and_then(|s| s.extract_size()),
        ) {
            // Get title (try multiple attributes)
            let title = element
                .get_string_attribute("AXTitle")
                .or_else(|| element.get_string_attribute("AXDescription"))
                .or_else(|| element.get_string_attribute("AXValue"))
                .or_else(|| element.get_string_attribute("AXLabel"))
                .or_else(|| element.get_string_attribute("AXHelp"))
                .unwrap_or_default();

            // Create AX handle for later actions
            if let Some(ax_handle) = unsafe { AXElementHandle::new(element.as_ptr()) } {
                elements.push((pos.0, pos.1, size.0, size.1, role, title, ax_handle));
            }
        }
    }

    // Recurse into children
    if let Some(children) = element.get_attribute("AXChildren") {
        let children_array: CFArray<CFTypeRef> =
            unsafe { CFArray::wrap_under_create_rule(children.0 as _) };
        std::mem::forget(children);

        for i in 0..children_array.len() {
            if elements.len() >= MAX_ELEMENTS {
                break;
            }
            if let Some(child_ref) = children_array.get(i) {
                let child_ptr: CFTypeRef = *child_ref;
                if !child_ptr.is_null() {
                    // Retain the child for our use
                    let child = CFHandle(unsafe {
                        core_foundation::base::CFRetain(child_ptr)
                    });
                    collect_elements(&child, elements, depth + 1);
                }
            }
        }
    }
}

/// Query all clickable elements in the frontmost application
pub fn get_clickable_elements() -> Result<Vec<ClickableElementInternal>, String> {
    // Add a small delay to ensure the app state is stable
    std::thread::sleep(std::time::Duration::from_millis(10));

    let pid = get_frontmost_app_pid().ok_or("Could not get frontmost app")?;

    log::info!("Querying clickable elements for PID {}", pid);

    let app_element = unsafe {
        let ptr = AXUIElementCreateApplication(pid);
        if ptr.is_null() {
            return Err("Could not create AX element for app".to_string());
        }
        CFHandle(ptr)
    };

    let mut raw_elements: Vec<(f64, f64, f64, f64, String, String, AXElementHandle)> = Vec::new();

    // Start from focused window if available, otherwise from app
    let start_element = app_element
        .get_attribute("AXFocusedWindow")
        .and_then(|w| {
            let ptr = w.as_ptr();
            if ptr.is_null() {
                return None;
            }
            std::mem::forget(w);
            Some(CFHandle(unsafe { core_foundation::base::CFRetain(ptr) }))
        })
        .unwrap_or_else(|| {
            // Retain the app element for traversal
            CFHandle(unsafe { core_foundation::base::CFRetain(app_element.as_ptr()) })
        });

    collect_elements(&start_element, &mut raw_elements, 0);

    log::info!("Found {} raw clickable elements", raw_elements.len());

    // Generate hints
    let hints = generate_hints(raw_elements.len(), super::hints::DEFAULT_HINT_CHARS);

    // Convert to internal elements with hints
    let elements: Vec<ClickableElementInternal> = raw_elements
        .into_iter()
        .enumerate()
        .map(|(i, (x, y, w, h, role, title, ax_handle))| {
            ClickableElementInternal::new(
                i,
                hints.get(i).cloned().unwrap_or_else(|| i.to_string()),
                x,
                y,
                w,
                h,
                role,
                title,
                ax_handle,
            )
        })
        .collect();

    Ok(elements)
}

/// Perform a click action on an element
pub fn perform_click(element: &AXElementHandle) -> Result<(), String> {
    // First try AXPress
    let action = CFString::new("AXPress");
    let result = unsafe { AXUIElementPerformAction(element.as_ptr(), action.as_CFTypeRef()) };

    if result == 0 {
        log::info!("Successfully performed AXPress action");
        return Ok(());
    }

    log::warn!("AXPress failed with code {}, trying mouse click", result);

    // Fall back to simulated mouse click
    perform_mouse_click(element)
}

/// Perform a simulated mouse click at the element's position
fn perform_mouse_click(element: &AXElementHandle) -> Result<(), String> {
    use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use core_graphics::geometry::CGPoint;

    // Get element position
    let position_attr = CFString::new("AXPosition");
    let mut position_value: CFTypeRef = std::ptr::null();

    let result = unsafe {
        AXUIElementCopyAttributeValue(
            element.as_ptr(),
            position_attr.as_CFTypeRef(),
            &mut position_value,
        )
    };

    if result != 0 || position_value.is_null() {
        return Err("Could not get element position".to_string());
    }

    let position_handle = CFHandle(position_value);
    let (x, y) = position_handle
        .extract_point()
        .ok_or("Could not extract position point")?;

    // Get element size to click in center
    let size_attr = CFString::new("AXSize");
    let mut size_value: CFTypeRef = std::ptr::null();

    let result = unsafe {
        AXUIElementCopyAttributeValue(element.as_ptr(), size_attr.as_CFTypeRef(), &mut size_value)
    };

    let (center_x, center_y) = if result == 0 && !size_value.is_null() {
        let size_handle = CFHandle(size_value);
        if let Some((w, h)) = size_handle.extract_size() {
            (x + w / 2.0, y + h / 2.0)
        } else {
            (x, y)
        }
    } else {
        (x, y)
    };

    log::info!("Performing mouse click at ({}, {})", center_x, center_y);

    // Create and post mouse events
    let point = CGPoint::new(center_x, center_y);

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "Could not create event source")?;

    // Mouse down
    let mouse_down = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::LeftMouseDown,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| "Could not create mouse down event")?;

    mouse_down.post(CGEventTapLocation::HID);

    // Small delay
    std::thread::sleep(std::time::Duration::from_millis(10));

    // Mouse up
    let mouse_up = CGEvent::new_mouse_event(
        source,
        CGEventType::LeftMouseUp,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| "Could not create mouse up event")?;

    mouse_up.post(CGEventTapLocation::HID);

    log::info!("Mouse click completed");
    Ok(())
}

/// Perform a right-click (context menu) action on an element
pub fn perform_right_click(element: &AXElementHandle) -> Result<(), String> {
    let action = CFString::new("AXShowMenu");
    let result = unsafe { AXUIElementPerformAction(element.as_ptr(), action.as_CFTypeRef()) };

    if result == 0 {
        log::info!("Successfully performed AXShowMenu action");
        Ok(())
    } else {
        Err(format!("AXShowMenu failed with error code: {}", result))
    }
}
