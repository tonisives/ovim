//! RAII wrapper for Core Foundation types

use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::string::CFString;

use super::bindings::{kAXValueCGPointType, kAXValueCGSizeType, AXUIElementCopyAttributeValue, AXValueGetValue};

/// RAII wrapper for CFTypeRef
pub struct CFHandle(pub CFTypeRef);

impl CFHandle {
    pub fn get_attribute(&self, attr_name: &str) -> Option<CFHandle> {
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

    pub fn get_string_attribute(&self, attr_name: &str) -> Option<String> {
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

    pub fn extract_point(&self) -> Option<(f64, f64)> {
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

    pub fn extract_size(&self) -> Option<(f64, f64)> {
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
