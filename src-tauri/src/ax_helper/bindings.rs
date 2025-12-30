//! FFI bindings for macOS Accessibility and Core Foundation frameworks

use core_foundation::base::CFTypeRef;

#[link(name = "AppKit", kind = "framework")]
extern "C" {}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    pub fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
    pub fn AXUIElementCreateSystemWide() -> CFTypeRef;
    pub fn AXUIElementCopyAttributeValue(
        element: CFTypeRef,
        attribute: CFTypeRef,
        value: *mut CFTypeRef,
    ) -> i32;
}

#[allow(non_upper_case_globals)]
pub const kAXValueCGPointType: i32 = 1;
#[allow(non_upper_case_globals)]
pub const kAXValueCGSizeType: i32 = 2;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    pub fn AXValueGetValue(
        value: CFTypeRef,
        the_type: i32,
        value_ptr: *mut std::ffi::c_void,
    ) -> bool;
}

/// Roles that are considered clickable
pub const CLICKABLE_ROLES: &[&str] = &[
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

/// Depth limit for traversal
pub const MAX_DEPTH: usize = 12;
pub const MAX_ELEMENTS: usize = 300;
