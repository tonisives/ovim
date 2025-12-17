//! "Edit with Neovim" feature - open any text field in nvim via a keyboard shortcut

mod accessibility;
mod session;
mod terminal;

pub use session::EditSessionManager;

use crate::config::NvimEditSettings;
use crate::keyboard::{inject_key_press, KeyCode, Modifiers};
use std::process::Command;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Trigger the "Edit with Neovim" flow
pub fn trigger_nvim_edit(
    manager: Arc<EditSessionManager>,
    settings: NvimEditSettings,
) -> Result<(), String> {
    // 1. Capture focus context (which app we're in)
    let focus_context = accessibility::capture_focus_context()
        .ok_or("No focused application found")?;
    log::info!("Captured focus context: {:?}", focus_context);

    // 2. Get text from the focused element
    let text = accessibility::get_focused_element_text().unwrap_or_default();
    log::info!("Got text from focused element: {} chars", text.len());

    // 3. Start edit session (writes temp file, spawns terminal)
    let session_id = manager.start_session(focus_context, text.clone(), settings.clone())?;
    log::info!("Started edit session: {}", session_id);

    // 4. Spawn a thread to wait for nvim to exit and restore text
    let manager_clone = Arc::clone(&manager);
    thread::spawn(move || {
        // Wait for the terminal process to exit
        if let Some(session) = manager_clone.get_session(&session_id) {
            log::info!("Waiting for process: {:?} (PID: {:?})", session.terminal_type, session.process_id);

            // Wait for process
            if let Err(e) = terminal::wait_for_process(&session.terminal_type, session.process_id) {
                log::error!("Error waiting for terminal process: {}", e);
                manager_clone.cancel_session(&session_id);
                return;
            }

            log::info!("Terminal process exited, reading edited file");

            // Small delay to ensure file is written
            thread::sleep(Duration::from_millis(100));

            // Complete the session (read file, restore text)
            if let Err(e) = complete_edit_session(&manager_clone, &session_id, &session.original_text, &session.focus_context) {
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
fn complete_edit_session(
    manager: &EditSessionManager,
    session_id: &uuid::Uuid,
    original_text: &str,
    focus_context: &accessibility::FocusContext,
) -> Result<(), String> {
    // Read the temp file
    let session = manager.get_session(session_id)
        .ok_or("Session not found")?;

    log::info!("Reading temp file: {:?}", session.temp_file);

    let edited_text = std::fs::read_to_string(&session.temp_file)
        .map_err(|e| format!("Failed to read temp file: {}", e))?;

    log::info!("Read {} chars from temp file", edited_text.len());

    // Clean up temp file
    let _ = std::fs::remove_file(&session.temp_file);

    // Check if text changed (but always paste if original was empty - user might be creating new content)
    if !original_text.is_empty() && edited_text.trim() == original_text.trim() {
        log::info!("No changes made, skipping restoration");
        return Ok(());
    }

    log::info!("Restoring focus to app: {:?}", focus_context);

    // Restore focus to original app
    accessibility::restore_focus(focus_context)?;

    // Small delay for focus to settle
    thread::sleep(Duration::from_millis(200));

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
