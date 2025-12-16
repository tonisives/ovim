use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::string::CFString;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub struct SelectionInfo {
    pub char_count: usize,
    pub line_count: usize,
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateSystemWide() -> CFTypeRef;
    fn AXUIElementCopyAttributeValue(
        element: CFTypeRef,
        attribute: CFTypeRef,
        value: *mut CFTypeRef,
    ) -> i32;
}

/// Get selection info from the focused application using Accessibility APIs
pub fn get_selection_info() -> SelectionInfo {
    match get_selected_text() {
        Some(text) if !text.is_empty() => {
            let char_count = text.chars().count();
            let line_count = text.lines().count().max(1);
            SelectionInfo {
                char_count,
                line_count,
            }
        }
        _ => SelectionInfo::default(),
    }
}

/// Get the selected text from the currently focused application
fn get_selected_text() -> Option<String> {
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

        // Get selected text
        let selected_text_attr = CFString::new("AXSelectedText");
        let mut selected_text: CFTypeRef = std::ptr::null();
        let result = AXUIElementCopyAttributeValue(
            focused_element,
            selected_text_attr.as_CFTypeRef(),
            &mut selected_text,
        );

        CFRelease(focused_element);
        CFRelease(focused_app);
        CFRelease(system_wide);

        if result != 0 || selected_text.is_null() {
            return None;
        }

        // Convert CFString to Rust String
        let cf_string: CFString = CFString::wrap_under_create_rule(selected_text as _);
        Some(cf_string.to_string())
    }
}
