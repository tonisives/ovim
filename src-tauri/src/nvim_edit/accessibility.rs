//! Accessibility APIs for getting text from focused UI elements

use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::string::CFString;

#[allow(non_upper_case_globals)]
const kAXValueCGPointType: i32 = 1;
#[allow(non_upper_case_globals)]
const kAXValueCGSizeType: i32 = 2;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateSystemWide() -> CFTypeRef;
    fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
    fn AXUIElementCopyAttributeValue(
        element: CFTypeRef,
        attribute: CFTypeRef,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXUIElementSetAttributeValue(
        element: CFTypeRef,
        attribute: CFTypeRef,
        value: CFTypeRef,
    ) -> i32;
    fn AXValueGetValue(
        value: CFTypeRef,
        the_type: i32,
        value_ptr: *mut std::ffi::c_void,
    ) -> bool;
    fn CFRetain(cf: CFTypeRef) -> CFTypeRef;
}

/// Wrapper for AXUIElementRef that can be sent across threads
/// The element is retained on creation and released on drop
pub struct AXElementHandle {
    element: CFTypeRef,
}

impl AXElementHandle {
    /// Create a new handle by retaining the element
    pub unsafe fn new(element: CFTypeRef) -> Option<Self> {
        if element.is_null() {
            return None;
        }
        CFRetain(element);
        Some(Self { element })
    }

    /// Get the raw element pointer
    pub fn as_ptr(&self) -> CFTypeRef {
        self.element
    }
}

impl Drop for AXElementHandle {
    fn drop(&mut self) {
        if !self.element.is_null() {
            unsafe { CFRelease(self.element) };
        }
    }
}

// Safety: AXUIElementRef is safe to send across threads
unsafe impl Send for AXElementHandle {}
unsafe impl Sync for AXElementHandle {}

impl std::fmt::Debug for AXElementHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AXElementHandle")
            .field("element", &self.element)
            .finish()
    }
}

impl Clone for AXElementHandle {
    fn clone(&self) -> Self {
        unsafe {
            if !self.element.is_null() {
                CFRetain(self.element);
            }
            Self { element: self.element }
        }
    }
}

/// RAII wrapper for CFTypeRef that automatically releases the reference when dropped.
struct CFHandle(CFTypeRef);

impl CFHandle {
    /// Create a new handle from a CFTypeRef. Returns None if the pointer is null.
    fn new(ptr: CFTypeRef) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            Some(Self(ptr))
        }
    }

    /// Get an attribute value from this element
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

    /// Extract a CGPoint from an AXValue
    fn extract_point(&self) -> Option<core_graphics::geometry::CGPoint> {
        let mut point = core_graphics::geometry::CGPoint::new(0.0, 0.0);
        let extracted = unsafe {
            AXValueGetValue(
                self.0,
                kAXValueCGPointType,
                &mut point as *mut _ as *mut std::ffi::c_void,
            )
        };
        if extracted {
            Some(point)
        } else {
            None
        }
    }

    /// Extract a CGSize from an AXValue
    fn extract_size(&self) -> Option<core_graphics::geometry::CGSize> {
        let mut size = core_graphics::geometry::CGSize::new(0.0, 0.0);
        let extracted = unsafe {
            AXValueGetValue(
                self.0,
                kAXValueCGSizeType,
                &mut size as *mut _ as *mut std::ffi::c_void,
            )
        };
        if extracted {
            Some(size)
        } else {
            None
        }
    }

    /// Convert to CFString and get as Rust String.
    /// Note: This consumes the handle to avoid double-free.
    fn into_string(self) -> Option<String> {
        // wrap_under_create_rule takes ownership, so we need to prevent
        // our Drop from also releasing
        let cf_string: CFString = unsafe { CFString::wrap_under_create_rule(self.0 as _) };
        let result = cf_string.to_string();
        // Prevent double-free by forgetting self (CFString now owns the ref)
        std::mem::forget(self);
        Some(result)
    }
}

impl Drop for CFHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) };
        }
    }
}

/// Context about the focused application for later restoration
#[derive(Debug, Clone)]
pub struct FocusContext {
    pub app_pid: i32,
    #[allow(dead_code)]
    pub app_bundle_id: String,
    /// The focused UI element (if captured) for live text updates
    pub focused_element: Option<AXElementHandle>,
}

/// Capture the current focus context (which app is focused)
pub fn capture_focus_context() -> Option<FocusContext> {
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

        let bundle_id: *mut objc::runtime::Object = msg_send![app, bundleIdentifier];
        if bundle_id.is_null() {
            return None;
        }

        let utf8: *const std::os::raw::c_char = msg_send![bundle_id, UTF8String];
        if utf8.is_null() {
            return None;
        }

        let bundle_id_str = std::ffi::CStr::from_ptr(utf8)
            .to_string_lossy()
            .into_owned();

        // Also try to capture the focused UI element for live text updates
        let focused_element = capture_focused_element();

        Some(FocusContext {
            app_pid: pid,
            app_bundle_id: bundle_id_str,
            focused_element,
        })
    }
}

/// Capture a handle to the currently focused UI element
fn capture_focused_element() -> Option<AXElementHandle> {
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

        CFRelease(focused_app);
        CFRelease(system_wide);

        if result != 0 || focused_element.is_null() {
            return None;
        }

        // Create a handle (which will retain the element)
        let handle = AXElementHandle::new(focused_element);

        // Release our copy since AXElementHandle retained it
        CFRelease(focused_element);

        handle
    }
}

/// Restore focus to a previously captured application
pub fn restore_focus(context: &FocusContext) -> Result<(), String> {
    log::info!("Attempting to restore focus to PID {}", context.app_pid);

    unsafe {
        use objc::{class, msg_send, sel, sel_impl};

        let running_app_class = class!(NSRunningApplication);
        let app: *mut objc::runtime::Object = msg_send![
            running_app_class,
            runningApplicationWithProcessIdentifier: context.app_pid
        ];

        if app.is_null() {
            log::error!(
                "Could not find running application with PID {}",
                context.app_pid
            );
            return Err(format!(
                "Could not find running application with PID {}",
                context.app_pid
            ));
        }

        // NSApplicationActivateIgnoringOtherApps = 1 << 1 = 2
        let options: u64 = 2;
        let success: bool = msg_send![app, activateWithOptions: options];

        if success {
            log::info!("Successfully activated application");
            Ok(())
        } else {
            log::error!("Failed to activate application");
            Err("Failed to activate application".to_string())
        }
    }
}

/// Position and size of a UI element
#[derive(Debug, Clone)]
pub struct ElementFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Get the frame of the frontmost window of the focused application
pub fn get_focused_window_frame() -> Option<ElementFrame> {
    let context = capture_focus_context()?;
    get_window_frame_for_pid(context.app_pid)
}

/// Get the frame of the focused window for a specific application PID
pub fn get_window_frame_for_pid(pid: i32) -> Option<ElementFrame> {
    let app_element = CFHandle::new(unsafe { AXUIElementCreateApplication(pid) })?;

    let focused_window = app_element.get_attribute("AXFocusedWindow").or_else(|| {
        log::warn!(
            "get_window_frame_for_pid: Failed to get AXFocusedWindow for pid {}",
            pid
        );
        None
    })?;

    let position_value = focused_window.get_attribute("AXPosition").or_else(|| {
        log::warn!("get_window_frame_for_pid: Failed to get AXPosition");
        None
    })?;

    let size_value = focused_window.get_attribute("AXSize").or_else(|| {
        log::warn!("get_window_frame_for_pid: Failed to get AXSize");
        None
    })?;

    let point = position_value.extract_point().or_else(|| {
        log::warn!("get_window_frame_for_pid: Failed to extract CGPoint from AXPosition");
        None
    })?;

    let size = size_value.extract_size().or_else(|| {
        log::warn!("get_window_frame_for_pid: Failed to extract CGSize from AXSize");
        None
    })?;

    log::info!(
        "get_window_frame_for_pid: Got frame x={}, y={}, w={}, h={}",
        point.x,
        point.y,
        size.width,
        size.height
    );

    Some(ElementFrame {
        x: point.x,
        y: point.y,
        width: size.width,
        height: size.height,
    })
}

/// Get the position and size of the currently focused UI element
pub fn get_focused_element_frame() -> Option<ElementFrame> {
    let system_wide = CFHandle::new(unsafe { AXUIElementCreateSystemWide() })?;
    let focused_app = system_wide.get_attribute("AXFocusedApplication")?;
    let focused_element = focused_app.get_attribute("AXFocusedUIElement")?;

    let position_value = focused_element.get_attribute("AXPosition")?;
    let size_value = focused_element.get_attribute("AXSize")?;

    let point = position_value.extract_point()?;
    let size = size_value.extract_size()?;

    Some(ElementFrame {
        x: point.x,
        y: point.y,
        width: size.width,
        height: size.height,
    })
}

/// Get the full text value from the currently focused UI element
pub fn get_focused_element_text() -> Option<String> {
    let system_wide = CFHandle::new(unsafe { AXUIElementCreateSystemWide() })?;
    let focused_app = system_wide.get_attribute("AXFocusedApplication")?;
    let focused_element = focused_app.get_attribute("AXFocusedUIElement")?;
    let value = focused_element.get_attribute("AXValue")?;
    value.into_string()
}

/// Get the AXRole of the currently focused UI element
pub fn get_focused_element_role() -> Option<String> {
    let system_wide = CFHandle::new(unsafe { AXUIElementCreateSystemWide() })?;
    let focused_app = system_wide.get_attribute("AXFocusedApplication")?;
    let focused_element = focused_app.get_attribute("AXFocusedUIElement")?;
    let role = focused_element.get_attribute("AXRole")?;
    role.into_string()
}

/// Get the AXSubrole of the currently focused UI element (if any)
pub fn get_focused_element_subrole() -> Option<String> {
    let system_wide = CFHandle::new(unsafe { AXUIElementCreateSystemWide() })?;
    let focused_app = system_wide.get_attribute("AXFocusedApplication")?;
    let focused_element = focused_app.get_attribute("AXFocusedUIElement")?;
    let subrole = focused_element.get_attribute("AXSubrole")?;
    subrole.into_string()
}

/// Check if the currently focused element is a text input field or editable area
/// Returns true if a text field is focused, false otherwise
pub fn is_text_field_focused() -> bool {
    let system_wide = match CFHandle::new(unsafe { AXUIElementCreateSystemWide() }) {
        Some(sw) => sw,
        None => return false,
    };
    let focused_app = match system_wide.get_attribute("AXFocusedApplication") {
        Some(app) => app,
        None => return false,
    };
    let focused_element = match focused_app.get_attribute("AXFocusedUIElement") {
        Some(el) => el,
        None => return false,
    };
    let role = match focused_element.get_attribute("AXRole") {
        Some(r) => r,
        None => return false,
    };
    let role_str = match role.into_string() {
        Some(s) => s,
        None => return false,
    };

    // Check for known text input roles
    // - AXTextField: standard single-line text input
    // - AXTextArea: multi-line text input (e.g., notes, code editors)
    // - AXComboBox: dropdown with text input
    // - AXSearchField: search input field
    let is_known_text_role = matches!(
        role_str.as_str(),
        "AXTextField" | "AXTextArea" | "AXComboBox" | "AXSearchField"
    );

    if is_known_text_role {
        return true;
    }

    // Check for autocomplete/suggestion list elements
    // When these are focused, the user is still in a text editing context
    // (e.g., Monaco Editor's IntelliSense popup, VS Code suggestions)
    // - AXList: list of suggestions
    // - AXRow: individual suggestion row
    // - AXCell: table cell in suggestions
    // - AXMenuItem/AXMenu: context menus that appear during typing
    let is_autocomplete_element = matches!(
        role_str.as_str(),
        "AXList" | "AXRow" | "AXCell" | "AXMenuItem" | "AXMenu" | "AXPopover"
    );

    if is_autocomplete_element {
        return true;
    }

    // For web content and other elements, check if the element is editable
    // This handles contenteditable elements in browsers (email composers, etc.)
    // Note: We need to get focused_element again since it was consumed by into_string
    if let Some(focused_app) = system_wide.get_attribute("AXFocusedApplication") {
        if let Some(focused_element) = focused_app.get_attribute("AXFocusedUIElement") {
            if let Some(editable_attr) = focused_element.get_attribute("AXEditable") {
                // AXEditable is a boolean attribute
                // The value is a CFBoolean - check if it's true
                if is_cf_boolean_true(&editable_attr) {
                    return true;
                }
            }
        }
    }

    log::trace!("is_text_field_focused: role={} (not editable)", role_str);
    false
}

/// Check if a CFHandle wraps a CFBoolean with value true
fn is_cf_boolean_true(handle: &CFHandle) -> bool {
    // Check if this is kCFBooleanTrue
    // CFBoolean values are singletons, so we can compare pointers
    unsafe {
        let cf_true = core_foundation::boolean::kCFBooleanTrue;
        handle.0 == cf_true as CFTypeRef
    }
}

/// Check if any app in the blocklist has visible windows
/// Returns true if an overlay from a blocklisted app is visible
pub fn has_visible_overlay_window(blocklist: &[String]) -> bool {
    if blocklist.is_empty() {
        return false;
    }

    unsafe {
        use objc::{class, msg_send, sel, sel_impl};

        let workspace: *mut objc::runtime::Object =
            msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace.is_null() {
            return false;
        }

        let running_apps: *mut objc::runtime::Object = msg_send![workspace, runningApplications];
        if running_apps.is_null() {
            return false;
        }

        let count: usize = msg_send![running_apps, count];

        for i in 0..count {
            let app: *mut objc::runtime::Object = msg_send![running_apps, objectAtIndex: i];
            if app.is_null() {
                continue;
            }

            let bundle_id: *mut objc::runtime::Object = msg_send![app, bundleIdentifier];
            if bundle_id.is_null() {
                continue;
            }

            let utf8: *const std::os::raw::c_char = msg_send![bundle_id, UTF8String];
            if utf8.is_null() {
                continue;
            }

            let bundle_str = std::ffi::CStr::from_ptr(utf8)
                .to_string_lossy()
                .into_owned();

            if blocklist.contains(&bundle_str) {
                // Check if this app has any visible windows using CGWindowList
                let pid: i32 = msg_send![app, processIdentifier];
                if has_visible_windows_for_pid(pid) {
                    return true;
                }
            }
        }
    }

    false
}

/// Check if a process has any visible windows using CGWindowListCopyWindowInfo
fn has_visible_windows_for_pid(pid: i32) -> bool {
    use core_graphics::window::{
        kCGNullWindowID, kCGWindowListOptionOnScreenOnly, CGWindowListCopyWindowInfo,
    };

    unsafe {
        let window_list = CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly, kCGNullWindowID);
        if window_list.is_null() {
            return false;
        }

        let count = core_foundation::array::CFArrayGetCount(window_list as _);

        for i in 0..count {
            let window_info = core_foundation::array::CFArrayGetValueAtIndex(window_list as _, i)
                as core_foundation::dictionary::CFDictionaryRef;
            if window_info.is_null() {
                continue;
            }

            // Get the owner PID
            let pid_key = core_foundation::string::CFString::new("kCGWindowOwnerPID");
            let mut pid_value: *const std::ffi::c_void = std::ptr::null();

            if core_foundation::dictionary::CFDictionaryGetValueIfPresent(
                window_info,
                pid_key.as_CFTypeRef() as _,
                &mut pid_value,
            ) != 0 && !pid_value.is_null()
            {
                let mut owner_pid: i32 = 0;
                if core_foundation::number::CFNumberGetValue(
                    pid_value as core_foundation::number::CFNumberRef,
                    core_foundation::number::kCFNumberSInt32Type,
                    &mut owner_pid as *mut i32 as *mut std::ffi::c_void,
                ) && owner_pid == pid
                {
                    // Check window layer - layer 0 is normal windows
                    let layer_key = core_foundation::string::CFString::new("kCGWindowLayer");
                    let mut layer_value: *const std::ffi::c_void = std::ptr::null();

                    if core_foundation::dictionary::CFDictionaryGetValueIfPresent(
                        window_info,
                        layer_key.as_CFTypeRef() as _,
                        &mut layer_value,
                    ) != 0 && !layer_value.is_null()
                    {
                        let mut layer: i32 = -1;
                        core_foundation::number::CFNumberGetValue(
                            layer_value as core_foundation::number::CFNumberRef,
                            core_foundation::number::kCFNumberSInt32Type,
                            &mut layer as *mut i32 as *mut std::ffi::c_void,
                        );

                        // Normal windows are at layer 0, some overlays might be higher
                        // Accept layer 0 and above as visible windows
                        if layer >= 0 {
                            core_foundation::base::CFRelease(window_list as _);
                            return true;
                        }
                    }
                }
            }
        }

        core_foundation::base::CFRelease(window_list as _);
    }

    false
}

/// Get the bounds of the screen containing a given point
/// Returns the screen frame (x, y, width, height) in screen coordinates
pub fn get_screen_bounds_for_point(x: f64, y: f64) -> Option<ElementFrame> {
    unsafe {
        use objc::{class, msg_send, sel, sel_impl};

        // Get all screens
        let screens: *mut objc::runtime::Object = msg_send![class!(NSScreen), screens];
        if screens.is_null() {
            return None;
        }

        let count: usize = msg_send![screens, count];
        if count == 0 {
            return None;
        }

        // Find the screen containing the point
        for i in 0..count {
            let screen: *mut objc::runtime::Object = msg_send![screens, objectAtIndex: i];
            if screen.is_null() {
                continue;
            }

            // Get the screen's frame (in Cocoa coordinates - origin at bottom-left)
            let frame: core_graphics::geometry::CGRect = msg_send![screen, frame];

            // Convert Cocoa coordinates to screen coordinates (origin at top-left)
            // In Cocoa, y=0 is at the bottom, but we work with y=0 at top
            // The main screen's height gives us the reference
            let main_screen: *mut objc::runtime::Object = msg_send![class!(NSScreen), mainScreen];
            let main_frame: core_graphics::geometry::CGRect = msg_send![main_screen, frame];
            let main_height = main_frame.size.height;

            // Convert y coordinate: screen_y = main_height - cocoa_y - screen_height
            let screen_y = main_height - frame.origin.y - frame.size.height;

            // Check if point is within this screen (using screen coordinates)
            let screen_left = frame.origin.x;
            let screen_right = frame.origin.x + frame.size.width;
            let screen_top = screen_y;
            let screen_bottom = screen_y + frame.size.height;

            if x >= screen_left && x < screen_right && y >= screen_top && y < screen_bottom {
                return Some(ElementFrame {
                    x: screen_left,
                    y: screen_top,
                    width: frame.size.width,
                    height: frame.size.height,
                });
            }
        }

        // Fallback to main screen if point not found on any screen
        let main_screen: *mut objc::runtime::Object = msg_send![class!(NSScreen), mainScreen];
        if main_screen.is_null() {
            return None;
        }

        let frame: core_graphics::geometry::CGRect = msg_send![main_screen, frame];

        Some(ElementFrame {
            x: frame.origin.x,
            y: 0.0, // Main screen top is always 0 in screen coordinates
            width: frame.size.width,
            height: frame.size.height,
        })
    }
}

/// Set the text value of a UI element
///
/// This is used for live text sync - updating the original text field
/// while the user edits in nvim. Returns Ok(()) if successful, Err otherwise.
///
/// Note: This may not work for all apps, especially:
/// - Web browsers (use JavaScript injection instead)
/// - Some Electron apps
/// - Apps that don't properly implement accessibility
pub fn set_element_text(element: &AXElementHandle, text: &str) -> Result<(), String> {
    unsafe {
        let value_attr = CFString::new("AXValue");
        let cf_text = CFString::new(text);

        let result = AXUIElementSetAttributeValue(
            element.as_ptr(),
            value_attr.as_CFTypeRef(),
            cf_text.as_CFTypeRef(),
        );

        if result == 0 {
            Ok(())
        } else {
            // Common error codes:
            // -25200: kAXErrorAttributeUnsupported
            // -25201: kAXErrorActionUnsupported
            // -25202: kAXErrorNotificationUnsupported
            // -25203: kAXErrorNotImplemented
            // -25204: kAXErrorNotificationAlreadyRegistered
            // -25205: kAXErrorNotificationNotRegistered
            // -25206: kAXErrorAPIDisabled
            // -25207: kAXErrorNoValue
            // -25208: kAXErrorParameterizedAttributeUnsupported
            // -25209: kAXErrorNotEnoughPrecision
            // -25210: kAXErrorIllegalArgument
            // -25211: kAXErrorCannotComplete
            // -25212: kAXErrorFailure
            Err(format!("AXUIElementSetAttributeValue failed with error code: {}", result))
        }
    }
}
