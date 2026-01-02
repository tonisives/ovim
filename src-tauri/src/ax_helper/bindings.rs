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

pub const K_AX_VALUE_CG_POINT_TYPE: i32 = 1;
pub const K_AX_VALUE_CG_SIZE_TYPE: i32 = 2;

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

/// Default depth limit for traversal
pub const DEFAULT_MAX_DEPTH: usize = 10;
pub const DEFAULT_MAX_ELEMENTS: usize = 500;

use std::sync::atomic::{AtomicUsize, Ordering};

/// Runtime-configurable limits
pub static MAX_DEPTH: AtomicUsize = AtomicUsize::new(DEFAULT_MAX_DEPTH);
pub static MAX_ELEMENTS: AtomicUsize = AtomicUsize::new(DEFAULT_MAX_ELEMENTS);

pub fn set_limits(max_depth: usize, max_elements: usize) {
    MAX_DEPTH.store(max_depth, Ordering::Relaxed);
    MAX_ELEMENTS.store(max_elements, Ordering::Relaxed);
}

pub fn get_max_depth() -> usize {
    MAX_DEPTH.load(Ordering::Relaxed)
}

pub fn get_max_elements() -> usize {
    MAX_ELEMENTS.load(Ordering::Relaxed)
}
