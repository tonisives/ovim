//! AppleScript utilities for window management

use std::process::Command;

use super::WindowGeometry;

/// Set window size using AppleScript
pub fn set_window_size(app_name: &str, width: u32, height: u32) {
    let script = format!(
        r#"
        tell application "System Events"
            tell process "{}"
                try
                    set size of front window to {{{}, {}}}
                end try
            end tell
        end tell
        "#,
        app_name, width, height
    );

    let _ = Command::new("osascript").arg("-e").arg(&script).output();
}

/// Move a window to a specific position using AppleScript
#[allow(dead_code)]
pub fn move_window_to_position(app_name: &str, x: i32, y: i32) {
    // Small delay to let the window appear
    std::thread::sleep(std::time::Duration::from_millis(200));

    let script = format!(
        r#"
        tell application "System Events"
            tell process "{}"
                try
                    set position of front window to {{{}, {}}}
                end try
            end tell
        end tell
        "#,
        app_name, x, y
    );

    let _ = Command::new("osascript").arg("-e").arg(&script).output();
}

/// Find Alacritty window index by title (returns 1-based index)
pub fn find_alacritty_window_by_title(title: &str) -> Option<usize> {
    // First, let's list all window titles to debug
    let list_script = r#"
        tell application "System Events"
            tell process "Alacritty"
                set windowNames to {}
                repeat with i from 1 to (count of windows)
                    set end of windowNames to name of window i
                end repeat
                return windowNames
            end tell
        end tell
        "#;

    if let Ok(out) = Command::new("osascript")
        .arg("-e")
        .arg(list_script)
        .output()
    {
        log::info!(
            "Alacritty window titles: {}",
            String::from_utf8_lossy(&out.stdout).trim()
        );
    }

    let script = format!(
        r#"
        tell application "System Events"
            tell process "Alacritty"
                set windowIndex to 0
                repeat with i from 1 to (count of windows)
                    set w to window i
                    if name of w contains "{}" then
                        return i
                    end if
                end repeat
                return 0
            end tell
        end tell
        "#,
        title
    );

    let output = Command::new("osascript").arg("-e").arg(&script).output();

    if let Ok(out) = output {
        if out.status.success() {
            let index_str = String::from_utf8_lossy(&out.stdout);
            let index: usize = index_str.trim().parse().unwrap_or(0);
            log::info!("Search for '{}' returned index: {}", title, index);
            if index > 0 {
                return Some(index);
            }
        }
    }
    None
}

/// Set window bounds by window index (1-based)
#[allow(dead_code)]
pub fn set_window_bounds_by_index(
    app_name: &str,
    index: usize,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) {
    let script = format!(
        r#"
        tell application "System Events"
            tell process "{}"
                if (count of windows) >= {} then
                    set w to window {}
                    set position of w to {{{}, {}}}
                    set size of w to {{{}, {}}}
                end if
            end tell
        end tell
        "#,
        app_name, index, index, x, y, width, height
    );

    log::info!(
        "Setting window {} index {} bounds: {}x{} at ({}, {})",
        app_name,
        index,
        width,
        height,
        x,
        y
    );

    let output = Command::new("osascript").arg("-e").arg(&script).output();

    if let Ok(out) = output {
        if !out.status.success() {
            log::error!(
                "AppleScript failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
    }
}

/// Focus an Alacritty window by index (without bringing all app windows to front)
pub fn focus_alacritty_window_by_index(index: usize) {
    // Use AXRaise to bring the specific window to front and give it keyboard focus.
    let script = format!(
        r#"
        tell application "System Events"
            tell process "Alacritty"
                if (count of windows) >= {} then
                    set w to window {}
                    -- Raise just this window to the front
                    perform action "AXRaise" of w
                    -- Return the window position for clicking
                    set winPos to position of w
                    return (item 1 of winPos) & "," & (item 2 of winPos)
                end if
            end tell
        end tell
        "#,
        index, index
    );

    log::info!("Focusing Alacritty window at index {}", index);

    let output = Command::new("osascript").arg("-e").arg(&script).output();

    if let Ok(out) = output {
        if out.status.success() {
            // Parse the position and click to give keyboard focus
            let pos_str = String::from_utf8_lossy(&out.stdout);
            let pos_str = pos_str.trim();
            if let Some((x_str, y_str)) = pos_str.split_once(',') {
                if let (Ok(x), Ok(y)) = (x_str.trim().parse::<i32>(), y_str.trim().parse::<i32>()) {
                    // Click inside the window to give it keyboard focus
                    click_at_position(x + 50, y + 50);
                }
            }
        } else {
            log::error!(
                "Failed to focus window: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
    }
}

/// Click at a screen position using CGEvent (gives keyboard focus to the window)
fn click_at_position(x: i32, y: i32) {
    use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use core_graphics::geometry::CGPoint;

    let point = CGPoint::new(x as f64, y as f64);

    if let Ok(source) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
        // Mouse down
        if let Ok(event) = CGEvent::new_mouse_event(
            source.clone(),
            CGEventType::LeftMouseDown,
            point,
            CGMouseButton::Left,
        ) {
            event.post(CGEventTapLocation::HID);
        }

        // Mouse up
        if let Ok(event) = CGEvent::new_mouse_event(
            source,
            CGEventType::LeftMouseUp,
            point,
            CGMouseButton::Left,
        ) {
            event.post(CGEventTapLocation::HID);
        }
    }
}

/// Set window bounds atomically (position and size in one call)
pub fn set_window_bounds_atomic(
    app_name: &str,
    index: usize,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) {
    // Set both position and size in a single script for speed
    let script = format!(
        r#"
        tell application "System Events"
            tell process "{}"
                if (count of windows) >= {} then
                    set w to window {}
                    set position of w to {{{}, {}}}
                    set size of w to {{{}, {}}}
                end if
            end tell
        end tell
        "#,
        app_name, index, index, x, y, width, height
    );

    log::info!(
        "Setting window {} index {} to {}x{} at ({}, {})",
        app_name,
        index,
        width,
        height,
        x,
        y
    );

    let output = Command::new("osascript").arg("-e").arg(&script).output();

    if let Ok(out) = output {
        if !out.status.success() {
            log::error!(
                "AppleScript set bounds failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
    }
}

/// Convert pixel dimensions to approximate terminal cell dimensions
#[allow(dead_code)]
pub fn pixels_to_cells(geometry: &WindowGeometry) -> (u32, u32) {
    // Approximate: 8px per column, 16px per row
    let cols = (geometry.width / 8).max(10);
    let rows = (geometry.height / 16).max(4);
    (cols, rows)
}
