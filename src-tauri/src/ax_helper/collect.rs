//! Element collection and traversal logic

use core_foundation::base::{CFRetain, CFTypeRef, TCFType};
use core_foundation::string::CFString;

use super::bindings::{AXUIElementCopyAttributeValue, get_max_depth, get_max_elements};
use super::cf_handle::CFHandle;
use super::element::{has_press_action, is_clickable_role, is_visible};
use super::types::{RawElement, WindowBounds};

/// Inner element collection function
pub fn collect_elements_inner(
    element: &CFHandle,
    elements: &mut Vec<RawElement>,
    depth: usize,
    window_bounds: Option<WindowBounds>,
    inside_row: bool,
) {
    if depth > get_max_depth() || elements.len() >= get_max_elements() {
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
    let skip_row_children =
        inside_row && matches!(role.as_str(), "AXCell" | "AXStaticText" | "AXImage");

    // Only check AXActions for roles that commonly have them to avoid crashes
    // Note: AXStaticText, AXImage, AXHeading are only clickable if they have AXPress action
    let check_actions = matches!(
        role.as_str(),
        "AXButton"
            | "AXLink"
            | "AXMenuItem"
            | "AXMenuButton"
            | "AXCheckBox"
            | "AXRadioButton"
            | "AXPopUpButton"
            | "AXDisclosureTriangle"
            | "AXToolbarButton"
            | "AXStaticText"
            | "AXImage"
            | "AXHeading"
            | "AXCell"
            | "AXRow"
    );

    let is_clickable = !skip_as_clickable
        && !skip_row_children
        && (is_clickable_role(&role) || (check_actions && has_press_action(element)));

    // Track if this element is a row (for children)
    let is_row = role == "AXRow";

    if is_clickable && is_visible(element) {
        if let (Some(pos), Some(size)) = (
            element
                .get_attribute("AXPosition")
                .and_then(|p| p.extract_point()),
            element
                .get_attribute("AXSize")
                .and_then(|s| s.extract_size()),
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
        if elements.len() >= get_max_elements() {
            break;
        }

        let child_ptr = unsafe {
            core_foundation::array::CFArrayGetValueAtIndex(children_value as _, i as isize)
        };

        if child_ptr.is_null() {
            continue;
        }

        unsafe { CFRetain(child_ptr) };
        let child = CFHandle(child_ptr);
        collect_elements_inner(&child, elements, depth + 1, window_bounds, inside_row || is_row);
    }
}

/// Get a title for a row by looking at its first text child
pub fn get_row_title(row: &CFHandle) -> Option<String> {
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
        let child_ptr =
            unsafe { core_foundation::array::CFArrayGetValueAtIndex(children_value as _, i) };
        if child_ptr.is_null() {
            continue;
        }

        unsafe { CFRetain(child_ptr) };
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
            if let Some(title) = child
                .get_string_attribute("AXValue")
                .or_else(|| child.get_string_attribute("AXTitle"))
            {
                if !title.is_empty() {
                    return Some(title);
                }
            }
        }
    }

    None
}

/// Find the first text value in an element or its children
pub fn find_text_in_element(element: &CFHandle) -> Option<String> {
    // Try direct attributes first
    if let Some(title) = element
        .get_string_attribute("AXValue")
        .or_else(|| element.get_string_attribute("AXTitle"))
        .or_else(|| element.get_string_attribute("AXDescription"))
    {
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
        let child_ptr =
            unsafe { core_foundation::array::CFArrayGetValueAtIndex(children_value as _, i) };
        if child_ptr.is_null() {
            continue;
        }

        unsafe { CFRetain(child_ptr) };
        let child = CFHandle(child_ptr);

        let role = child.get_string_attribute("AXRole").unwrap_or_default();
        if role == "AXStaticText" {
            if let Some(title) = child
                .get_string_attribute("AXValue")
                .or_else(|| child.get_string_attribute("AXTitle"))
            {
                if !title.is_empty() {
                    return Some(title);
                }
            }
        }
    }

    None
}
