//! Scroll mode keyboard handler
//!
//! Handles keyboard events for scroll mode (Vimium-style navigation).

use crate::keyboard::keycode::KeyCode;
use crate::keyboard::KeyEvent;
use crate::scroll_mode::{ScrollResult, SharedScrollModeState};

/// Handle a key event in scroll mode
///
/// Returns `None` to suppress the key, `Some(event)` to pass it through.
pub fn handle_scroll_mode_key(
    event: KeyEvent,
    scroll_state: &SharedScrollModeState,
    scroll_step: u32,
) -> Option<KeyEvent> {
    // Only process key down events
    if !event.is_key_down {
        // Suppress key up for keys we handled on key down
        // For simplicity, we'll check if it's a scroll key and suppress
        if let Some(keycode) = KeyCode::from_raw(event.code) {
            if is_scroll_key(keycode, event.modifiers.shift) {
                return None;
            }
        }
        return Some(event);
    }

    let keycode = match KeyCode::from_raw(event.code) {
        Some(k) => k,
        None => return Some(event),
    };

    // Clone state for async execution
    let state = scroll_state.clone();
    let shift = event.modifiers.shift;
    let control = event.modifiers.control;
    let option = event.modifiers.option;
    let command = event.modifiers.command;

    // Process the key
    let mut scroll_state_guard = state.lock().unwrap();
    let result = scroll_state_guard.process_key(
        keycode,
        shift,
        control,
        option,
        command,
        scroll_step,
    );
    drop(scroll_state_guard);

    match result {
        ScrollResult::Handled => None,
        ScrollResult::PassThrough => Some(event),
    }
}

/// Check if a key is a potential scroll mode key
/// Used to determine if we should suppress key up events
fn is_scroll_key(keycode: KeyCode, shift: bool) -> bool {
    matches!(
        (keycode, shift),
        (KeyCode::H, false)
            | (KeyCode::J, false)
            | (KeyCode::K, false)
            | (KeyCode::L, false)
            | (KeyCode::G, _)
            | (KeyCode::D, false)
            | (KeyCode::U, false)
            | (KeyCode::Slash, false)
            | (KeyCode::H, true)
            | (KeyCode::L, true)
            | (KeyCode::R, _)
    )
}
