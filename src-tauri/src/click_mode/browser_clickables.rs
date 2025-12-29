//! Browser-specific clickable element detection via JavaScript injection
//!
//! Chromium-based browsers (Brave, Chrome, Arc, Edge) don't expose web content
//! well through the accessibility API. This module uses AppleScript + JavaScript
//! to query clickable elements directly from the DOM.

use std::process::Command;

/// Browser types we support for JavaScript injection
#[derive(Debug, Clone, Copy)]
pub enum BrowserType {
    Safari,
    Chrome,
    Brave,
    Arc,
}

impl BrowserType {
    /// Get the application name for AppleScript
    fn app_name(&self) -> &'static str {
        match self {
            BrowserType::Safari => "Safari",
            BrowserType::Chrome => "Google Chrome",
            BrowserType::Brave => "Brave Browser",
            BrowserType::Arc => "Arc",
        }
    }

    /// Check if this browser needs JavaScript injection for web content
    pub fn needs_js_injection(&self) -> bool {
        // Safari exposes web content through accessibility, others don't
        !matches!(self, BrowserType::Safari)
    }
}

/// Browser bundle ID constants
pub const SAFARI_BUNDLE: &str = "com.apple.Safari";
pub const CHROME_BUNDLE: &str = "com.google.Chrome";
pub const ARC_BUNDLE: &str = "company.thebrowser.Browser";
pub const BRAVE_BUNDLE: &str = "com.brave.Browser";
pub const EDGE_BUNDLE: &str = "com.microsoft.edgemac";

/// Detect browser type from bundle ID
pub fn detect_browser_type(bundle_id: &str) -> Option<BrowserType> {
    match bundle_id {
        SAFARI_BUNDLE => Some(BrowserType::Safari),
        CHROME_BUNDLE | EDGE_BUNDLE => Some(BrowserType::Chrome),
        BRAVE_BUNDLE => Some(BrowserType::Brave),
        ARC_BUNDLE => Some(BrowserType::Arc),
        _ => None,
    }
}

/// A clickable element from the web page
#[derive(Debug, Clone, serde::Deserialize)]
pub struct WebClickable {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub tag: String,
    pub text: String,
}

/// JavaScript to query all clickable elements from the page
/// Note: Uses double quotes which get escaped for AppleScript
const GET_CLICKABLES_JS: &str = r#"(function(){var r=[];var seen=new Set();var s="a[href],button,input,textarea,select,[role=button],[role=link],[onclick],[tabindex]";var els=document.querySelectorAll(s);for(var i=0;i<els.length&&r.length<200;i++){var el=els[i];var rect=el.getBoundingClientRect();if(rect.width<=0||rect.height<=0)continue;if(rect.top>window.innerHeight||rect.bottom<0)continue;if(rect.left>window.innerWidth||rect.right<0)continue;var k=Math.round(rect.left)+","+Math.round(rect.top);if(seen.has(k))continue;seen.add(k);var t=el.textContent||el.value||el.placeholder||"";t=t.trim().substring(0,50);r.push({x:rect.left,y:rect.top,width:rect.width,height:rect.height,tag:el.tagName.toLowerCase(),text:t});}return JSON.stringify(r);})()"#;

/// Get the browser window's viewport offset (chrome height)
fn get_browser_viewport_info(app_name: &str) -> Option<(f64, f64, f64)> {
    // Get window position and size from System Events
    let script = format!(
        r#"tell application "System Events"
    set winPos to position of front window of process "{}"
    set winSize to size of front window of process "{}"
    return (item 1 of winPos as text) & "," & (item 2 of winPos as text) & "," & (item 2 of winSize as text)
end tell"#,
        app_name, app_name
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = stdout.trim().split(',').collect();
    if parts.len() != 3 {
        return None;
    }

    let x: f64 = parts[0].parse().ok()?;
    let y: f64 = parts[1].parse().ok()?;
    let height: f64 = parts[2].parse().ok()?;

    Some((x, y, height))
}

/// Get viewport height from browser via JavaScript
fn get_viewport_height(browser_type: BrowserType) -> Option<f64> {
    let js = "window.innerHeight";
    let script = match browser_type {
        BrowserType::Safari => build_safari_script(js),
        BrowserType::Chrome | BrowserType::Brave | BrowserType::Arc => {
            build_chrome_script(browser_type.app_name(), js)
        }
    };

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    stdout.parse().ok()
}

/// Build AppleScript for Safari JavaScript execution
fn build_safari_script(js: &str) -> String {
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
        js.replace('"', "\\\"").replace('\n', "")
    )
}

/// Build AppleScript for Chrome-based browsers
fn build_chrome_script(app_name: &str, js: &str) -> String {
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
        js.replace('"', "\\\"").replace('\n', "")
    )
}

/// Query clickable elements from the browser's web content
pub fn get_browser_clickables(browser_type: BrowserType) -> Result<Vec<WebClickable>, String> {
    log::info!("Querying web clickables from {:?}", browser_type);

    // Get window position info
    let (win_x, win_y, win_height) = get_browser_viewport_info(browser_type.app_name())
        .ok_or("Failed to get browser window info")?;

    // Get viewport height to calculate chrome height
    let viewport_height = get_viewport_height(browser_type).unwrap_or(win_height);
    let chrome_height = win_height - viewport_height;

    log::info!(
        "Browser window: x={}, y={}, win_h={}, viewport_h={}, chrome_h={}",
        win_x,
        win_y,
        win_height,
        viewport_height,
        chrome_height
    );

    // Execute JavaScript to get clickables
    let script = match browser_type {
        BrowserType::Safari => build_safari_script(GET_CLICKABLES_JS),
        BrowserType::Chrome | BrowserType::Brave | BrowserType::Arc => {
            build_chrome_script(browser_type.app_name(), GET_CLICKABLES_JS)
        }
    };

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

    log::debug!("AppleScript raw output (first 500 chars): {}", &stdout[..stdout.len().min(500)]);

    if stdout.is_empty() || stdout == "null" || stdout == "missing value" {
        log::info!("No clickables returned from browser (stdout was: '{}')", stdout);
        return Ok(Vec::new());
    }

    // Parse JSON results
    let clickables: Vec<WebClickable> = serde_json::from_str(&stdout)
        .map_err(|e| format!("Failed to parse clickables JSON: {} (stdout: {})", e, &stdout[..stdout.len().min(200)]))?;

    log::info!("Found {} raw web clickables", clickables.len());

    // Convert viewport-relative coordinates to screen coordinates
    let screen_clickables: Vec<WebClickable> = clickables
        .into_iter()
        .map(|mut c| {
            c.x += win_x;
            c.y += win_y + chrome_height;
            c
        })
        .collect();

    log::info!(
        "Converted {} web clickables to screen coordinates",
        screen_clickables.len()
    );

    Ok(screen_clickables)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_browser_type() {
        assert!(matches!(
            detect_browser_type("com.apple.Safari"),
            Some(BrowserType::Safari)
        ));
        assert!(matches!(
            detect_browser_type("com.brave.Browser"),
            Some(BrowserType::Brave)
        ));
        assert!(matches!(
            detect_browser_type("com.google.Chrome"),
            Some(BrowserType::Chrome)
        ));
        assert!(detect_browser_type("com.example.unknown").is_none());
    }

    #[test]
    fn test_needs_js_injection() {
        assert!(!BrowserType::Safari.needs_js_injection());
        assert!(BrowserType::Brave.needs_js_injection());
        assert!(BrowserType::Chrome.needs_js_injection());
        assert!(BrowserType::Arc.needs_js_injection());
    }
}
