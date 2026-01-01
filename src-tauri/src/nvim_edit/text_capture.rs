//! Text capture from focused elements

use super::accessibility::{self, ElementFrame};
use super::browser_scripting::{self, BrowserType};
use super::clipboard::capture_text_via_clipboard;

/// Result of text capture with additional context
pub struct CaptureResult {
    pub text: String,
    pub element_frame: Option<ElementFrame>,
}

/// Capture text and element frame from the focused element
pub fn capture_text_and_frame(
    app_bundle_id: &str,
    initial_element_frame: Option<ElementFrame>,
) -> CaptureResult {
    // If accessibility API didn't return element frame, try browser scripting for web text fields
    let element_frame = if initial_element_frame.is_none() {
        if let Some(browser_type) = browser_scripting::detect_browser_type(app_bundle_id) {
            log::info!("Detected browser type {:?}, attempting browser scripting", browser_type);
            let browser_frame = browser_scripting::get_browser_element_frame(browser_type);
            log::info!("Browser scripting element frame: {:?}", browser_frame.as_ref().map(|f| (f.x, f.y, f.width, f.height)));
            browser_frame
        } else {
            None
        }
    } else {
        initial_element_frame
    };

    // Get text from the focused element
    let browser_type = browser_scripting::detect_browser_type(app_bundle_id);
    let text = capture_text_content(browser_type);

    CaptureResult { text, element_frame }
}

/// Capture text content from the focused element
fn capture_text_content(browser_type: Option<BrowserType>) -> String {
    // For browsers, use clipboard-based capture (Cmd+A, Cmd+C) as it's the most reliable
    // Both accessibility API and JavaScript DOM access can return mangled text for code editors
    let mut text = if browser_type.is_some() {
        log::info!("Browser detected, using clipboard-based text capture");
        capture_text_via_clipboard()
            .or_else(|| {
                log::info!("Clipboard capture failed, falling back to accessibility API");
                accessibility::get_focused_element_text()
            })
            .unwrap_or_default()
    } else {
        accessibility::get_focused_element_text().unwrap_or_default()
    };

    let preview: String = text.lines().take(5).collect::<Vec<_>>().join("\\n");
    log::info!("Got text: {} chars, preview: {}", text.len(), preview);

    // If still empty, try clipboard-based capture (for non-browser apps)
    if text.is_empty() {
        log::info!("Text capture returned empty, trying clipboard-based capture");
        if let Some(captured) = capture_text_via_clipboard() {
            text = captured;
            log::info!("Captured {} chars via clipboard", text.len());
        }
    }

    text
}
