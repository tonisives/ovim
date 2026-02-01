//! Text capture from focused elements

use super::accessibility::{self, ElementFrame};
use super::browser_scripting::{self, BrowserType, CursorPosition};
use super::clipboard::capture_text_via_clipboard;

/// Result of text capture with additional context
pub struct CaptureResult {
    pub text: String,
    pub element_frame: Option<ElementFrame>,
    /// Initial cursor position (0-based line and column) if available
    pub cursor_position: Option<CursorPosition>,
    /// Browser type if this is a browser
    pub browser_type: Option<BrowserType>,
}

/// Capture text and element frame from the focused element
/// If clipboard_mode is true, always use clipboard-based capture (Cmd+A, Cmd+C)
pub fn capture_text_and_frame(
    app_bundle_id: &str,
    initial_element_frame: Option<ElementFrame>,
    clipboard_mode: bool,
) -> CaptureResult {
    let browser_type = browser_scripting::detect_browser_type(app_bundle_id);

    // If clipboard_mode is enabled, skip smart detection and use clipboard directly
    if clipboard_mode {
        log::info!("Clipboard mode enabled, using Cmd+A/Cmd+C for text capture");
        let text = capture_text_via_clipboard().unwrap_or_default();
        log::info!("Clipboard capture: {} chars", text.len());

        return CaptureResult {
            text,
            element_frame: initial_element_frame,
            cursor_position: None, // No cursor tracking in clipboard mode
            browser_type: None,    // Disable browser-specific features
        };
    }

    // For browsers, try to get text AND cursor in one JS call
    // This is more reliable as cursor position won't be affected by text capture
    if let Some(bt) = browser_type {
        log::info!("Text capture: attempting JS capture for browser {:?}", bt);
        if let Some(result) = browser_scripting::get_browser_text_and_cursor(bt) {
            // Only use JS result if we actually got text
            // Otherwise fall back to clipboard-based capture (for non-CodeMirror editors like GitHub)
            if !result.text.is_empty() {
                log::info!("Text capture: JS succeeded, {} chars, cursor={:?}", result.text.len(), result.cursor);

                // Get element frame if needed
                let element_frame = if initial_element_frame.is_none() {
                    log::info!("Getting element frame via browser scripting");
                    browser_scripting::get_browser_element_frame(bt)
                } else {
                    initial_element_frame
                };

                return CaptureResult {
                    text: result.text,
                    element_frame,
                    cursor_position: result.cursor,
                    browser_type: Some(bt),
                };
            }
            log::info!("Text capture: JS returned empty text, falling back to clipboard");
        } else {
            log::info!("Text capture: JS failed, falling back to clipboard");
        }
    }

    // JS capture failed - fall back to clipboard mode
    // IMPORTANT: Set browser_type to None since JS-based live sync won't work
    // This ensures we use clipboard-based restoration instead of failing JS calls
    log::info!("JS capture failed, disabling browser_type for this session (will use clipboard mode)");

    // Get element frame from browser scripting (this can still work for positioning)
    let element_frame = if initial_element_frame.is_none() {
        if let Some(bt) = browser_type {
            log::info!("Getting element frame via browser scripting");
            let browser_frame = browser_scripting::get_browser_element_frame(bt);
            log::info!("Browser scripting element frame: {:?}", browser_frame.as_ref().map(|f| (f.x, f.y, f.width, f.height)));
            browser_frame
        } else {
            None
        }
    } else {
        initial_element_frame
    };

    // Get text from the focused element, tracking whether we used clipboard
    let (text, _used_clipboard, is_address_bar) = capture_text_content_with_source();

    // If we're in a browser's address bar, disable browser live sync
    // to avoid updating web page elements when editing the URL
    let effective_browser_type = if browser_type.is_some() && is_address_bar {
        log::info!("Browser address bar detected - disabling live sync to avoid updating web content");
        None
    } else {
        // Return with browser_type = None since JS-based features won't work
        None
    };

    CaptureResult { text, element_frame, cursor_position: None, browser_type: effective_browser_type }
}

/// Check if the focused element is the browser's address bar (URL field)
/// Returns true if it's the address bar, false otherwise
fn is_browser_address_bar() -> bool {
    let role = accessibility::get_focused_element_role();
    let subrole = accessibility::get_focused_element_subrole();
    log::info!("Focused element role: {:?}, subrole: {:?}", role, subrole);

    // Browser address bar: AXTextField with no subrole
    // Web content text fields: AXTextArea (multiline) or AXTextField with specific subroles
    //
    // Reddit comment field: AXTextArea
    // Chrome/Brave address bar: AXTextField (no subrole)
    if let Some(r) = &role {
        if r == "AXTextField" {
            // AXTextField could be address bar or a web form input
            // Address bar has no subrole, web inputs often have subroles
            // For safety, treat AXTextField without subrole as address bar in browsers
            if subrole.is_none() {
                return true;
            }
        }
    }

    false
}

/// Capture text content from the focused element
/// Returns (text, used_clipboard, is_address_bar)
fn capture_text_content_with_source() -> (String, bool, bool) {
    // Check if we're in the browser address bar before capturing
    let is_address_bar = is_browser_address_bar();

    // First try accessibility API (doesn't cause beeps)
    let text = accessibility::get_focused_element_text().unwrap_or_default();

    let preview: String = text.lines().take(5).collect::<Vec<_>>().join("\\n");
    log::info!("Got text: {} chars, preview: {}", text.len(), preview);

    // If accessibility API failed/empty, try clipboard-based capture as fallback
    if text.is_empty() {
        log::info!("Accessibility text capture returned empty, trying clipboard-based capture");
        if let Some(captured) = capture_text_via_clipboard() {
            log::info!("Captured {} chars via clipboard", captured.len());
            return (captured, true, is_address_bar);
        }
    }

    (text, false, is_address_bar)
}
