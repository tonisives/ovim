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
        log::info!("Attempting combined text+cursor capture via JS");
        if let Some(result) = browser_scripting::get_browser_text_and_cursor(bt) {
            // Only use JS result if we actually got text
            // Otherwise fall back to clipboard-based capture (for non-CodeMirror editors like GitHub)
            if !result.text.is_empty() {
                log::info!("JS capture succeeded: {} chars, cursor={:?}", result.text.len(), result.cursor);

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
            log::info!("JS capture returned empty text, falling back to clipboard method");
        } else {
            log::info!("JS capture failed, falling back to clipboard method");
        }
    }

    // Fallback: capture cursor position BEFORE text capture (which may move cursor via Cmd+A)
    let cursor_position = if let Some(bt) = browser_type {
        log::info!("Attempting to capture browser cursor position");
        let pos = browser_scripting::get_browser_cursor_position(bt);
        match &pos {
            Some(p) => log::info!("Captured cursor position: line={}, col={}", p.line, p.column),
            None => log::info!("Failed to capture cursor position"),
        }
        pos
    } else {
        None
    };

    // If accessibility API didn't return element frame, try browser scripting for web text fields
    let element_frame = if initial_element_frame.is_none() {
        if let Some(bt) = browser_type {
            log::info!("Detected browser type {:?}, attempting browser scripting", bt);
            let browser_frame = browser_scripting::get_browser_element_frame(bt);
            log::info!("Browser scripting element frame: {:?}", browser_frame.as_ref().map(|f| (f.x, f.y, f.width, f.height)));
            browser_frame
        } else {
            None
        }
    } else {
        initial_element_frame
    };

    // Get text from the focused element
    let text = capture_text_content(browser_type);

    CaptureResult { text, element_frame, cursor_position, browser_type }
}

/// Capture text content from the focused element
fn capture_text_content(_browser_type: Option<BrowserType>) -> String {
    // First try accessibility API (doesn't cause beeps)
    let mut text = accessibility::get_focused_element_text().unwrap_or_default();

    let preview: String = text.lines().take(5).collect::<Vec<_>>().join("\\n");
    log::info!("Got text: {} chars, preview: {}", text.len(), preview);

    // If accessibility API failed/empty, try clipboard-based capture as fallback
    if text.is_empty() {
        log::info!("Accessibility text capture returned empty, trying clipboard-based capture");
        if let Some(captured) = capture_text_via_clipboard() {
            text = captured;
            log::info!("Captured {} chars via clipboard", text.len());
        }
    }

    text
}
