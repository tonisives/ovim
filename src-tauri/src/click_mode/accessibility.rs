//! Accessibility API integration for Click Mode
//!
//! Uses macOS Accessibility API to discover clickable UI elements
//! in the frontmost application.

use std::sync::{Mutex, OnceLock};
use std::time::Instant;

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

/// Configurable timing settings (updated from user settings)
static TIMING_SETTINGS: OnceLock<Mutex<TimingSettings>> = OnceLock::new();

struct TimingSettings {
    cache_ttl_ms: u128,
    ax_delay_ms: u32,
    max_depth: u32,
    max_elements: u32,
}

impl Default for TimingSettings {
    fn default() -> Self {
        Self {
            cache_ttl_ms: 500,
            ax_delay_ms: 10,
            max_depth: 10,
            max_elements: 500,
        }
    }
}

fn get_timing_settings() -> &'static Mutex<TimingSettings> {
    TIMING_SETTINGS.get_or_init(|| Mutex::new(TimingSettings::default()))
}

/// Update timing settings from user configuration
pub fn update_timing_settings(cache_ttl_ms: u32, ax_delay_ms: u32, max_depth: u32, max_elements: u32) {
    if let Ok(mut settings) = get_timing_settings().lock() {
        settings.cache_ttl_ms = cache_ttl_ms as u128;
        settings.ax_delay_ms = ax_delay_ms;
        settings.max_depth = max_depth;
        settings.max_elements = max_elements;
        log::info!("Updated click mode settings: cache_ttl={}ms, ax_delay={}ms, max_depth={}, max_elements={}",
            cache_ttl_ms, ax_delay_ms, max_depth, max_elements);
    }
}

fn get_cache() -> &'static Mutex<Option<ElementCache>> {
    ELEMENT_CACHE.get_or_init(|| Mutex::new(None))
}

/// Check if we have valid cached elements for the given PID
fn get_cached_elements(pid: i32) -> Option<(Vec<RawElementData>, bool)> {
    let cache_ttl = get_timing_settings()
        .lock()
        .map(|s| s.cache_ttl_ms)
        .unwrap_or(500);

    let cache = get_cache().lock().ok()?;
    let cached = cache.as_ref()?;

    // Check if cache is for the right PID and not expired
    if cached.pid == pid && cached.timestamp.elapsed().as_millis() < cache_ttl {
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
    let start = Instant::now();

    let helper_path = match get_helper_binary_path() {
        Some(p) => p,
        None => {
            log::error!("Helper binary not found - click mode cannot work safely");
            return Err("Helper binary not found. Please reinstall ovim.".to_string());
        }
    };

    // Get settings
    let (delay_ms, max_depth, max_elements) = get_timing_settings()
        .lock()
        .map(|s| (s.ax_delay_ms, s.max_depth, s.max_elements))
        .unwrap_or((10, 30, 500));

    log::info!("[TIMING] helper_path lookup: {}ms", start.elapsed().as_millis());

    // Run the helper subprocess - single attempt for speed, retry only on failure
    let subprocess_start = Instant::now();
    let output = std::process::Command::new(&helper_path)
        .arg(pid.to_string())
        .arg(delay_ms.to_string())
        .arg(max_depth.to_string())
        .arg(max_elements.to_string())
        .output();

    log::info!("[TIMING] subprocess execution: {}ms", subprocess_start.elapsed().as_millis());

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            log::error!("Failed to run helper: {}", e);
            return Err(format!("Failed to run helper: {}", e));
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log::error!("Helper subprocess failed: {}", stderr.trim());
        return Err(format!("Helper failed: {}", stderr.trim()));
    }

    let parse_start = Instant::now();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let helper_output: HelperOutput = serde_json::from_str(&stdout)
        .map_err(|e| format!("Failed to parse helper output: {}", e))?;

    log::info!("[TIMING] JSON parsing: {}ms", parse_start.elapsed().as_millis());

    let is_modal = helper_output.is_modal;
    log::info!("Found {} raw clickable elements via subprocess (is_modal: {})",
        helper_output.elements.len(), is_modal);

    // Cache the results
    cache_elements(pid, helper_output.elements.clone(), is_modal);

    log::info!("[TIMING] total subprocess fn: {}ms", start.elapsed().as_millis());

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
    // NOTE: Browser JS query must ALWAYS run (even on cache hit) because the cache
    // only stores AX elements, not browser elements. Otherwise we get single-char hints
    // that conflict with multi-char hints when browser elements are added.
    let (mut all_elements, is_modal, web_clickables) = if let Some((cached_els, cached_modal)) = cached {
        log::info!("[TIMING] Cache hit! Using {} cached elements ({}ms)", cached_els.len(), start.elapsed().as_millis());
        // Even on cache hit, we need to fetch browser elements if this is a browser app
        let browser_els = if let Some(bt) = browser_type {
            if bt.needs_js_injection() && !cached_modal {
                log::info!("[TIMING] Cache hit but fetching browser elements for browser app");
                super::browser_clickables::get_browser_clickables(bt).ok()
            } else {
                None
            }
        } else {
            None
        };
        (cached_els, cached_modal, browser_els)
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

    // Log hint length for debugging prefix conflicts
    if let Some(first_hint) = hints.first() {
        log::info!("Hint length: {} chars (first hint: '{}', count: {})",
            first_hint.len(), first_hint, hints.len());
    }

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

// Re-export mouse click functions for backwards compatibility
pub use super::mouse::click_at as perform_click_at_position;
pub use super::mouse::right_click_at as perform_right_click_at_position;
pub use super::mouse::double_click_at as perform_double_click_at_position;
pub use super::mouse::cmd_click_at as perform_cmd_click_at_position;
