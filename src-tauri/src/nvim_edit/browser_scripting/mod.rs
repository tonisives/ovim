//! Browser scripting via AppleScript to get focused element positions in web browsers

mod applescript;
mod bmux;
mod javascript;
mod parsing;
mod types;

use super::accessibility::ElementFrame;
pub use types::{BrowserTarget, CursorPosition, TextAndCursor};

use applescript::{
    build_element_rect_script, build_execute_script, execute_applescript, get_browser_window_bounds,
};
use javascript::{
    build_set_cursor_position_js, build_set_element_text_js, GET_CURSOR_POSITION_JS,
    GET_TEXT_AND_CURSOR_JS,
};
use parsing::{parse_cursor_position_json, parse_text_and_cursor_json, parse_viewport_frame_json};
use types::viewport_to_element_frame;

pub fn detect_browser_target(bundle_id: &str) -> Option<BrowserTarget> {
    if bundle_id == types::BMUX_BUNDLE {
        return match bmux::focused_pane_id() {
            Ok(pane_id) => Some(BrowserTarget::Bmux { pane_id }),
            Err(error) => {
                log::warn!("Failed to identify focused bmux pane: {error}");
                None
            }
        };
    }
    types::detect_browser_type(bundle_id).map(BrowserTarget::AppleScript)
}

fn execute_javascript(target: &BrowserTarget, js: &str) -> Result<String, String> {
    match target {
        BrowserTarget::AppleScript(browser_type) => {
            let script = build_execute_script(*browser_type, js);
            execute_applescript(&script)
        }
        BrowserTarget::Bmux { pane_id } => bmux::execute_javascript(pane_id, js),
    }
}

/// Set text on the focused element in a browser using AppleScript + JavaScript
/// Returns Ok(Option<element_id>) on success, Err with message on failure
/// The element_id can be passed to subsequent calls to target the same element
pub fn set_browser_element_text(
    target: &BrowserTarget,
    text: &str,
    target_element_id: Option<&str>,
) -> Result<Option<String>, String> {
    if let BrowserTarget::Bmux { pane_id } = target {
        return bmux::replace_text(pane_id, text, target_element_id);
    }

    let js = build_set_element_text_js(text, target_element_id);
    log::info!(
        "set_browser_element_text: browser={:?}, text_len={}, target_id={:?}",
        target,
        text.len(),
        target_element_id
    );
    let stdout = execute_javascript(target, &js)?;

    if stdout.starts_with("ok") {
        log::info!("Browser text sync succeeded: {}", stdout);
        // Extract element ID if present (format: "ok_draftjs:element-id")
        let element_id = stdout
            .find(':')
            .map(|colon_pos| stdout[colon_pos + 1..].to_string());
        Ok(element_id)
    } else {
        Err(format!("JavaScript returned: {}", stdout))
    }
}

/// Get cursor position from the focused element in a browser
#[allow(dead_code)]
pub fn get_browser_cursor_position(target: &BrowserTarget) -> Option<CursorPosition> {
    let stdout = match execute_javascript(target, &GET_CURSOR_POSITION_JS) {
        Ok(s) => s,
        Err(e) => {
            log::info!("get_browser_cursor_position AppleScript failed: {}", e);
            return None;
        }
    };

    log::info!("get_browser_cursor_position raw output: '{}'", stdout);

    let cursor = parse_cursor_position_json(&stdout)?;
    log::info!(
        "Got browser cursor position: line={}, col={}",
        cursor.line,
        cursor.column
    );
    Some(cursor)
}

/// Set cursor position in the focused element in a browser
pub fn set_browser_cursor_position(
    target: &BrowserTarget,
    line: usize,
    column: usize,
) -> Result<(), String> {
    let js = build_set_cursor_position_js(line, column);
    let stdout = execute_javascript(target, &js)?;

    if stdout.starts_with("ok") {
        log::debug!("Set browser cursor position: {}", stdout);
        Ok(())
    } else {
        Err(format!("JavaScript returned: {}", stdout))
    }
}

/// Get text AND cursor position in a single JS call
/// This is more reliable than separate calls as cursor position won't be lost
pub fn get_browser_text_and_cursor(target: &BrowserTarget) -> Option<TextAndCursor> {
    let stdout = match execute_javascript(target, &GET_TEXT_AND_CURSOR_JS) {
        Ok(s) => s,
        Err(e) => {
            log::debug!("get_browser_text_and_cursor AppleScript failed: {}", e);
            return None;
        }
    };

    log::info!(
        "get_browser_text_and_cursor raw output length: {}",
        stdout.len()
    );

    let result = parse_text_and_cursor_json(&stdout)?;
    log::info!(
        "Got text ({} chars) and cursor ({:?}) via JS",
        result.text.len(),
        result.cursor
    );
    Some(result)
}

/// Get the hostname from the current browser tab
pub fn get_browser_hostname(target: &BrowserTarget) -> Option<String> {
    let js = "window.location.hostname";
    let stdout = match execute_javascript(target, js) {
        Ok(s) => s,
        Err(e) => {
            log::debug!("get_browser_hostname AppleScript failed: {}", e);
            return None;
        }
    };

    // Filter out error responses
    if stdout.is_empty()
        || stdout.starts_with("error")
        || stdout == "no_window"
        || stdout == "no_tab"
    {
        return None;
    }

    log::info!("Got browser hostname: {}", stdout);
    Some(stdout)
}

/// Get the focused element frame from a browser using AppleScript
pub fn get_browser_element_frame(target: &BrowserTarget) -> Option<ElementFrame> {
    log::info!("Attempting to get element frame from browser: {:?}", target);

    let BrowserTarget::AppleScript(browser_type) = target else {
        return None;
    };

    // Get window position and size from System Events
    let (window_x, window_y, _window_width, window_height) =
        get_browser_window_bounds(browser_type.app_name())?;
    log::info!(
        "Browser window bounds: x={}, y={}, h={}",
        window_x,
        window_y,
        window_height
    );

    // Get element's viewport-relative position via JavaScript
    let script = build_element_rect_script(*browser_type);
    let stdout = match execute_applescript(&script) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("AppleScript failed: {}", e);
            return None;
        }
    };

    log::info!("AppleScript returned: {}", stdout);

    // Handle "null" or empty response
    if stdout.is_empty() || stdout == "null" || stdout == "missing value" {
        log::info!("Browser returned no element frame");
        return None;
    }

    let frame = parse_viewport_frame_json(&stdout)?;
    Some(viewport_to_element_frame(
        frame,
        window_x,
        window_y,
        window_height,
    ))
}
