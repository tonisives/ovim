//! Element filtering and visibility logic

use core_foundation::string::CFString;

use core_foundation::base::TCFType;
use super::bindings::CLICKABLE_ROLES;
use super::cf_handle::CFHandle;

pub fn is_clickable_role(role: &str) -> bool {
    CLICKABLE_ROLES.iter().any(|r| *r == role)
}

pub fn has_press_action(element: &CFHandle) -> bool {
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
        let action_ptr =
            unsafe { core_foundation::array::CFArrayGetValueAtIndex(actions_ptr as _, i) };

        if action_ptr.is_null() {
            continue;
        }

        let action_cfstring: CFString = unsafe { CFString::wrap_under_get_rule(action_ptr as _) };
        let action_str = action_cfstring.to_string();

        if action_str == "AXPress" || action_str == "AXShowMenu" {
            return true;
        }
    }

    false
}

pub fn is_visible(element: &CFHandle) -> bool {
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

    // Require minimum size of 4px in both dimensions to filter out
    // invisible/collapsed elements (tooltips, hidden elements with 1px size)
    w >= 4.0 && h >= 4.0 && x >= -10000.0 && y >= -10000.0
}
