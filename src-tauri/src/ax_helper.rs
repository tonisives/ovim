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

use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::string::CFString;
use serde::{Deserialize, Serialize};
use std::env;

#[link(name = "AppKit", kind = "framework")]
extern "C" {}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
    fn AXUIElementCreateSystemWide() -> CFTypeRef;
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

/// Output from the helper, including metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
struct HelperOutput {
    elements: Vec<RawElement>,
    /// True if elements were collected from a sheet/dialog (modal UI)
    is_modal: bool,
}

/// RAII wrapper for CFTypeRef
struct CFHandle(CFTypeRef);

impl CFHandle {
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

        // Check if this is actually a CFString before trying to convert
        // Some AX attributes return CFNumber or other types
        let type_id = unsafe { core_foundation::base::CFGetTypeID(handle.0) };
        let string_type_id = unsafe { core_foundation::string::CFStringGetTypeID() };

        if type_id != string_type_id {
            // Not a string - skip it
            return None;
        }

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

/// Inner element collection function - no try_objc wrappers
/// Must be called from within a try_objc block in query_elements
fn collect_elements_inner(
    element: &CFHandle,
    elements: &mut Vec<RawElement>,
    depth: usize,
    window_bounds: Option<WindowBounds>,
    inside_row: bool,  // Track if we're inside a row to skip cell/text children
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

    // If we're inside a row, skip cell contents (text, images, cells)
    // These are redundant - clicking the row is sufficient
    let skip_row_children = inside_row && matches!(role.as_str(),
        "AXCell" | "AXStaticText" | "AXImage"
    );

    // Only check AXActions for roles that commonly have them to avoid crashes
    let check_actions = matches!(role.as_str(),
        "AXButton" | "AXLink" | "AXMenuItem" | "AXMenuButton" |
        "AXCheckBox" | "AXRadioButton" | "AXPopUpButton" |
        "AXDisclosureTriangle" | "AXToolbarButton" |
        "AXStaticText" | "AXImage" | "AXCell" | "AXRow"
    );

    let is_clickable = !skip_as_clickable && !skip_row_children &&
        (is_clickable_role(&role) || (check_actions && has_press_action(element)));

    // Track if this element is a row (for children)
    let is_row = role == "AXRow";

    if is_clickable && is_visible(element) {
        if let (Some(pos), Some(size)) = (
            element.get_attribute("AXPosition").and_then(|p| p.extract_point()),
            element.get_attribute("AXSize").and_then(|s| s.extract_size()),
        ) {
            // Filter out elements outside window bounds
            let in_bounds = window_bounds
                .map(|bounds| bounds.contains(pos.0, pos.1, size.0, size.1))
                .unwrap_or(true);

            if in_bounds {
                // For rows, try to get a meaningful title from children
                let title = if is_row {
                    get_row_title(element).unwrap_or_default()
                } else {
                    element
                        .get_string_attribute("AXTitle")
                        .or_else(|| element.get_string_attribute("AXDescription"))
                        .or_else(|| element.get_string_attribute("AXValue"))
                        .or_else(|| element.get_string_attribute("AXLabel"))
                        .or_else(|| element.get_string_attribute("AXHelp"))
                        .unwrap_or_default()
                };

                elements.push(RawElement {
                    x: pos.0,
                    y: pos.1,
                    width: size.0,
                    height: size.1,
                    role: role.clone(),
                    title,
                });
            }
        }
    }

    // Don't recurse into row children - we already added the row
    if is_row {
        return;
    }

    // Recurse into children
    let children_attr = CFString::new("AXChildren");
    let mut children_value: CFTypeRef = std::ptr::null();

    let result = unsafe {
        AXUIElementCopyAttributeValue(element.0, children_attr.as_CFTypeRef(), &mut children_value)
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
        collect_elements_inner(&child, elements, depth + 1, window_bounds, inside_row || is_row);
    }
}

/// Get a title for a row by looking at its first text child
fn get_row_title(row: &CFHandle) -> Option<String> {
    let children_attr = CFString::new("AXChildren");
    let mut children_value: CFTypeRef = std::ptr::null();

    let result = unsafe {
        AXUIElementCopyAttributeValue(row.0, children_attr.as_CFTypeRef(), &mut children_value)
    };

    if result != 0 || children_value.is_null() {
        return None;
    }

    let _children_handle = CFHandle(children_value);
    let count = unsafe { core_foundation::array::CFArrayGetCount(children_value as _) };

    // Look through children (cells) for text
    for i in 0..count.min(10) {
        let child_ptr = unsafe {
            core_foundation::array::CFArrayGetValueAtIndex(children_value as _, i)
        };
        if child_ptr.is_null() {
            continue;
        }

        unsafe { core_foundation::base::CFRetain(child_ptr) };
        let child = CFHandle(child_ptr);

        // Check if this is a cell
        let child_role = child.get_string_attribute("AXRole").unwrap_or_default();
        if child_role == "AXCell" {
            // Look for text in the cell
            if let Some(title) = find_text_in_element(&child) {
                if !title.is_empty() {
                    return Some(title);
                }
            }
        } else if child_role == "AXStaticText" {
            if let Some(title) = child.get_string_attribute("AXValue")
                .or_else(|| child.get_string_attribute("AXTitle")) {
                if !title.is_empty() {
                    return Some(title);
                }
            }
        }
    }

    None
}

/// Find the first text value in an element or its children
fn find_text_in_element(element: &CFHandle) -> Option<String> {
    // Try direct attributes first
    if let Some(title) = element.get_string_attribute("AXValue")
        .or_else(|| element.get_string_attribute("AXTitle"))
        .or_else(|| element.get_string_attribute("AXDescription")) {
        if !title.is_empty() {
            return Some(title);
        }
    }

    // Check children
    let children_attr = CFString::new("AXChildren");
    let mut children_value: CFTypeRef = std::ptr::null();

    let result = unsafe {
        AXUIElementCopyAttributeValue(element.0, children_attr.as_CFTypeRef(), &mut children_value)
    };

    if result != 0 || children_value.is_null() {
        return None;
    }

    let _children_handle = CFHandle(children_value);
    let count = unsafe { core_foundation::array::CFArrayGetCount(children_value as _) };

    for i in 0..count.min(5) {
        let child_ptr = unsafe {
            core_foundation::array::CFArrayGetValueAtIndex(children_value as _, i)
        };
        if child_ptr.is_null() {
            continue;
        }

        unsafe { core_foundation::base::CFRetain(child_ptr) };
        let child = CFHandle(child_ptr);

        let role = child.get_string_attribute("AXRole").unwrap_or_default();
        if role == "AXStaticText" {
            if let Some(title) = child.get_string_attribute("AXValue")
                .or_else(|| child.get_string_attribute("AXTitle")) {
                if !title.is_empty() {
                    return Some(title);
                }
            }
        }
    }

    None
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

/// Check if the focused element is a menu item and collect menu items
/// This handles popup/context menus that appear outside the normal window hierarchy
fn collect_menu_elements(elements: &mut Vec<RawElement>, pid: i32) -> bool {
    let system_wide = unsafe {
        let ptr = AXUIElementCreateSystemWide();
        if ptr.is_null() {
            return false;
        }
        CFHandle(ptr)
    };

    // Get the focused UI element
    let focused = system_wide.get_attribute("AXFocusedUIElement");

    if let Some(ref focused) = focused {
        // Check if the focused element is related to a menu
        let role = focused.get_string_attribute("AXRole").unwrap_or_default();

        // If focused element is a menu item, find the parent menu and collect all items
        if role == "AXMenuItem" {
            // Get the parent menu
            if let Some(parent) = focused.get_attribute("AXParent") {
                let parent_role = parent.get_string_attribute("AXRole").unwrap_or_default();
                if parent_role == "AXMenu" {
                    // Collect all menu items from this menu (no window bounds filtering)
                    collect_menu_items(&parent, elements);
                    return !elements.is_empty();
                }
            }
        }

        // Check if focused element IS a menu
        if role == "AXMenu" {
            collect_menu_items(&focused, elements);
            return !elements.is_empty();
        }

        // Check if the focused element has a visible menu child (e.g., popup button with open menu)
        if let Some(menu) = focused.get_attribute("AXChildren") {
            let menu_ptr = menu.0;
            let count = unsafe { core_foundation::array::CFArrayGetCount(menu_ptr as _) };

            for i in 0..count.min(10) {
                let child_ptr = unsafe {
                    core_foundation::array::CFArrayGetValueAtIndex(menu_ptr as _, i)
                };
                if child_ptr.is_null() {
                    continue;
                }

                unsafe { core_foundation::base::CFRetain(child_ptr) };
                let child = CFHandle(child_ptr);

                let child_role = child.get_string_attribute("AXRole").unwrap_or_default();
                if child_role == "AXMenu" {
                    collect_menu_items(&child, elements);
                    if !elements.is_empty() {
                        return true;
                    }
                }
            }
        }
    }

    // If focus-based detection didn't find a menu, check the app's popup menus
    // Context menus from right-click appear here even when focus hasn't changed
    let app_element = unsafe {
        let ptr = AXUIElementCreateApplication(pid);
        if ptr.is_null() {
            return false;
        }
        CFHandle(ptr)
    };

    // Check for AXExtrasMenuBar (some apps put context menus here)
    // and also recursively look for visible AXMenu elements
    if let Some(extras) = app_element.get_attribute("AXExtrasMenuBar") {
        let role = extras.get_string_attribute("AXRole").unwrap_or_default();
        if role == "AXMenu" {
            collect_menu_items(&extras, elements);
            if !elements.is_empty() {
                return true;
            }
        }
    }

    // Look for any visible popup menus in the app's children
    // Context menus are often added as direct children of the app element
    if let Some(children) = app_element.get_attribute("AXChildren") {
        let children_ptr = children.0;
        let count = unsafe { core_foundation::array::CFArrayGetCount(children_ptr as _) };

        for i in 0..count.min(20) {
            let child_ptr = unsafe {
                core_foundation::array::CFArrayGetValueAtIndex(children_ptr as _, i)
            };
            if child_ptr.is_null() {
                continue;
            }

            unsafe { core_foundation::base::CFRetain(child_ptr) };
            let child = CFHandle(child_ptr);

            let child_role = child.get_string_attribute("AXRole").unwrap_or_default();

            // Check if this is a menu
            if child_role == "AXMenu" {
                collect_menu_items(&child, elements);
                if !elements.is_empty() {
                    return true;
                }
            }

            // Check if this is a window - look for menus inside any window
            // Context menus can appear in various window types
            if child_role == "AXWindow" {
                if find_menus_in_element(&child, elements) {
                    return true;
                }
            }
        }
    }

    // Also check AXWindows array directly (more reliable way to get all windows)
    if let Some(windows) = app_element.get_attribute("AXWindows") {
        let windows_ptr = windows.0;
        let count = unsafe { core_foundation::array::CFArrayGetCount(windows_ptr as _) };

        for i in 0..count.min(10) {
            let window_ptr = unsafe {
                core_foundation::array::CFArrayGetValueAtIndex(windows_ptr as _, i)
            };
            if window_ptr.is_null() {
                continue;
            }

            unsafe { core_foundation::base::CFRetain(window_ptr) };
            let window = CFHandle(window_ptr);

            if find_menus_in_element(&window, elements) {
                return true;
            }
        }
    }

    false
}

/// Recursively search for AXMenu elements within an element (limited depth)
fn find_menus_in_element(element: &CFHandle, elements: &mut Vec<RawElement>) -> bool {
    find_menus_recursive(element, elements, 0)
}

fn find_menus_recursive(element: &CFHandle, elements: &mut Vec<RawElement>, depth: usize) -> bool {
    // Limit recursion depth to avoid performance issues
    if depth > 3 {
        return false;
    }

    let children_attr = CFString::new("AXChildren");
    let mut children_value: CFTypeRef = std::ptr::null();

    let result = unsafe {
        AXUIElementCopyAttributeValue(element.0, children_attr.as_CFTypeRef(), &mut children_value)
    };

    if result != 0 || children_value.is_null() {
        return false;
    }

    let _children_handle = CFHandle(children_value);
    let count = unsafe { core_foundation::array::CFArrayGetCount(children_value as _) };

    for i in 0..count.min(30) {
        let child_ptr = unsafe {
            core_foundation::array::CFArrayGetValueAtIndex(children_value as _, i)
        };
        if child_ptr.is_null() {
            continue;
        }

        unsafe { core_foundation::base::CFRetain(child_ptr) };
        let child = CFHandle(child_ptr);

        let role = child.get_string_attribute("AXRole").unwrap_or_default();

        // Found a menu - collect its items
        if role == "AXMenu" {
            collect_menu_items(&child, elements);
            if !elements.is_empty() {
                return true;
            }
        }

        // Recurse into other elements that might contain menus
        // Skip certain roles that won't contain popup menus
        if !matches!(role.as_str(), "AXStaticText" | "AXImage" | "AXTextField" | "AXTextArea") {
            if find_menus_recursive(&child, elements, depth + 1) {
                return true;
            }
        }
    }

    false
}

/// Collect menu items from a menu element
fn collect_menu_items(menu: &CFHandle, elements: &mut Vec<RawElement>) {
    let children_attr = CFString::new("AXChildren");
    let mut children_value: CFTypeRef = std::ptr::null();

    let result = unsafe {
        AXUIElementCopyAttributeValue(menu.0, children_attr.as_CFTypeRef(), &mut children_value)
    };

    if result != 0 || children_value.is_null() {
        return;
    }

    let _children_handle = CFHandle(children_value);
    let count = unsafe { core_foundation::array::CFArrayGetCount(children_value as _) };

    for i in 0..count.min(50) {
        let child_ptr = unsafe {
            core_foundation::array::CFArrayGetValueAtIndex(children_value as _, i)
        };
        if child_ptr.is_null() {
            continue;
        }

        unsafe { core_foundation::base::CFRetain(child_ptr) };
        let child = CFHandle(child_ptr);

        let role = child.get_string_attribute("AXRole").unwrap_or_default();

        // Only collect actual menu items (skip separators, etc.)
        if role == "AXMenuItem" {
            if let (Some(pos), Some(size)) = (
                child.get_attribute("AXPosition").and_then(|p| p.extract_point()),
                child.get_attribute("AXSize").and_then(|s| s.extract_size()),
            ) {
                // Skip zero-sized elements
                if size.0 > 0.0 && size.1 > 0.0 {
                    let title = child
                        .get_string_attribute("AXTitle")
                        .or_else(|| child.get_string_attribute("AXDescription"))
                        .unwrap_or_default();

                    // Skip separator-like items (empty title, very small height)
                    if !title.is_empty() || size.1 > 10.0 {
                        elements.push(RawElement {
                            x: pos.0,
                            y: pos.1,
                            width: size.0,
                            height: size.1,
                            role: role.clone(),
                            title,
                        });
                    }
                }
            }
        }

        // Recurse into submenus
        if role == "AXMenu" {
            collect_menu_items(&child, elements);
        }
    }
}

/// Inner function that does all the work without try_objc wrappers
/// This is safe to call from within a try_objc block
fn query_elements_inner(pid: i32) -> Result<HelperOutput, String> {
    let mut elements: Vec<RawElement> = Vec::new();

    // First, check if there's an open menu (popup menu, context menu, etc.)
    // These take priority as they're the most likely target when visible
    if collect_menu_elements(&mut elements, pid) {
        return Ok(HelperOutput { elements, is_modal: true });
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
    let focused_window = app_element
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
            std::mem::forget(w);
            unsafe { core_foundation::base::CFRetain(ptr) };
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
                    unsafe { core_foundation::base::CFRetain(sheet_ptr) };
                    let sheet = CFHandle(sheet_ptr);
                    let sheet_bounds = get_window_bounds(&sheet);
                    // Collect elements from the sheet instead of the window
                    collect_elements_inner(&sheet, &mut elements, 0, sheet_bounds, false);
                    return Ok(HelperOutput { elements, is_modal: true });
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
                    unsafe { core_foundation::base::CFRetain(dialog_ptr) };
                    let dialog = CFHandle(dialog_ptr);
                    let dialog_bounds = get_window_bounds(&dialog);
                    collect_elements_inner(&dialog, &mut elements, 0, dialog_bounds, false);
                    return Ok(HelperOutput { elements, is_modal: true });
                }
            }
        }
    }

    // Also check app-level for sheets/dialogs (some apps report them at the app level)
    if let Some(sheets) = app_element.get_attribute("AXSheets") {
        let sheets_ptr = sheets.0;
        let count = unsafe { core_foundation::array::CFArrayGetCount(sheets_ptr as _) };
        if count > 0 {
            let sheet_ptr = unsafe {
                core_foundation::array::CFArrayGetValueAtIndex(sheets_ptr as _, 0)
            };
            if !sheet_ptr.is_null() {
                unsafe { core_foundation::base::CFRetain(sheet_ptr) };
                let sheet = CFHandle(sheet_ptr);
                let sheet_bounds = get_window_bounds(&sheet);
                collect_elements_inner(&sheet, &mut elements, 0, sheet_bounds, false);
                return Ok(HelperOutput { elements, is_modal: true });
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
            let app_ptr = app_element.0;
            unsafe { core_foundation::base::CFRetain(app_ptr) };
            (CFHandle(app_ptr), None)
        });

    collect_elements_inner(&start_element, &mut elements, 0, window_bounds, false);

    Ok(HelperOutput { elements, is_modal: false })
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
        Ok(output) => {
            let json = serde_json::to_string(&output).unwrap_or_else(|_|
                r#"{"elements":[],"is_modal":false}"#.to_string()
            );
            println!("{}", json);
        }
        Err(e) => {
            eprintln!("{{\"error\": \"{}\"}}", e);
            std::process::exit(1);
        }
    }
}
