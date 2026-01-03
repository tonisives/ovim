//! Keyboard event handler for vim mode processing

mod click_mode;
mod shortcuts;

use std::sync::{Arc, Mutex};

use crate::click_mode::SharedClickModeManager;
use crate::commands::RecordedKey;
use crate::config::Settings;
use crate::keyboard::KeyEvent;
use crate::nvim_edit::EditSessionManager;
use crate::vim::VimState;

use click_mode::handle_click_mode_key;
use shortcuts::{check_click_mode_shortcut, check_nvim_edit_shortcut, check_vim_key, process_vim_input};

/// Create the keyboard callback that processes key events
pub fn create_keyboard_callback(
    vim_state: Arc<Mutex<VimState>>,
    settings: Arc<Mutex<Settings>>,
    record_key_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<RecordedKey>>>>,
    edit_session_manager: Arc<EditSessionManager>,
    click_mode_manager: SharedClickModeManager,
) -> impl Fn(KeyEvent) -> Option<KeyEvent> + Send + 'static {
    move |event| {
        // Check if click mode is active - if so, route keys there first
        {
            let click_manager = click_mode_manager.lock().unwrap();
            if click_manager.is_active() {
                drop(click_manager);
                return handle_click_mode_key(event, Arc::clone(&click_mode_manager));
            }
        }

        // Check if we're recording a key (only on key down)
        if event.is_key_down {
            if let Some(recorded) = try_record_key(&event, &record_key_tx) {
                let mut record_tx = record_key_tx.lock().unwrap();
                if let Some(tx) = record_tx.take() {
                    let _ = tx.send(recorded);
                    return None;
                }
            }
        }

        // Check shortcuts on key down
        if event.is_key_down {
            let settings_guard = settings.lock().unwrap();

            // Check nvim edit shortcut
            if let Some(result) = check_nvim_edit_shortcut(
                &event,
                &settings_guard,
                Arc::clone(&edit_session_manager),
            ) {
                return result;
            }

            // Check click mode shortcut
            if let Some(result) = check_click_mode_shortcut(
                &event,
                &settings_guard,
                Arc::clone(&click_mode_manager),
            ) {
                return result;
            }

            // Check vim key
            if let Some(result) = check_vim_key(&event, &settings_guard, Arc::clone(&vim_state)) {
                return result;
            }
        }

        // Process normal vim input
        process_vim_input(event, &settings, &vim_state)
    }
}

/// Try to record a key if recording is active
fn try_record_key(
    event: &KeyEvent,
    record_key_tx: &Arc<Mutex<Option<tokio::sync::oneshot::Sender<RecordedKey>>>>,
) -> Option<RecordedKey> {
    use crate::commands::RecordedModifiers;

    let record_tx = record_key_tx.lock().unwrap();
    if record_tx.is_some() {
        if let Some(keycode) = event.keycode() {
            return Some(RecordedKey {
                name: keycode.to_name().to_string(),
                display_name: keycode.to_display_name().to_string(),
                modifiers: RecordedModifiers {
                    shift: event.modifiers.shift,
                    control: event.modifiers.control,
                    option: event.modifiers.option,
                    command: event.modifiers.command,
                },
            });
        }
    }
    None
}
