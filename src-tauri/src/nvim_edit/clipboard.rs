//! Clipboard operations for text capture and restoration

use crate::keyboard::{inject_key_press, KeyCode, Modifiers};
use std::process::Command;
use std::thread;
use std::time::Duration;

/// Replace text in the focused field using clipboard
pub fn replace_text_via_clipboard(text: &str) -> Result<(), String> {
    log::info!("Saving current clipboard and setting new content ({} chars)", text.len());

    // Save current clipboard
    let original_clipboard = get_clipboard_content();

    // Set new clipboard content
    set_clipboard_content(text)?;

    log::info!("Clipboard set, now sending Cmd+A");

    // Select all and paste
    thread::sleep(Duration::from_millis(100));
    inject_key_press(
        KeyCode::A,
        Modifiers { command: true, ..Default::default() },
    )?;

    log::info!("Sent Cmd+A, now sending Cmd+V");

    thread::sleep(Duration::from_millis(100));
    inject_key_press(
        KeyCode::V,
        Modifiers { command: true, ..Default::default() },
    )?;

    log::info!("Sent Cmd+V");

    // Restore original clipboard after a delay
    if let Some(original) = original_clipboard {
        restore_clipboard_async(original);
    }

    Ok(())
}

/// Capture text from focused element via clipboard (fallback for web text fields)
pub fn capture_text_via_clipboard() -> Option<String> {
    // Save current clipboard
    let original_clipboard = get_clipboard_content();

    // Clear clipboard with a unique marker to detect if copy actually worked
    let marker = "\x00__OVIM_EMPTY_MARKER__\x00";
    let _ = set_clipboard_content(marker);

    thread::sleep(Duration::from_millis(50));

    // Select all (Cmd+A)
    if inject_key_press(
        KeyCode::A,
        Modifiers { command: true, ..Default::default() },
    ).is_err() {
        return None;
    }

    thread::sleep(Duration::from_millis(50));

    // Copy (Cmd+C)
    if inject_key_press(
        KeyCode::C,
        Modifiers { command: true, ..Default::default() },
    ).is_err() {
        return None;
    }

    thread::sleep(Duration::from_millis(100));

    // Read clipboard
    let captured_text = get_clipboard_content();

    // Deselect by pressing Right arrow (moves cursor to end of selection)
    let _ = inject_key_press(
        KeyCode::Right,
        Modifiers::default(),
    );

    // Restore original clipboard
    if let Some(original) = original_clipboard {
        restore_clipboard_async(original);
    }

    // If clipboard still contains our marker, the field was empty
    captured_text.filter(|text| text != marker)
}

/// Get current clipboard content
fn get_clipboard_content() -> Option<String> {
    Command::new("pbpaste")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
}

/// Set clipboard content
fn set_clipboard_content(text: &str) -> Result<(), String> {
    let mut pbcopy = Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn pbcopy: {}", e))?;

    if let Some(mut stdin) = pbcopy.stdin.take() {
        use std::io::Write;
        stdin.write_all(text.as_bytes())
            .map_err(|e| format!("Failed to write to pbcopy: {}", e))?;
    }
    pbcopy.wait().map_err(|e| format!("pbcopy failed: {}", e))?;
    Ok(())
}

/// Restore clipboard content asynchronously after a delay
fn restore_clipboard_async(content: String) {
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(500));
        let _ = Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut p| {
                if let Some(mut stdin) = p.stdin.take() {
                    use std::io::Write;
                    let _ = stdin.write_all(content.as_bytes());
                }
                p.wait()
            });
    });
}
