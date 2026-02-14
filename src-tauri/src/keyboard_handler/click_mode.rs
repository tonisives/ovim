//! Click mode keyboard input handling

use std::thread;

use tauri::Emitter;

use crate::click_mode::native_hints;
use crate::click_mode::{self, ClickAction, HintInputResult, SharedClickModeManager};
use crate::get_app_handle;
use crate::keyboard::{KeyCode, KeyEvent};

/// Handle keyboard input when click mode is active
pub fn handle_click_mode_key(event: KeyEvent, manager: SharedClickModeManager) -> Option<KeyEvent> {
    // Only handle key down events
    if !event.is_key_down {
        return None; // Suppress key up events in click mode
    }

    let keycode = event.keycode()?;

    // Handle special keys
    if let Some(result) = handle_special_keys(keycode, &manager) {
        return result;
    }

    // Handle action switching keys (r/c/d/n without modifiers)
    if is_no_modifiers(&event) {
        if let Some(c) = keycode.to_char() {
            if let Some(result) = handle_action_switch(c, &manager) {
                return result;
            }
        }
    }

    // Handle alphanumeric hint input
    if let Some(c) = keycode.to_char() {
        if c.is_alphanumeric() {
            return handle_hint_input(c, manager);
        }
    }

    // Suppress all other keys while in click mode
    None
}

/// Check if no modifiers are pressed
fn is_no_modifiers(event: &KeyEvent) -> bool {
    !event.modifiers.shift
        && !event.modifiers.control
        && !event.modifiers.option
        && !event.modifiers.command
}

/// Handle special keys (Escape, Delete, Return)
fn handle_special_keys(
    keycode: KeyCode,
    manager: &SharedClickModeManager,
) -> Option<Option<KeyEvent>> {
    match keycode {
        KeyCode::Escape => {
            deactivate_click_mode(manager);
            Some(None)
        }
        KeyCode::Delete => {
            handle_backspace(manager);
            Some(None)
        }
        KeyCode::Return => {
            // TODO: Implement selection confirmation
            Some(None)
        }
        _ => None,
    }
}

/// Deactivate click mode and hide hints
fn deactivate_click_mode(manager: &SharedClickModeManager) {
    click_mode::deactivate_and_notify(manager);
    log::info!("Click mode cancelled via Escape");
}

/// Handle backspace to clear last input
fn handle_backspace(manager: &SharedClickModeManager) {
    let mut mgr = manager.lock().unwrap();
    mgr.clear_last_input();
    log::debug!("Click mode: cleared last input");

    let all_elements = mgr.get_all_elements();
    let current_input = mgr.get_current_input();
    native_hints::filter_hints(&current_input, &all_elements);

    let filtered = mgr.get_filtered_elements();
    if let Some(app) = get_app_handle() {
        let _ = app.emit("click-mode-filtered", &filtered);
    }
}

/// Handle action switching keys (r/c/d/n)
fn handle_action_switch(c: char, manager: &SharedClickModeManager) -> Option<Option<KeyEvent>> {
    let new_action = match c.to_ascii_lowercase() {
        'r' => Some(ClickAction::RightClick),
        'c' => Some(ClickAction::CmdClick),
        'd' => Some(ClickAction::DoubleClick),
        'n' => Some(ClickAction::Click),
        _ => None,
    };

    if let Some(action) = new_action {
        let mut mgr = manager.lock().unwrap();
        mgr.set_click_action(action);
        log::info!("Click mode: switched to {:?} action", action);
        if let Some(app) = get_app_handle() {
            let _ = app.emit("click-action-changed", action);
        }
        return Some(None);
    }

    None
}

/// Handle alphanumeric hint input
fn handle_hint_input(c: char, manager: SharedClickModeManager) -> Option<KeyEvent> {
    let mut mgr = manager.lock().unwrap();
    let click_action = mgr.get_click_action();

    match mgr.handle_hint_input(c) {
        HintInputResult::Match(element) => {
            handle_hint_match(element, click_action, &mut mgr, manager.clone())
        }
        HintInputResult::Partial => {
            handle_partial_match(&mgr);
            None
        }
        HintInputResult::WrongSecondKey => {
            handle_wrong_key();
            None
        }
        HintInputResult::NoMatch => {
            handle_no_match(&mut mgr);
            None
        }
    }
}

/// Handle exact hint match - perform click
fn handle_hint_match(
    element: crate::click_mode::ClickableElement,
    click_action: ClickAction,
    mgr: &mut std::sync::MutexGuard<crate::click_mode::ClickModeManager>,
    _manager: SharedClickModeManager,
) -> Option<KeyEvent> {
    let action_name = click_action.display_name();
    log::info!(
        "Click mode: {} on element '{}' ({})",
        action_name,
        element.hint,
        element.title
    );

    let element_id = element.id;
    let position = mgr.get_element_position(element_id);

    // Deactivate click mode state, hide hints, and notify frontend
    click_mode::deactivate_with_guard(mgr);

    // Perform click on a separate thread with delay
    if let Some((x, y)) = position {
        thread::spawn(move || {
            thread::sleep(std::time::Duration::from_millis(50));
            let result = perform_click(x, y, click_action);
            if let Err(e) = result {
                log::error!("Failed to {} element: {}", action_name, e);
            }
        });
    } else {
        log::error!("Could not get position for element {}", element_id);
    }

    None
}

/// Perform click based on action type
fn perform_click(x: f64, y: f64, action: ClickAction) -> Result<(), String> {
    use crate::click_mode::accessibility;

    match action {
        ClickAction::Click => accessibility::perform_click_at_position(x, y),
        ClickAction::RightClick => accessibility::perform_right_click_at_position(x, y),
        ClickAction::CmdClick => accessibility::perform_cmd_click_at_position(x, y),
        ClickAction::DoubleClick => accessibility::perform_double_click_at_position(x, y),
    }
}

/// Handle partial hint match
fn handle_partial_match(mgr: &std::sync::MutexGuard<crate::click_mode::ClickModeManager>) {
    log::debug!("Click mode: partial match, waiting for more input");
    let all_elements = mgr.get_all_elements();
    let current_input = mgr.get_current_input();
    native_hints::filter_hints_with_input(&current_input, &all_elements);

    let filtered = mgr.get_filtered_elements();
    if let Some(app) = get_app_handle() {
        let _ = app.emit("click-mode-filtered", (&filtered, &current_input));
    }
}

/// Handle wrong second key
fn handle_wrong_key() {
    log::debug!("Click mode: wrong second key, allowing retry");
    if let Some(app) = get_app_handle() {
        let _ = app.emit("click-mode-wrong-key", ());
    }
    native_hints::shake_hints();
}

/// Handle no match
fn handle_no_match(mgr: &mut std::sync::MutexGuard<crate::click_mode::ClickModeManager>) {
    log::debug!("Click mode: no match, deactivating");
    click_mode::deactivate_with_guard(mgr);
}
