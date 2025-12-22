//! "Edit with Neovim" feature - open any text field in nvim via a keyboard shortcut

mod accessibility;
mod browser_scripting;
mod session;
pub mod terminals;

pub use session::EditSessionManager;

use crate::config::NvimEditSettings;
use crate::keyboard::{inject_key_press, KeyCode, Modifiers};
use std::process::Command;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use terminals::WindowGeometry;

/// Trigger the "Edit with Neovim" flow
pub fn trigger_nvim_edit(
    manager: Arc<EditSessionManager>,
    settings: NvimEditSettings,
) -> Result<(), String> {
    // 1. Capture focus context (which app we're in)
    let focus_context = accessibility::capture_focus_context()
        .ok_or("No focused application found")?;
    log::info!("Captured focus context: {:?}", focus_context);

    // 2. Capture geometry info BEFORE any clipboard operations (which may change focus)
    log::info!("popup_mode={}, popup_width={}, popup_height={}", settings.popup_mode, settings.popup_width, settings.popup_height);
    let element_frame = accessibility::get_focused_element_frame();
    let window_frame = accessibility::get_focused_window_frame();
    log::info!("Element frame from accessibility: {:?}", element_frame.as_ref().map(|f| (f.x, f.y, f.width, f.height)));
    log::info!("Window frame: {:?}", window_frame.as_ref().map(|f| (f.x, f.y, f.width, f.height)));

    // If accessibility API didn't return element frame, try browser scripting for web text fields
    let element_frame = if element_frame.is_none() {
        if let Some(browser_type) = browser_scripting::detect_browser_type(&focus_context.app_bundle_id) {
            log::info!("Detected browser type {:?}, attempting browser scripting", browser_type);
            let browser_frame = browser_scripting::get_browser_element_frame(browser_type);
            log::info!("Browser scripting element frame: {:?}", browser_frame.as_ref().map(|f| (f.x, f.y, f.width, f.height)));
            browser_frame
        } else {
            None
        }
    } else {
        element_frame
    };

    // 3. Get text from the focused element (try accessibility first, then clipboard fallback)
    let mut text = accessibility::get_focused_element_text().unwrap_or_default();
    log::info!("Got text from accessibility API: {} chars", text.len());

    // If accessibility returned empty, try clipboard-based capture (for web text fields)
    if text.is_empty() {
        log::info!("Accessibility returned empty, trying clipboard-based capture");
        if let Some(captured) = capture_text_via_clipboard() {
            text = captured;
            log::info!("Captured {} chars via clipboard", text.len());
        }
    }

    // 4. Calculate window geometry if popup mode is enabled
    let geometry = if settings.popup_mode {
        // Try to get element frame from accessibility API
        let frame_geometry = element_frame.map(|frame| {
            // Position window below the text field with a small gap
            let x = frame.x as i32;
            let y = (frame.y + frame.height) as i32 + 5;

            // Use configured width, or match text field width (min 400)
            let width = if settings.popup_width > 0 {
                settings.popup_width
            } else {
                (frame.width as u32).max(400)
            };

            let height = settings.popup_height;

            log::info!("Using element frame geometry: x={}, y={}, w={}, h={}", x, y, width, height);
            WindowGeometry { x, y, width, height }
        });

        // If element frame not available (e.g., web views), center in the focused window
        let result = frame_geometry.or_else(|| {
            window_frame.map(|wf| {
                let width = if settings.popup_width > 0 {
                    settings.popup_width
                } else {
                    500 // Default width for web views
                };

                let height = settings.popup_height;

                // Center popup in the focused window
                let x = (wf.x + (wf.width - width as f64) / 2.0) as i32;
                let y = (wf.y + (wf.height - height as f64) / 2.0) as i32;

                log::info!("Using window frame geometry (centered): x={}, y={}, w={}, h={}", x, y, width, height);
                WindowGeometry { x, y, width, height }
            })
        });

        if result.is_none() {
            log::warn!("No geometry available - window will open at default size/position");
        }

        result
    } else {
        log::info!("popup_mode is disabled");
        None
    };

    log::info!("Final geometry: {:?}", geometry);

    // 4. Start edit session (writes temp file, spawns terminal)
    let session_id = manager.start_session(focus_context, text.clone(), settings.clone(), geometry)?;
    log::info!("Started edit session: {}", session_id);

    // 4. Spawn a thread to wait for nvim to exit and restore text
    let manager_clone = Arc::clone(&manager);
    thread::spawn(move || {
        // Wait for the terminal process to exit
        if let Some(session) = manager_clone.get_session(&session_id) {
            log::info!("Waiting for process: {:?} (PID: {:?})", session.terminal_type, session.process_id);

            // Wait for process
            if let Err(e) = terminals::wait_for_process(&session.terminal_type, session.process_id) {
                log::error!("Error waiting for terminal process: {}", e);
                manager_clone.cancel_session(&session_id);
                return;
            }

            log::info!("Terminal process exited, reading edited file");

            // Restore focus to the original app immediately
            // This races with alacritty window closing - we want to activate the target
            // app before macOS can auto-switch to another alacritty window
            log::info!("Restoring focus immediately");
            if let Err(e) = accessibility::restore_focus(&session.focus_context) {
                log::error!("Error restoring focus: {}", e);
            }

            // Small delay to ensure file is written and focus is settled
            thread::sleep(Duration::from_millis(100));

            // Complete the session (read file, restore text) - focus already restored above
            if let Err(e) = complete_edit_session_no_focus(&manager_clone, &session_id, &session.original_text) {
                log::error!("Error completing edit session: {}", e);
            }

            // Clean up
            manager_clone.remove_session(&session_id);
        } else {
            log::error!("Session not found: {}", session_id);
        }
    });

    Ok(())
}

/// Complete the edit session: read edited text and restore to original field
/// Note: Focus should already be restored before calling this function
fn complete_edit_session_no_focus(
    manager: &EditSessionManager,
    session_id: &uuid::Uuid,
    _original_text: &str,
) -> Result<(), String> {
    // Read the temp file
    let session = manager.get_session(session_id)
        .ok_or("Session not found")?;

    log::info!("Reading temp file: {:?}", session.temp_file);

    // Check if file was modified by comparing modification times
    let current_mtime = std::fs::metadata(&session.temp_file)
        .and_then(|m| m.modified())
        .map_err(|e| format!("Failed to get current file mtime: {}", e))?;

    if current_mtime == session.file_mtime {
        log::info!("File not modified (nvim quit without saving), skipping restoration");
        // Clean up temp file
        let _ = std::fs::remove_file(&session.temp_file);
        return Ok(());
    }

    let edited_text = std::fs::read_to_string(&session.temp_file)
        .map_err(|e| format!("Failed to read temp file: {}", e))?;

    log::info!("Read {} chars from temp file", edited_text.len());

    // Clean up temp file
    let _ = std::fs::remove_file(&session.temp_file);

    // Small delay for focus to settle (focus was restored before this call)
    thread::sleep(Duration::from_millis(100));

    log::info!("Replacing text via clipboard");

    // Replace text via clipboard
    replace_text_via_clipboard(&edited_text)?;

    log::info!("Successfully restored edited text");
    Ok(())
}

/// Replace text in the focused field using clipboard
fn replace_text_via_clipboard(text: &str) -> Result<(), String> {
    log::info!("Saving current clipboard and setting new content ({} chars)", text.len());

    // Save current clipboard
    let original_clipboard = Command::new("pbpaste")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok());

    // Set new clipboard content
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
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(500));
            let _ = Command::new("pbcopy")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .and_then(|mut p| {
                    if let Some(mut stdin) = p.stdin.take() {
                        use std::io::Write;
                        let _ = stdin.write_all(original.as_bytes());
                    }
                    p.wait()
                });
        });
    }

    Ok(())
}

/// Capture text from focused element via clipboard (fallback for web text fields)
fn capture_text_via_clipboard() -> Option<String> {
    // Save current clipboard
    let original_clipboard = Command::new("pbpaste")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok());

    // Clear clipboard with a unique marker to detect if copy actually worked
    let marker = "\x00__OVIM_EMPTY_MARKER__\x00";
    let _ = Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut p| {
            if let Some(mut stdin) = p.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(marker.as_bytes());
            }
            p.wait()
        });

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
    let captured_text = Command::new("pbpaste")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok());

    // Deselect by pressing Right arrow (moves cursor to end of selection)
    let _ = inject_key_press(
        KeyCode::Right,
        Modifiers::default(),
    );

    // Restore original clipboard
    if let Some(original) = original_clipboard {
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            let _ = Command::new("pbcopy")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .and_then(|mut p| {
                    if let Some(mut stdin) = p.stdin.take() {
                        use std::io::Write;
                        let _ = stdin.write_all(original.as_bytes());
                    }
                    p.wait()
                });
        });
    }

    // If clipboard still contains our marker, the field was empty
    captured_text.filter(|text| text != marker)
}
