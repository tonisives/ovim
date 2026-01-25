//! Keyboard event handler for vim mode processing

mod click_mode;
pub mod double_tap;
mod shortcuts;

use std::sync::{Arc, Mutex};

use crate::click_mode::SharedClickModeManager;
use crate::commands::RecordedKey;
use crate::config::click_mode::DoubleTapModifier;
use crate::config::Settings;
use crate::keyboard::{KeyCode, KeyEvent};
use crate::nvim_edit::EditSessionManager;
use crate::vim::VimState;

use click_mode::handle_click_mode_key;
use double_tap::{DoubleTapKey, DoubleTapManager};
use shortcuts::{check_click_mode_shortcut, check_nvim_edit_shortcut, check_vim_key, process_vim_input};

/// Callback type for when a double-tap triggers a mode activation
pub type DoubleTapCallback = Box<dyn Fn(DoubleTapKey) + Send + 'static>;

/// Create the keyboard callback that processes key events
pub fn create_keyboard_callback(
    vim_state: Arc<Mutex<VimState>>,
    settings: Arc<Mutex<Settings>>,
    record_key_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<RecordedKey>>>>,
    edit_session_manager: Arc<EditSessionManager>,
    click_mode_manager: SharedClickModeManager,
    double_tap_manager: Arc<Mutex<DoubleTapManager>>,
    double_tap_callback: DoubleTapCallback,
) -> impl Fn(KeyEvent) -> Option<KeyEvent> + Send + 'static {
    move |event| {
        // Check for Escape key double-tap (for non-modifier double-tap shortcuts)
        if let Some(keycode) = event.keycode() {
            if keycode == KeyCode::Escape {
                let mut dt_manager = double_tap_manager.lock().unwrap();
                if let Some(double_tap_key) = dt_manager.process_key_event(DoubleTapKey::Escape, event.is_key_down) {
                    // Check if Escape double-tap is configured for either mode
                    let settings_guard = settings.lock().unwrap();
                    let click_uses_escape = settings_guard.click_mode.double_tap_modifier == DoubleTapModifier::Escape;
                    let nvim_uses_escape = settings_guard.nvim_edit.double_tap_modifier == DoubleTapModifier::Escape;
                    drop(settings_guard);

                    if click_uses_escape || nvim_uses_escape {
                        double_tap_callback(double_tap_key);
                        return None; // Suppress the escape key
                    }
                }
            }
        }
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
                Arc::clone(&settings),
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
