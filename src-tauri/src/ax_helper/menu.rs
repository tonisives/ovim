//! Menu detection and collection logic

use core_foundation::base::{CFRetain, CFTypeRef, TCFType};
use core_foundation::string::CFString;

use super::bindings::{
    AXUIElementCopyAttributeValue, AXUIElementCreateApplication, AXUIElementCreateSystemWide,
};
use super::cf_handle::CFHandle;
use super::types::RawElement;

/// Check if the focused element is a menu item and collect menu items
/// This handles popup/context menus that appear outside the normal window hierarchy
pub fn collect_menu_elements(elements: &mut Vec<RawElement>, pid: i32) -> bool {
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
            collect_menu_items(focused, elements);
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

                unsafe { CFRetain(child_ptr) };
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

            unsafe { CFRetain(child_ptr) };
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

            unsafe { CFRetain(window_ptr) };
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

        unsafe { CFRetain(child_ptr) };
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
        if !matches!(
            role.as_str(),
            "AXStaticText" | "AXImage" | "AXTextField" | "AXTextArea"
        ) {
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

        unsafe { CFRetain(child_ptr) };
        let child = CFHandle(child_ptr);

        let role = child.get_string_attribute("AXRole").unwrap_or_default();

        // Only collect actual menu items (skip separators, etc.)
        if role == "AXMenuItem" {
            if let (Some(pos), Some(size)) = (
                child
                    .get_attribute("AXPosition")
                    .and_then(|p| p.extract_point()),
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
