//! Browser scripting via AppleScript to get focused element positions in web browsers

mod applescript;
mod javascript;
mod parsing;
mod types;

use std::process::Command;

use super::accessibility::ElementFrame;
pub use types::{detect_browser_type, BrowserType, CursorPosition, TextAndCursor};

use applescript::{
    build_element_rect_script, build_execute_script, execute_applescript,
    get_browser_window_bounds,
};
use javascript::{
    build_set_cursor_position_js, build_set_element_text_js, GET_CURSOR_POSITION_JS,
    GET_TEXT_AND_CURSOR_JS,
};
use parsing::{parse_cursor_position_json, parse_text_and_cursor_json, parse_viewport_frame_json};
use types::viewport_to_element_frame;

/// Set text on the focused element in a browser using AppleScript + JavaScript
/// Returns Ok(()) on success, Err with message on failure
pub fn set_browser_element_text(browser_type: BrowserType, text: &str) -> Result<(), String> {
    let js = build_set_element_text_js(text);
    let script = build_execute_script(browser_type, &js);

    // Debug: write script to file for inspection
    let _ = std::fs::write("/tmp/set_text_script.txt", &script);
    log::info!(
        "set_browser_element_text: browser={:?}, text_len={}, script_len={}",
        browser_type,
        text.len(),
        script.len()
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("Failed to execute AppleScript: {}", e))?;

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    log::info!(
        "set_browser_element_text: exit_code={}, stdout='{}', stderr='{}'",
        output.status,
        stdout,
        stderr
    );

    if !output.status.success() {
        return Err(format!("AppleScript failed: {}", stderr));
    }

    if stdout.starts_with("ok") {
        log::info!("Browser text sync succeeded: {}", stdout);
        Ok(())
    } else {
        Err(format!("JavaScript returned: {}", stdout))
    }
}

/// Get cursor position from the focused element in a browser
pub fn get_browser_cursor_position(browser_type: BrowserType) -> Option<CursorPosition> {
    let script = build_execute_script(browser_type, &GET_CURSOR_POSITION_JS);

    // Debug: write script to file for inspection
    let _ = std::fs::write("/tmp/cursor_script.txt", &script);

    let stdout = match execute_applescript(&script) {
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
    browser_type: BrowserType,
    line: usize,
    column: usize,
) -> Result<(), String> {
    let js = build_set_cursor_position_js(line, column);
    let script = build_execute_script(browser_type, &js);

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("Failed to execute AppleScript: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("AppleScript failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if stdout.starts_with("ok") {
        log::debug!("Set browser cursor position: {}", stdout);
        Ok(())
    } else {
        Err(format!("JavaScript returned: {}", stdout))
    }
}

/// Get text AND cursor position in a single JS call
/// This is more reliable than separate calls as cursor position won't be lost
pub fn get_browser_text_and_cursor(browser_type: BrowserType) -> Option<TextAndCursor> {
    let script = build_execute_script(browser_type, &GET_TEXT_AND_CURSOR_JS);

    let stdout = match execute_applescript(&script) {
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

/// Get the focused element frame from a browser using AppleScript
pub fn get_browser_element_frame(browser_type: BrowserType) -> Option<ElementFrame> {
    log::info!(
        "Attempting to get element frame from browser: {:?}",
        browser_type
    );

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
    let script = build_element_rect_script(browser_type);
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
