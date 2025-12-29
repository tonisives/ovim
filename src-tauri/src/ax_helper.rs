//! Accessibility helper subprocess
//!
//! This binary queries the accessibility tree and outputs element data as JSON.
//! It's run as a subprocess so that if the accessibility API throws an
//! Objective-C exception, it crashes this process instead of the main app.
//!
//! We use objc_exception to catch ObjC exceptions and recover gracefully.

#![allow(unexpected_cfgs)]

use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::string::CFString;
use objc_exception::r#try as try_objc;
use serde::{Deserialize, Serialize};
use std::env;

#[link(name = "AppKit", kind = "framework")]
extern "C" {}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
    fn AXUIElementCopyAttributeValue(
        element: CFTypeRef,
        attribute: CFTypeRef,
        value: *mut CFTypeRef,
    ) -> i32;
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
    "AXMenuButton",
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
    "AXHeading",
];

// Depth limit for traversal
const MAX_DEPTH: usize = 12;
const MAX_ELEMENTS: usize = 300;

/// Window bounds for filtering elements
#[derive(Debug, Clone, Copy)]
struct WindowBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl WindowBounds {
    fn contains(&self, elem_x: f64, elem_y: f64, elem_w: f64, elem_h: f64) -> bool {
        // Check if element is at least partially within window bounds
        // Element must have some overlap with the window
        let elem_right = elem_x + elem_w;
        let elem_bottom = elem_y + elem_h;
        let win_right = self.x + self.width;
        let win_bottom = self.y + self.height;

        // Element is visible if it overlaps with window
        elem_x < win_right && elem_right > self.x &&
        elem_y < win_bottom && elem_bottom > self.y
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawElement {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    role: String,
    title: String,
}

/// RAII wrapper for CFTypeRef
struct CFHandle(CFTypeRef);

impl CFHandle {
    fn get_attribute(&self, attr_name: &str) -> Option<CFHandle> {
        let attr = CFString::new(attr_name);
        let mut value: CFTypeRef = std::ptr::null();

        // Wrap in ObjC exception handler to catch crashes from unstable UI state
        let element_ptr = self.0;
        let attr_ptr = attr.as_CFTypeRef();
        let value_ptr = &mut value as *mut CFTypeRef;

        let result = unsafe {
            try_objc(|| {
                AXUIElementCopyAttributeValue(element_ptr, attr_ptr, value_ptr)
            })
        };

        match result {
            Ok(res) if res == 0 && !value.is_null() => Some(CFHandle(value)),
            _ => None,
        }
    }

    fn get_string_attribute(&self, attr_name: &str) -> Option<String> {
        let handle = self.get_attribute(attr_name)?;
        let cf_string: CFString = unsafe { CFString::wrap_under_create_rule(handle.0 as _) };
        let result = cf_string.to_string();
        std::mem::forget(handle);
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
}

impl Drop for CFHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) };
        }
    }
}

fn is_clickable_role(role: &str) -> bool {
    CLICKABLE_ROLES.iter().any(|r| *r == role)
}

fn has_press_action(element: &CFHandle) -> bool {
    let actions_handle = match element.get_attribute("AXActions") {
        Some(h) => h,
        None => return false,
    };

    let actions_ptr = actions_handle.0;
    if actions_ptr.is_null() {
        return false;
    }

    let count = unsafe { core_foundation::array::CFArrayGetCount(actions_ptr as _) };
    if count <= 0 {
        return false;
    }

    for i in 0..count.min(20) {
        let action_ptr = unsafe {
            core_foundation::array::CFArrayGetValueAtIndex(actions_ptr as _, i)
        };

        if action_ptr.is_null() {
            continue;
        }

        let action_cfstring: CFString = unsafe {
            CFString::wrap_under_get_rule(action_ptr as _)
        };
        let action_str = action_cfstring.to_string();

        if action_str == "AXPress" || action_str == "AXShowMenu" {
            return true;
        }
    }

    false
}

fn is_visible(element: &CFHandle) -> bool {
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

    w > 0.0 && h > 0.0 && x >= -10000.0 && y >= -10000.0
}

fn collect_elements(
    element: &CFHandle,
    elements: &mut Vec<RawElement>,
    depth: usize,
    window_bounds: Option<WindowBounds>,
) {
    if depth > MAX_DEPTH || elements.len() >= MAX_ELEMENTS {
        return;
    }

    let role = element.get_string_attribute("AXRole").unwrap_or_default();

    // Skip elements with empty/unknown roles entirely
    if role.is_empty() || role == "AXUnknown" {
        return;
    }

    // Skip these roles as clickable but still traverse their children
    let skip_as_clickable = role == "AXMenu"
        || role == "AXMenuBar"
        || role == "AXBusyIndicator"
        || role == "AXProgressIndicator"
        || role == "AXValueIndicator"
        || role == "AXScrollBar"
        || role == "AXOutline"
        || role == "AXScrollArea"
        || role == "AXSplitGroup"
        || role == "AXGroup";

    // Only check AXActions for roles that commonly have them to avoid crashes
    let check_actions = matches!(role.as_str(),
        "AXButton" | "AXLink" | "AXMenuItem" | "AXMenuButton" |
        "AXCheckBox" | "AXRadioButton" | "AXPopUpButton" |
        "AXDisclosureTriangle" | "AXToolbarButton" |
        "AXStaticText" | "AXImage" | "AXCell" | "AXRow"
    );

    let is_clickable = !skip_as_clickable && (is_clickable_role(&role) || (check_actions && has_press_action(element)));

    if is_clickable && is_visible(element) {
        if let (Some(pos), Some(size)) = (
            element.get_attribute("AXPosition").and_then(|p| p.extract_point()),
            element.get_attribute("AXSize").and_then(|s| s.extract_size()),
        ) {
            // Filter out elements outside window bounds
            if let Some(bounds) = window_bounds {
                if !bounds.contains(pos.0, pos.1, size.0, size.1) {
                    // Element is outside window - skip it but still recurse into children
                    // (children might be visible even if parent container extends outside)
                } else {
                    let title = element
                        .get_string_attribute("AXTitle")
                        .or_else(|| element.get_string_attribute("AXDescription"))
                        .or_else(|| element.get_string_attribute("AXValue"))
                        .or_else(|| element.get_string_attribute("AXLabel"))
                        .or_else(|| element.get_string_attribute("AXHelp"))
                        .unwrap_or_default();

                    elements.push(RawElement {
                        x: pos.0,
                        y: pos.1,
                        width: size.0,
                        height: size.1,
                        role,
                        title,
                    });
                }
            } else {
                let title = element
                    .get_string_attribute("AXTitle")
                    .or_else(|| element.get_string_attribute("AXDescription"))
                    .or_else(|| element.get_string_attribute("AXValue"))
                    .or_else(|| element.get_string_attribute("AXLabel"))
                    .or_else(|| element.get_string_attribute("AXHelp"))
                    .unwrap_or_default();

                elements.push(RawElement {
                    x: pos.0,
                    y: pos.1,
                    width: size.0,
                    height: size.1,
                    role,
                    title,
                });
            }
        }
    }

    // Recurse into children - wrapped in exception handler
    let children_attr = CFString::new("AXChildren");
    let mut children_value: CFTypeRef = std::ptr::null();

    let element_ptr = element.0;
    let attr_ptr = children_attr.as_CFTypeRef();
    let value_ptr = &mut children_value as *mut CFTypeRef;

    let result = unsafe {
        try_objc(|| {
            AXUIElementCopyAttributeValue(element_ptr, attr_ptr, value_ptr)
        })
    };

    let result = match result {
        Ok(r) => r,
        Err(_) => return, // ObjC exception caught, skip this element's children
    };

    if result != 0 || children_value.is_null() {
        return;
    }

    let _children_handle = CFHandle(children_value);

    let count = unsafe { core_foundation::array::CFArrayGetCount(children_value as _) };
    if count <= 0 {
        return;
    }

    let safe_count = (count as usize).min(100);

    for i in 0..safe_count {
        if elements.len() >= MAX_ELEMENTS {
            break;
        }

        let child_ptr = unsafe {
            core_foundation::array::CFArrayGetValueAtIndex(children_value as _, i as isize)
        };

        if child_ptr.is_null() {
            continue;
        }

        unsafe { core_foundation::base::CFRetain(child_ptr) };
        let child = CFHandle(child_ptr);
        collect_elements(&child, elements, depth + 1, window_bounds);
    }
}

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
    let pos = window.get_attribute("AXPosition").and_then(|p| p.extract_point())?;
    let size = window.get_attribute("AXSize").and_then(|s| s.extract_size())?;

    Some(WindowBounds {
        x: pos.0,
        y: pos.1,
        width: size.0,
        height: size.1,
    })
}

fn query_elements(pid: i32) -> Result<Vec<RawElement>, String> {
    let app_element = unsafe {
        let ptr = AXUIElementCreateApplication(pid);
        if ptr.is_null() {
            return Err("Could not create AX element for app".to_string());
        }
        CFHandle(ptr)
    };

    // First, test if we can safely access the app's role
    // This is a simple operation that should fail fast if the app is in a bad state
    let _role = app_element.get_string_attribute("AXRole");

    let mut elements: Vec<RawElement> = Vec::new();

    // Try to get focused window first, fall back to app element
    let (start_element, window_bounds) = app_element
        .get_attribute("AXFocusedWindow")
        .and_then(|w| {
            let ptr = w.0;
            if ptr.is_null() {
                return None;
            }
            // Test if we can access the window's role before using it
            let role_test = w.get_string_attribute("AXRole");
            if role_test.is_none() {
                return None;
            }
            // Get window bounds for filtering
            let bounds = get_window_bounds(&w);
            std::mem::forget(w);
            Some((CFHandle(unsafe { core_foundation::base::CFRetain(ptr) }), bounds))
        })
        .or_else(|| {
            // Try AXWindows array as fallback
            app_element.get_attribute("AXWindows").and_then(|windows| {
                let count = unsafe { core_foundation::array::CFArrayGetCount(windows.0 as _) };
                if count > 0 {
                    let first = unsafe {
                        core_foundation::array::CFArrayGetValueAtIndex(windows.0 as _, 0)
                    };
                    if !first.is_null() {
                        unsafe { core_foundation::base::CFRetain(first) };
                        let handle = CFHandle(first);
                        let bounds = get_window_bounds(&handle);
                        return Some((handle, bounds));
                    }
                }
                None
            })
        })
        .unwrap_or_else(|| {
            (CFHandle(unsafe { core_foundation::base::CFRetain(app_element.0) }), None)
        });

    collect_elements(&start_element, &mut elements, 0, window_bounds);

    Ok(elements)
}

fn main() {
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

    // Delay to let UI stabilize - accessibility API can crash during transitions
    // Longer delay helps avoid crashes from UI animations/transitions
    // System Settings especially needs more time to stabilize
    std::thread::sleep(std::time::Duration::from_millis(300));

    match query_elements(pid) {
        Ok(elements) => {
            let json = serde_json::to_string(&elements).unwrap_or_else(|_| "[]".to_string());
            println!("{}", json);
        }
        Err(e) => {
            eprintln!("{{\"error\": \"{}\"}}", e);
            std::process::exit(1);
        }
    }
}
