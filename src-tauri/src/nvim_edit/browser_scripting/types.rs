//! Type definitions for browser scripting

use super::super::accessibility::ElementFrame;

/// Supported browser types for AppleScript scripting
#[derive(Debug, Clone, Copy)]
pub enum BrowserType {
    Safari,
    Chrome,
    Brave,
    Arc,
}

impl BrowserType {
    /// Get the application name for AppleScript
    pub fn app_name(&self) -> &'static str {
        match self {
            BrowserType::Safari => "Safari",
            BrowserType::Chrome => "Google Chrome",
            BrowserType::Brave => "Brave Browser",
            BrowserType::Arc => "Arc",
        }
    }
}

/// Browser bundle ID constants
pub const SAFARI_BUNDLE: &str = "com.apple.Safari";
pub const CHROME_BUNDLE: &str = "com.google.Chrome";
pub const ARC_BUNDLE: &str = "company.thebrowser.Browser";
pub const BRAVE_BUNDLE: &str = "com.brave.Browser";
pub const EDGE_BUNDLE: &str = "com.microsoft.edgemac";

/// Detect if a bundle ID corresponds to a scriptable browser
pub fn detect_browser_type(bundle_id: &str) -> Option<BrowserType> {
    match bundle_id {
        SAFARI_BUNDLE => Some(BrowserType::Safari),
        CHROME_BUNDLE | EDGE_BUNDLE => Some(BrowserType::Chrome),
        BRAVE_BUNDLE => Some(BrowserType::Brave),
        ARC_BUNDLE => Some(BrowserType::Arc),
        _ => None,
    }
}

/// Cursor position (0-based line and column)
#[derive(Debug, Clone, Copy, Default)]
pub struct CursorPosition {
    pub line: usize,
    pub column: usize,
}

/// Result of getting text and cursor in one call
pub struct TextAndCursor {
    pub text: String,
    pub cursor: Option<CursorPosition>,
}

/// Viewport-relative frame from browser JavaScript
pub struct ViewportFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub viewport_height: Option<f64>,
}

/// Convert ViewportFrame to ElementFrame using window bounds
pub fn viewport_to_element_frame(
    frame: ViewportFrame,
    window_x: f64,
    window_y: f64,
    window_height: f64,
) -> ElementFrame {
    let chrome_height = window_height - frame.viewport_height.unwrap_or(window_height);
    log::info!(
        "Chrome height: {} (window_h={}, viewport_h={})",
        chrome_height,
        window_height,
        frame.viewport_height.unwrap_or(0.0)
    );

    ElementFrame {
        x: window_x + frame.x,
        y: window_y + chrome_height + frame.y,
        width: frame.width,
        height: frame.height,
    }
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
            detect_browser_type("com.google.Chrome"),
            Some(BrowserType::Chrome)
        ));
        assert!(matches!(
            detect_browser_type("company.thebrowser.Browser"),
            Some(BrowserType::Arc)
        ));
        assert!(matches!(
            detect_browser_type("com.brave.Browser"),
            Some(BrowserType::Brave)
        ));
        assert!(detect_browser_type("org.mozilla.firefox").is_none());
        assert!(detect_browser_type("com.apple.TextEdit").is_none());
    }
}
