//! AppleScript builders for browser interaction

use super::javascript::GET_ELEMENT_RECT_JS;
use super::types::BrowserType;
use std::process::Command;

/// Build AppleScript for Safari to get element rect
pub fn build_safari_script() -> String {
    format!(
        r#"tell application "Safari"
    if (count of windows) = 0 then return "null"
    tell front window
        if (count of tabs) = 0 then return "null"
        try
            return do JavaScript "{}" in current tab
        on error
            return "null"
        end try
    end tell
end tell"#,
        (*GET_ELEMENT_RECT_JS).replace('"', "\\\"")
    )
}

/// Build AppleScript for Chrome-based browsers (Chrome, Arc, Brave, Edge)
pub fn build_chrome_script(app_name: &str) -> String {
    format!(
        r#"tell application "{}"
    if (count of windows) = 0 then return "null"
    tell active tab of front window
        try
            return execute javascript "{}"
        on error
            return "null"
        end try
    end tell
end tell"#,
        app_name,
        (*GET_ELEMENT_RECT_JS).replace('"', "\\\"")
    )
}

/// Build AppleScript to execute arbitrary JavaScript in Safari
pub fn build_safari_execute_script(js: &str) -> String {
    let js_escaped = js.replace('\r', "").replace('"', "\\\"");
    format!(
        r#"tell application "Safari"
    if (count of windows) = 0 then return "no_window"
    tell front window
        if (count of tabs) = 0 then return "no_tab"
        try
            return do JavaScript "{}" in current tab
        on error errMsg
            return "error: " & errMsg
        end try
    end tell
end tell"#,
        js_escaped
    )
}

/// Build AppleScript to execute arbitrary JavaScript in Chrome-based browsers
pub fn build_chrome_execute_script(app_name: &str, js: &str) -> String {
    let js_escaped = js.replace('\r', "").replace('"', "\\\"");
    format!(
        r#"tell application "{}"
    if (count of windows) = 0 then return "no_window"
    tell active tab of front window
        try
            return execute javascript "{}"
        on error errMsg
            return "error: " & errMsg
        end try
    end tell
end tell"#,
        app_name, js_escaped
    )
}

/// Build the appropriate AppleScript for the given browser type
pub fn build_execute_script(browser_type: BrowserType, js: &str) -> String {
    match browser_type {
        BrowserType::Safari => build_safari_execute_script(js),
        BrowserType::Chrome | BrowserType::Brave | BrowserType::Arc => {
            build_chrome_execute_script(browser_type.app_name(), js)
        }
    }
}

/// Build the appropriate AppleScript for getting element rect
pub fn build_element_rect_script(browser_type: BrowserType) -> String {
    match browser_type {
        BrowserType::Safari => build_safari_script(),
        BrowserType::Chrome | BrowserType::Brave | BrowserType::Arc => {
            build_chrome_script(browser_type.app_name())
        }
    }
}

/// Get the browser window's position and size using System Events
pub fn get_browser_window_bounds(app_name: &str) -> Option<(f64, f64, f64, f64)> {
    let script = format!(
        r#"tell application "System Events"
    set winPos to position of front window of process "{}"
    set winSize to size of front window of process "{}"
    return (item 1 of winPos as text) & "," & (item 2 of winPos as text) & "," & (item 1 of winSize as text) & "," & (item 2 of winSize as text)
end tell"#,
        app_name, app_name
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .ok()?;

    if !output.status.success() {
        log::warn!(
            "Failed to get window bounds: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = stdout.trim().split(',').collect();
    if parts.len() != 4 {
        log::warn!("Unexpected window bounds format: {}", stdout);
        return None;
    }

    let x: f64 = parts[0].parse().ok()?;
    let y: f64 = parts[1].parse().ok()?;
    let w: f64 = parts[2].parse().ok()?;
    let h: f64 = parts[3].parse().ok()?;

    Some((x, y, w, h))
}

/// Execute an AppleScript command and return output
pub fn execute_applescript(script: &str) -> Result<String, String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| format!("Failed to execute AppleScript: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("AppleScript failed: {}", stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
