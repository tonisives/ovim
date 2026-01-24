//! AppleScript utilities for window management

use std::process::Command;

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

/// Find a window by title across all processes matching the given names (case-insensitive)
/// Returns 1-based window index if found
pub fn find_window_by_title(process_names: &[&str], title: &str) -> Option<usize> {
    let name_conditions: Vec<String> = process_names
        .iter()
        .map(|n| format!("name is \"{}\"", n))
        .collect();
    let condition = name_conditions.join(" or ");

    let script = format!(
        r#"
        tell application "System Events"
            repeat with p in (every process whose {})
                try
                    repeat with i from 1 to (count of windows of p)
                        set w to window i of p
                        if name of w contains "{}" then
                            return i
                        end if
                    end repeat
                end try
            end repeat
            return 0
        end tell
        "#,
        condition, title
    );

    let output = Command::new("osascript").arg("-e").arg(&script).output();

    if let Ok(out) = output {
        if out.status.success() {
            let index_str = String::from_utf8_lossy(&out.stdout);
            let index: usize = index_str.trim().parse().unwrap_or(0);
            if index > 0 {
                return Some(index);
            }
        }
    }
    None
}

/// Get the current position of a window by title
pub fn get_window_position_by_title(process_names: &[&str], title: &str) -> Option<(i32, i32)> {
    let name_conditions: Vec<String> = process_names
        .iter()
        .map(|n| format!("name is \"{}\"", n))
        .collect();
    let condition = name_conditions.join(" or ");

    let script = format!(
        r#"
        tell application "System Events"
            repeat with p in (every process whose {})
                try
                    repeat with w in windows of p
                        if name of w contains "{}" then
                            set pos to position of w
                            return (item 1 of pos as text) & "," & (item 2 of pos as text)
                        end if
                    end repeat
                end try
            end repeat
        end tell
        "#,
        condition, title
    );

    let output = Command::new("osascript").arg("-e").arg(&script).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let pos_str = String::from_utf8_lossy(&output.stdout);
    let pos_str = pos_str.trim();
    if pos_str.is_empty() {
        return None;
    }
    let (x_str, y_str) = pos_str.split_once(',')?;
    let x = x_str.trim().parse::<i32>().ok()?;
    let y = y_str.trim().parse::<i32>().ok()?;
    Some((x, y))
}

/// Set window bounds by title (position and size in one call)
pub fn set_window_bounds_by_title(process_names: &[&str], title: &str, x: i32, y: i32, width: u32, height: u32) {
    let name_conditions: Vec<String> = process_names
        .iter()
        .map(|n| format!("name is \"{}\"", n))
        .collect();
    let condition = name_conditions.join(" or ");

    let script = format!(
        r#"
        tell application "System Events"
            repeat with p in (every process whose {})
                try
                    repeat with w in windows of p
                        if name of w contains "{}" then
                            set position of w to {{{}, {}}}
                            set size of w to {{{}, {}}}
                            return "ok"
                        end if
                    end repeat
                end try
            end repeat
            return "no_window_found"
        end tell
        "#,
        condition, title, x, y, width, height
    );

    log::debug!(
        "Setting window '{}' to {}x{} at ({}, {})",
        title, width, height, x, y
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

/// Focus a window by process names and title (without bringing all app windows to front)
#[allow(dead_code)]
pub fn focus_window_by_title(process_names: &[&str], title: &str) {
    let name_conditions: Vec<String> = process_names
        .iter()
        .map(|n| format!("name is \"{}\"", n))
        .collect();
    let condition = name_conditions.join(" or ");

    let script = format!(
        r#"
        tell application "System Events"
            repeat with p in (every process whose {})
                try
                    repeat with w in windows of p
                        if name of w contains "{}" then
                            -- Raise just this window to the front
                            perform action "AXRaise" of w
                            -- Set frontmost to give keyboard focus
                            set frontmost of p to true
                            return "ok"
                        end if
                    end repeat
                end try
            end repeat
        end tell
        "#,
        condition, title
    );

    log::info!("Focusing window with title '{}'", title);

    let output = Command::new("osascript").arg("-e").arg(&script).output();

    if let Ok(out) = output {
        if !out.status.success() {
            log::error!(
                "Failed to focus window: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
    }
}

/// Convert pixel dimensions to approximate terminal cell dimensions
#[allow(dead_code)]
pub fn pixels_to_cells(width: u32, height: u32) -> (u32, u32) {
    // Approximate: 8px per column, 16px per row
    let cols = (width / 8).max(10);
    let rows = (height / 16).max(4);
    (cols, rows)
}
