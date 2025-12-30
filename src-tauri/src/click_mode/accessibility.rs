//! Accessibility API integration for Click Mode
//!
//! Uses macOS Accessibility API to discover clickable UI elements
//! in the frontmost application.

#![allow(dead_code)]

use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::string::CFString;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use crate::nvim_edit::accessibility::AXElementHandle;

use super::element::ClickableElementInternal;
use super::hints::generate_hints;

/// Cache for clickable elements to speed up repeated activations
struct ElementCache {
    /// Cached elements (raw data before hint generation)
    elements: Vec<RawElementData>,
    /// PID of the app these elements belong to
    pid: i32,
    /// When the cache was populated
    timestamp: Instant,
    /// Whether this is modal content (sheet/dialog)
    is_modal: bool,
}

/// Global element cache with short TTL
static ELEMENT_CACHE: OnceLock<Mutex<Option<ElementCache>>> = OnceLock::new();

/// Cache TTL in milliseconds - elements are valid for this long
const CACHE_TTL_MS: u128 = 500;

fn get_cache() -> &'static Mutex<Option<ElementCache>> {
    ELEMENT_CACHE.get_or_init(|| Mutex::new(None))
}

/// Check if we have valid cached elements for the given PID
fn get_cached_elements(pid: i32) -> Option<(Vec<RawElementData>, bool)> {
    let cache = get_cache().lock().ok()?;
    let cached = cache.as_ref()?;

    // Check if cache is for the right PID and not expired
    if cached.pid == pid && cached.timestamp.elapsed().as_millis() < CACHE_TTL_MS {
        log::info!("Using cached elements (age: {}ms)", cached.timestamp.elapsed().as_millis());
        Some((cached.elements.clone(), cached.is_modal))
    } else {
        None
    }
}

/// Store elements in cache
fn cache_elements(pid: i32, elements: Vec<RawElementData>, is_modal: bool) {
    if let Ok(mut cache) = get_cache().lock() {
        *cache = Some(ElementCache {
            elements,
            pid,
            timestamp: Instant::now(),
            is_modal,
        });
    }
}

/// Invalidate the cache (call when app focus changes)
pub fn invalidate_cache() {
    if let Ok(mut cache) = get_cache().lock() {
        *cache = None;
        log::debug!("Element cache invalidated");
    }
}

/// Prefetch elements in background for faster click mode activation
/// Call this when window focus changes to warm the cache
pub fn prefetch_elements() {
    std::thread::spawn(|| {
        if let Some(pid) = get_frontmost_app_pid() {
            log::debug!("Prefetching elements for PID {}", pid);
            // Query elements - this will populate the cache
            let _ = query_elements_subprocess(pid);
        }
    });
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
    fn AXUIElementCopyAttributeValue(
        element: CFTypeRef,
        attribute: CFTypeRef,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXUIElementCopyAttributeNames(element: CFTypeRef, names: *mut CFTypeRef) -> i32;
    fn AXUIElementPerformAction(element: CFTypeRef, action: CFTypeRef) -> i32;
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

/// Maximum depth to traverse the UI hierarchy
const MAX_DEPTH: usize = 10;

/// Maximum number of elements to collect (performance limit)
const MAX_ELEMENTS: usize = 200;

/// Get the frontmost application's PID
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

/// Get the bundle identifier of the frontmost application
pub fn get_frontmost_app_bundle_id() -> Option<String> {
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

        let bundle_id: *mut objc::runtime::Object = msg_send![app, bundleIdentifier];
        if bundle_id.is_null() {
            return None;
        }

        let utf8: *const std::os::raw::c_char = msg_send![bundle_id, UTF8String];
        if utf8.is_null() {
            return None;
        }

        let c_str = std::ffi::CStr::from_ptr(utf8);
        c_str.to_str().ok().map(|s| s.to_string())
    }
}

/// RAII wrapper for CFTypeRef
struct CFHandle(CFTypeRef);

impl CFHandle {
    fn new(ptr: CFTypeRef) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            Some(Self(ptr))
        }
    }

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
        handle.into_string()
    }

    fn into_string(self) -> Option<String> {
        let cf_string: CFString = unsafe { CFString::wrap_under_create_rule(self.0 as _) };
        let result = cf_string.to_string();
        std::mem::forget(self);
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

    fn as_ptr(&self) -> CFTypeRef {
        self.0
    }
}

impl Drop for CFHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) };
        }
    }
}

/// Check if an element has a clickable action
fn has_press_action(element: &CFHandle) -> bool {
    // Get AXActions attribute - this returns an array of action names
    let actions_handle = match element.get_attribute("AXActions") {
        Some(h) => h,
        None => return false,
    };

    let actions_ptr = actions_handle.0;
    if actions_ptr.is_null() {
        return false;
    }

    // Safely get array count
    let count = unsafe { core_foundation::array::CFArrayGetCount(actions_ptr as _) };
    if count <= 0 {
        return false;
    }

    // Check each action
    for i in 0..count.min(20) {
        let action_ptr = unsafe {
            core_foundation::array::CFArrayGetValueAtIndex(actions_ptr as _, i)
        };

        if action_ptr.is_null() {
            continue;
        }

        // Try to convert to string - be defensive
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

/// Check if an element is visible on screen
fn is_visible(element: &CFHandle) -> bool {
    // Check if position and size are valid
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

    // Element must have positive size and be on screen
    w > 0.0 && h > 0.0 && x >= -10000.0 && y >= -10000.0
}

/// Check if a role is clickable
fn is_clickable_role(role: &str) -> bool {
    CLICKABLE_ROLES.iter().any(|r| *r == role)
}

/// Recursively collect clickable elements
fn collect_elements(
    element: &CFHandle,
    elements: &mut Vec<(f64, f64, f64, f64, String, String, AXElementHandle)>,
    depth: usize,
) {
    if depth > MAX_DEPTH || elements.len() >= MAX_ELEMENTS {
        return;
    }

    // Get role
    let role = element.get_string_attribute("AXRole").unwrap_or_default();

    // Check if this element is clickable
    let is_clickable = is_clickable_role(&role) || has_press_action(element);

    if is_clickable && is_visible(element) {
        // Get position and size
        if let (Some(pos), Some(size)) = (
            element.get_attribute("AXPosition").and_then(|p| p.extract_point()),
            element.get_attribute("AXSize").and_then(|s| s.extract_size()),
        ) {
            // Get title (try multiple attributes)
            let title = element
                .get_string_attribute("AXTitle")
                .or_else(|| element.get_string_attribute("AXDescription"))
                .or_else(|| element.get_string_attribute("AXValue"))
                .or_else(|| element.get_string_attribute("AXLabel"))
                .or_else(|| element.get_string_attribute("AXHelp"))
                .unwrap_or_default();

            // Create AX handle for later actions
            if let Some(ax_handle) = unsafe { AXElementHandle::new(element.as_ptr()) } {
                elements.push((pos.0, pos.1, size.0, size.1, role, title, ax_handle));
            }
        }
    }

    // Recurse into children
    // Use AXUIElementCopyAttributeValue which gives us an owned array
    let children_attr = CFString::new("AXChildren");
    let mut children_value: CFTypeRef = std::ptr::null();
    let result = unsafe {
        AXUIElementCopyAttributeValue(element.0, children_attr.as_CFTypeRef(), &mut children_value)
    };

    if result != 0 || children_value.is_null() {
        return;
    }

    // We now own children_value - wrap it in CFHandle for cleanup
    let _children_handle = CFHandle(children_value);

    let count = unsafe { core_foundation::array::CFArrayGetCount(children_value as _) };
    if count <= 0 {
        return;
    }

    // Limit to prevent performance issues
    let safe_count = (count as usize).min(100);

    for i in 0..safe_count {
        if elements.len() >= MAX_ELEMENTS {
            break;
        }

        // Get child - the array owns it, we get a borrowed reference
        let child_ptr = unsafe {
            core_foundation::array::CFArrayGetValueAtIndex(children_value as _, i as isize)
        };

        if child_ptr.is_null() {
            continue;
        }

        // Retain the child so we own it for recursion
        // The array retains its elements, so this should be safe
        unsafe { core_foundation::base::CFRetain(child_ptr) };
        let child = CFHandle(child_ptr);
        collect_elements(&child, elements, depth + 1);
    }
}

/// Inner function to collect elements - separated so we can wrap with objc exception handling
fn collect_elements_inner(
    pid: i32,
) -> Result<Vec<(f64, f64, f64, f64, String, String, AXElementHandle)>, String> {
    log::debug!("Creating AX element for app");

    let app_element = unsafe {
        let ptr = AXUIElementCreateApplication(pid);
        if ptr.is_null() {
            return Err("Could not create AX element for app".to_string());
        }
        CFHandle(ptr)
    };

    log::debug!("AX element created, checking focused window");

    let mut raw_elements: Vec<(f64, f64, f64, f64, String, String, AXElementHandle)> = Vec::new();

    // Start from focused window if available, otherwise from app
    log::debug!("Getting focused window");
    let start_element = app_element
        .get_attribute("AXFocusedWindow")
        .and_then(|w| {
            let ptr = w.as_ptr();
            if ptr.is_null() {
                return None;
            }
            std::mem::forget(w);
            Some(CFHandle(unsafe { core_foundation::base::CFRetain(ptr) }))
        })
        .unwrap_or_else(|| {
            log::debug!("No focused window, using app element");
            // Retain the app element for traversal
            CFHandle(unsafe { core_foundation::base::CFRetain(app_element.as_ptr()) })
        });

    log::debug!("Starting element collection");
    collect_elements(&start_element, &mut raw_elements, 0);
    log::debug!("Element collection complete");

    Ok(raw_elements)
}

/// Raw element data from subprocess (matches ax_helper output)
#[derive(Debug, Clone, serde::Deserialize)]
struct RawElementData {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    role: String,
    title: String,
}

/// Helper output with metadata
#[derive(Debug, Clone, serde::Deserialize)]
struct HelperOutput {
    elements: Vec<RawElementData>,
    /// True if elements were collected from a sheet/dialog (modal UI)
    is_modal: bool,
}

/// Get the path to the helper binary in Application Support
fn get_helper_path() -> Option<std::path::PathBuf> {
    dirs::data_dir().map(|d| d.join("ovim").join("ovim-ax-helper"))
}

/// Initialize the helper binary by copying it to Application Support
/// Call this on app startup
pub fn init_helper() {
    // Try to find source binary next to main executable
    let source_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("ovim-ax-helper")));

    let source_path = match source_path {
        Some(p) if p.exists() => p,
        _ => {
            log::debug!("Helper binary not found next to executable");
            return;
        }
    };

    let dest_path = match get_helper_path() {
        Some(p) => p,
        None => {
            log::warn!("Could not determine Application Support path");
            return;
        }
    };

    // Create directory if needed
    if let Some(parent) = dest_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            log::warn!("Failed to create helper directory: {}", e);
            return;
        }
    }

    // Copy if source is newer or dest doesn't exist
    let should_copy = if dest_path.exists() {
        // Check if source is newer
        match (source_path.metadata(), dest_path.metadata()) {
            (Ok(src), Ok(dst)) => {
                src.modified().ok() > dst.modified().ok()
            }
            _ => true,
        }
    } else {
        true
    };

    if should_copy {
        match std::fs::copy(&source_path, &dest_path) {
            Ok(_) => {
                log::info!("Copied helper binary to {:?}", dest_path);
                // Make executable
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = dest_path.metadata() {
                        let mut perms = metadata.permissions();
                        perms.set_mode(0o755);
                        let _ = std::fs::set_permissions(&dest_path, perms);
                    }
                }
            }
            Err(e) => {
                log::warn!("Failed to copy helper binary: {}", e);
            }
        }
    }
}

/// Get the helper binary path
fn get_helper_binary_path() -> Option<std::path::PathBuf> {
    // First try Application Support path
    get_helper_path()
        .filter(|p| p.exists())
        // Fall back to next to executable
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("ovim-ax-helper")))
                .filter(|p| p.exists())
        })
}

/// Query elements using the subprocess (internal, for caching)
/// Returns raw elements and is_modal flag
fn query_elements_subprocess(pid: i32) -> Result<(Vec<RawElementData>, bool), String> {
    let helper_path = match get_helper_binary_path() {
        Some(p) => p,
        None => {
            log::error!("Helper binary not found - click mode cannot work safely");
            return Err("Helper binary not found. Please reinstall ovim.".to_string());
        }
    };

    log::debug!("Using helper at {:?}", helper_path);

    // Run the helper subprocess with retry logic
    let mut helper_output: Option<HelperOutput> = None;

    for attempt in 0..3 {
        if attempt > 0 {
            // Short delay between retries - use exponential backoff
            let delay = 100 * (1 << (attempt - 1)); // 100ms, 200ms
            log::info!("Retry attempt {} after {}ms...", attempt, delay);
            std::thread::sleep(std::time::Duration::from_millis(delay));
        }

        let output = match std::process::Command::new(&helper_path)
            .arg(pid.to_string())
            .output()
        {
            Ok(o) => o,
            Err(e) => {
                log::warn!("Failed to run helper (attempt {}): {}", attempt + 1, e);
                continue;
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            log::warn!("Helper subprocess failed (attempt {}): {}", attempt + 1, stderr.trim());
            continue;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        match serde_json::from_str::<HelperOutput>(&stdout) {
            Ok(parsed) => {
                helper_output = Some(parsed);
                break;
            }
            Err(e) => {
                log::warn!("Failed to parse helper output (attempt {}): {}", attempt + 1, e);
                continue;
            }
        }
    }

    let helper_output = match helper_output {
        Some(o) => o,
        None => {
            log::error!("All subprocess attempts failed - accessibility API may be unstable");
            return Err("Failed to query elements - try again in a moment".to_string());
        }
    };

    let is_modal = helper_output.is_modal;
    log::info!("Found {} raw clickable elements via subprocess (is_modal: {})",
        helper_output.elements.len(), is_modal);

    // Cache the results
    cache_elements(pid, helper_output.elements.clone(), is_modal);

    Ok((helper_output.elements, is_modal))
}

/// Query all clickable elements using a subprocess
/// This prevents crashes from Objective-C exceptions in the accessibility API
pub fn get_clickable_elements() -> Result<Vec<ClickableElementInternal>, String> {
    let start = Instant::now();

    let pid = get_frontmost_app_pid().ok_or("Could not get frontmost app")?;
    let bundle_id = get_frontmost_app_bundle_id();

    log::info!("Querying clickable elements for PID {}", pid);

    // Detect browser type early so we can parallelize
    let browser_type = bundle_id
        .as_ref()
        .and_then(|id| super::browser_clickables::detect_browser_type(id));

    // Check cache first
    let cached = get_cached_elements(pid);

    // If cache miss and this is a browser, run AX query and JS query in parallel
    let (mut all_elements, is_modal, web_clickables) = if let Some((cached_els, cached_modal)) = cached {
        log::info!("[TIMING] Cache hit! Using {} cached elements ({}ms)", cached_els.len(), start.elapsed().as_millis());
        (cached_els, cached_modal, None)
    } else if let Some(bt) = browser_type {
        if bt.needs_js_injection() {
            log::info!("[TIMING] Cache miss, running AX + browser JS in parallel");

            // Run both queries in parallel using threads
            let bt_clone = bt;
            let js_handle = std::thread::spawn(move || {
                super::browser_clickables::get_browser_clickables(bt_clone)
            });

            // Run subprocess query on current thread
            let (ax_elements, is_modal) = query_elements_subprocess(pid)?;
            log::info!("[TIMING] Subprocess query took {}ms", start.elapsed().as_millis());

            // Wait for JS results
            let js_result = js_handle.join().ok().and_then(|r| r.ok());
            log::info!("[TIMING] Parallel queries complete at {}ms", start.elapsed().as_millis());

            (ax_elements, is_modal, js_result)
        } else {
            // Safari - no JS injection needed
            log::info!("[TIMING] Cache miss, querying via subprocess (Safari)");
            let result = query_elements_subprocess(pid)?;
            log::info!("[TIMING] Subprocess query took {}ms", start.elapsed().as_millis());
            (result.0, result.1, None)
        }
    } else {
        // Non-browser app
        log::info!("[TIMING] Cache miss, querying via subprocess (non-browser)");
        let result = query_elements_subprocess(pid)?;
        log::info!("[TIMING] Subprocess query took {}ms", start.elapsed().as_millis());
        (result.0, result.1, None)
    };

    // Append web clickables if we got any
    if let Some(web_els) = web_clickables {
        log::info!("Found {} web clickables via JavaScript", web_els.len());
        for wc in web_els {
            all_elements.push(RawElementData {
                x: wc.x,
                y: wc.y,
                width: wc.width,
                height: wc.height,
                role: wc.tag.clone(),
                title: wc.text.clone(),
            });
        }
    } else if is_modal {
        log::info!("Modal dialog detected, skipped browser JS injection");
    }

    log::info!("Total clickable elements: {}", all_elements.len());

    // Generate hints
    let hints = generate_hints(all_elements.len(), super::hints::DEFAULT_HINT_CHARS);

    // Convert to internal elements
    // Note: No AXElementHandle - clicks will use position-based mouse simulation
    let elements: Vec<ClickableElementInternal> = all_elements
        .into_iter()
        .enumerate()
        .map(|(i, elem)| {
            ClickableElementInternal::new(
                i,
                hints.get(i).cloned().unwrap_or_else(|| i.to_string()),
                elem.x,
                elem.y,
                elem.width,
                elem.height,
                elem.role,
                elem.title,
                None, // No AX handle in subprocess mode
            )
        })
        .collect();

    log::info!("[TIMING] Total get_clickable_elements took {}ms", start.elapsed().as_millis());

    Ok(elements)
}


/// Direct query without subprocess (fallback)
fn get_clickable_elements_direct() -> Result<Vec<ClickableElementInternal>, String> {
    std::thread::sleep(std::time::Duration::from_millis(200));

    let pid = get_frontmost_app_pid().ok_or("Could not get frontmost app")?;

    log::info!("Querying clickable elements for PID {} directly", pid);

    let raw_elements = collect_elements_inner(pid)?;

    log::info!("Found {} raw clickable elements", raw_elements.len());

    let hints = generate_hints(raw_elements.len(), super::hints::DEFAULT_HINT_CHARS);

    let elements: Vec<ClickableElementInternal> = raw_elements
        .into_iter()
        .enumerate()
        .map(|(i, (x, y, w, h, role, title, ax_handle))| {
            ClickableElementInternal::new(
                i,
                hints.get(i).cloned().unwrap_or_else(|| i.to_string()),
                x,
                y,
                w,
                h,
                role,
                title,
                Some(ax_handle),
            )
        })
        .collect();

    Ok(elements)
}

/// Perform a click action on an element
pub fn perform_click(element: &AXElementHandle) -> Result<(), String> {
    // Use simulated mouse click as primary method
    // AXPress often returns success but doesn't actually work for menu bar items
    log::info!("Performing mouse click on element");
    perform_mouse_click(element)
}

/// Perform a simulated mouse click at the element's position
fn perform_mouse_click(element: &AXElementHandle) -> Result<(), String> {
    use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use core_graphics::geometry::CGPoint;

    // Get element position
    let position_attr = CFString::new("AXPosition");
    let mut position_value: CFTypeRef = std::ptr::null();

    let result = unsafe {
        AXUIElementCopyAttributeValue(
            element.as_ptr(),
            position_attr.as_CFTypeRef(),
            &mut position_value,
        )
    };

    if result != 0 || position_value.is_null() {
        return Err("Could not get element position".to_string());
    }

    let position_handle = CFHandle(position_value);
    let (x, y) = position_handle
        .extract_point()
        .ok_or("Could not extract position point")?;

    // Get element size to click in center
    let size_attr = CFString::new("AXSize");
    let mut size_value: CFTypeRef = std::ptr::null();

    let result = unsafe {
        AXUIElementCopyAttributeValue(element.as_ptr(), size_attr.as_CFTypeRef(), &mut size_value)
    };

    let (center_x, center_y) = if result == 0 && !size_value.is_null() {
        let size_handle = CFHandle(size_value);
        if let Some((w, h)) = size_handle.extract_size() {
            (x + w / 2.0, y + h / 2.0)
        } else {
            (x, y)
        }
    } else {
        (x, y)
    };

    log::info!("Performing mouse click at ({}, {})", center_x, center_y);

    // Create and post mouse events
    let point = CGPoint::new(center_x, center_y);

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "Could not create event source")?;

    // Mouse down
    let mouse_down = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::LeftMouseDown,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| "Could not create mouse down event")?;

    mouse_down.post(CGEventTapLocation::HID);

    // Small delay
    std::thread::sleep(std::time::Duration::from_millis(10));

    // Mouse up
    let mouse_up = CGEvent::new_mouse_event(
        source,
        CGEventType::LeftMouseUp,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| "Could not create mouse up event")?;

    mouse_up.post(CGEventTapLocation::HID);

    log::info!("Mouse click completed");
    Ok(())
}

/// Perform a click at a specific position
pub fn perform_click_at_position(x: f64, y: f64) -> Result<(), String> {
    use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use core_graphics::geometry::CGPoint;

    log::info!("Performing mouse click at position ({}, {})", x, y);

    let point = CGPoint::new(x, y);

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "Could not create event source")?;

    // Mouse down
    let mouse_down = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::LeftMouseDown,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| "Could not create mouse down event")?;

    mouse_down.post(CGEventTapLocation::HID);

    // Small delay
    std::thread::sleep(std::time::Duration::from_millis(10));

    // Mouse up
    let mouse_up = CGEvent::new_mouse_event(
        source,
        CGEventType::LeftMouseUp,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| "Could not create mouse up event")?;

    mouse_up.post(CGEventTapLocation::HID);

    log::info!("Mouse click completed");
    Ok(())
}

/// Perform a right-click at a specific position
pub fn perform_right_click_at_position(x: f64, y: f64) -> Result<(), String> {
    use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use core_graphics::geometry::CGPoint;

    log::info!("Performing right-click at position ({}, {})", x, y);

    let point = CGPoint::new(x, y);

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "Could not create event source")?;

    // Right mouse down
    let mouse_down = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::RightMouseDown,
        point,
        CGMouseButton::Right,
    )
    .map_err(|_| "Could not create mouse down event")?;

    mouse_down.post(CGEventTapLocation::HID);

    // Small delay
    std::thread::sleep(std::time::Duration::from_millis(10));

    // Right mouse up
    let mouse_up = CGEvent::new_mouse_event(
        source,
        CGEventType::RightMouseUp,
        point,
        CGMouseButton::Right,
    )
    .map_err(|_| "Could not create mouse up event")?;

    mouse_up.post(CGEventTapLocation::HID);

    log::info!("Right-click completed");
    Ok(())
}

/// Perform a double-click at a specific position
pub fn perform_double_click_at_position(x: f64, y: f64) -> Result<(), String> {
    use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use core_graphics::geometry::CGPoint;

    log::info!("Performing double-click at position ({}, {})", x, y);

    let point = CGPoint::new(x, y);

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "Could not create event source")?;

    // First click - down
    let mouse_down1 = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::LeftMouseDown,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| "Could not create mouse down event")?;
    mouse_down1.set_integer_value_field(
        core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE,
        1,
    );
    mouse_down1.post(CGEventTapLocation::HID);

    std::thread::sleep(std::time::Duration::from_millis(10));

    // First click - up
    let mouse_up1 = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::LeftMouseUp,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| "Could not create mouse up event")?;
    mouse_up1.set_integer_value_field(
        core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE,
        1,
    );
    mouse_up1.post(CGEventTapLocation::HID);

    std::thread::sleep(std::time::Duration::from_millis(50));

    // Second click - down (click count = 2)
    let mouse_down2 = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::LeftMouseDown,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| "Could not create mouse down event")?;
    mouse_down2.set_integer_value_field(
        core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE,
        2,
    );
    mouse_down2.post(CGEventTapLocation::HID);

    std::thread::sleep(std::time::Duration::from_millis(10));

    // Second click - up (click count = 2)
    let mouse_up2 = CGEvent::new_mouse_event(
        source,
        CGEventType::LeftMouseUp,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| "Could not create mouse up event")?;
    mouse_up2.set_integer_value_field(
        core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE,
        2,
    );
    mouse_up2.post(CGEventTapLocation::HID);

    log::info!("Double-click completed");
    Ok(())
}

/// Perform a Cmd+click at a specific position
pub fn perform_cmd_click_at_position(x: f64, y: f64) -> Result<(), String> {
    use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, CGMouseButton};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use core_graphics::geometry::CGPoint;

    log::info!("Performing Cmd+click at position ({}, {})", x, y);

    let point = CGPoint::new(x, y);

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "Could not create event source")?;

    // Mouse down with Cmd modifier
    let mouse_down = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::LeftMouseDown,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| "Could not create mouse down event")?;
    mouse_down.set_flags(CGEventFlags::CGEventFlagCommand);
    mouse_down.post(CGEventTapLocation::HID);

    std::thread::sleep(std::time::Duration::from_millis(10));

    // Mouse up with Cmd modifier
    let mouse_up = CGEvent::new_mouse_event(
        source,
        CGEventType::LeftMouseUp,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| "Could not create mouse up event")?;
    mouse_up.set_flags(CGEventFlags::CGEventFlagCommand);
    mouse_up.post(CGEventTapLocation::HID);

    log::info!("Cmd+click completed");
    Ok(())
}

/// Perform a right-click (context menu) action on an element
pub fn perform_right_click(element: &AXElementHandle) -> Result<(), String> {
    let action = CFString::new("AXShowMenu");
    let result = unsafe { AXUIElementPerformAction(element.as_ptr(), action.as_CFTypeRef()) };

    if result == 0 {
        log::info!("Successfully performed AXShowMenu action");
        Ok(())
    } else {
        Err(format!("AXShowMenu failed with error code: {}", result))
    }
}
