//! List mode keyboard handler
//!
//! Handles keyboard events for list mode (hjkl arrow key navigation).

use crate::keyboard::keycode::KeyCode;
use crate::keyboard::KeyEvent;
use crate::list_mode::{ListResult, SharedListModeState};

/// Handle a key event in list mode
///
/// Returns `None` to suppress the key, `Some(event)` to pass it through.
pub fn handle_list_mode_key(
    event: KeyEvent,
    list_state: &SharedListModeState,
) -> Option<KeyEvent> {
    // Only process key down events
    if !event.is_key_down {
        // Suppress key up for keys we handled on key down
        if let Some(keycode) = KeyCode::from_raw(event.code) {
            if is_list_key(keycode, event.modifiers.shift) {
                return None;
            }
        }
        return Some(event);
    }

    let keycode = match KeyCode::from_raw(event.code) {
        Some(k) => k,
        None => return Some(event),
    };

    // Clone state for processing
    let state = list_state.clone();
    let shift = event.modifiers.shift;
    let control = event.modifiers.control;
    let option = event.modifiers.option;
    let command = event.modifiers.command;

    // Process the key
    let mut list_state_guard = state.lock().unwrap();
    let result = list_state_guard.process_key(
        keycode,
        shift,
        control,
        option,
        command,
    );
    drop(list_state_guard);

    match result {
        ListResult::Handled => None,
        ListResult::PassThrough => Some(event),
    }
}

/// Check if a key is a potential list mode key
/// Used to determine if we should suppress key up events
fn is_list_key(keycode: KeyCode, shift: bool) -> bool {
    matches!(
        (keycode, shift),
        (KeyCode::H, _)        // h (left) and H (back)
            | (KeyCode::J, _)  // j and J (shift for selection)
            | (KeyCode::K, _)  // k and K (shift for selection)
            | (KeyCode::L, _)  // l (right) and L (forward)
            | (KeyCode::G, _)  // g and G
            | (KeyCode::O, false)  // o for open
            | (KeyCode::Slash, false)  // / for search
    )
}
