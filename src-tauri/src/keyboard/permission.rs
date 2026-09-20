// ApplicationServices framework bindings for macOS privacy permissions.
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrustedWithOptions(options: core_foundation::dictionary::CFDictionaryRef)
        -> bool;
    fn CGPreflightListenEventAccess() -> bool;
}

/// Check whether macOS allows ovim to use the Accessibility API.
pub fn check_accessibility_permission() -> bool {
    unsafe { AXIsProcessTrustedWithOptions(std::ptr::null()) }
}

/// Check whether macOS allows ovim to listen to keyboard input.
pub fn check_input_monitoring_permission() -> bool {
    unsafe { CGPreflightListenEventAccess() }
}
