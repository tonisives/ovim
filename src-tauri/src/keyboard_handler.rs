//! Keyboard event handler for vim mode processing

use std::sync::{Arc, Mutex};
use std::thread;

use crate::commands::{RecordedKey, RecordedModifiers};
use crate::config::Settings;
use crate::keyboard::{KeyCode, KeyEvent};
use crate::nvim_edit::{self, EditSessionManager};
use crate::vim::{ProcessResult, VimAction, VimMode, VimState};

#[cfg(target_os = "macos")]
use objc::{class, msg_send, sel, sel_impl};

/// Execute a VimAction on a separate thread with a small delay
fn execute_action_async(action: VimAction) {
    thread::spawn(move || {
        thread::sleep(std::time::Duration::from_micros(500));
        if let Err(e) = action.execute() {
            log::error!("Failed to execute vim action: {}", e);
        }
    });
}

/// Get the bundle identifier of the frontmost (currently focused) application
#[cfg(target_os = "macos")]
fn get_frontmost_app_bundle_id() -> Option<String> {
    unsafe {
        let workspace: *mut objc::runtime::Object =
            msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace.is_null() {
            return None;
        }
        let app: *mut objc::runtime::Object = msg_send![workspace, frontmostApplication];
        if app.is_null() {
            return None;
        }
        let bundle_id: *mut objc::runtime::Object = msg_send![app, bundleIdentifier];
        if bundle_id.is_null() {
            return None;
        }
        let utf8: *const std::os::raw::c_char = msg_send![bundle_id, UTF8String];
        if utf8.is_null() {
            return None;
        }
        Some(
            std::ffi::CStr::from_ptr(utf8)
                .to_string_lossy()
                .into_owned(),
        )
    }
}

/// Check if the frontmost app is in the ignored apps list.
fn is_frontmost_app_ignored(ignored_apps: &[String]) -> bool {
    if ignored_apps.is_empty() {
        return false;
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(bundle_id) = get_frontmost_app_bundle_id() {
            return ignored_apps.iter().any(|id| id == &bundle_id);
        }
    }
    false
}

/// Create the keyboard callback that processes key events
pub fn create_keyboard_callback(
    vim_state: Arc<Mutex<VimState>>,
    settings: Arc<Mutex<Settings>>,
    record_key_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<RecordedKey>>>>,
    edit_session_manager: Arc<EditSessionManager>,
) -> impl Fn(KeyEvent) -> Option<KeyEvent> + Send + 'static {
    move |event| {
        // Check if we're recording a key (only on key down)
        if event.is_key_down {
            let mut record_tx = record_key_tx.lock().unwrap();
            if let Some(tx) = record_tx.take() {
                if let Some(keycode) = event.keycode() {
                    let recorded = RecordedKey {
                        name: keycode.to_name().to_string(),
                        display_name: keycode.to_display_name().to_string(),
                        modifiers: RecordedModifiers {
                            shift: event.modifiers.shift,
                            control: event.modifiers.control,
                            option: event.modifiers.option,
                            command: event.modifiers.command,
                        },
                    };
                    let _ = tx.send(recorded);
                    return None;
                }
            }
        }

        // Check if this is the configured nvim edit shortcut
        if event.is_key_down {
            let settings_guard = settings.lock().unwrap();
            let nvim_settings = &settings_guard.nvim_edit;

            if nvim_settings.enabled {
                let nvim_key = KeyCode::from_name(&nvim_settings.shortcut_key);
                let mods = &nvim_settings.shortcut_modifiers;

                let modifiers_match = event.modifiers.shift == mods.shift
                    && event.modifiers.control == mods.control
                    && event.modifiers.option == mods.option
                    && event.modifiers.command == mods.command;

                if let Some(configured_key) = nvim_key {
                    if event.keycode() == Some(configured_key) && modifiers_match {
                        let nvim_settings_clone = nvim_settings.clone();
                        drop(settings_guard);

                        let manager = Arc::clone(&edit_session_manager);
                        thread::spawn(move || {
                            if let Err(e) =
                                nvim_edit::trigger_nvim_edit(manager, nvim_settings_clone)
                            {
                                log::error!("Failed to trigger nvim edit: {}", e);
                            }
                        });

                        return None;
                    }
                }
            }
        }

        // Check if this is the configured vim key with matching modifiers
        if event.is_key_down {
            let settings_guard = settings.lock().unwrap();

            if !settings_guard.enabled {
                return Some(event);
            }

            let vim_key = KeyCode::from_name(&settings_guard.vim_key);
            let mods = &settings_guard.vim_key_modifiers;

            let modifiers_match = event.modifiers.shift == mods.shift
                && event.modifiers.control == mods.control
                && event.modifiers.option == mods.option
                && event.modifiers.command == mods.command;

            if let Some(configured_key) = vim_key {
                if event.keycode() == Some(configured_key) && modifiers_match {
                    let ignored_apps = settings_guard.ignored_apps.clone();
                    drop(settings_guard);

                    let current_mode = vim_state.lock().unwrap().mode();
                    if current_mode == VimMode::Insert {
                        if is_frontmost_app_ignored(&ignored_apps) {
                            log::debug!("Vim key: ignored app, passing through");
                            return Some(event);
                        }
                    }

                    let result = {
                        let mut state = vim_state.lock().unwrap();
                        state.handle_vim_key()
                    };

                    return match result {
                        ProcessResult::ModeChanged(_mode, action) => {
                            log::debug!("Vim key: ModeChanged");
                            if let Some(action) = action {
                                execute_action_async(action);
                            }
                            None
                        }
                        _ => None,
                    };
                }
            }
        }

        // Check if vim mode is disabled for non-key-down events
        {
            let settings_guard = settings.lock().unwrap();
            if !settings_guard.enabled {
                return Some(event);
            }
        }

        let result = {
            let mut state = vim_state.lock().unwrap();
            state.process_key(event)
        };

        match result {
            ProcessResult::Suppress => {
                log::debug!("Suppress: keycode={}", event.code);
                None
            }
            ProcessResult::SuppressWithAction(ref action) => {
                log::debug!(
                    "SuppressWithAction: keycode={}, action={:?}",
                    event.code,
                    action
                );
                execute_action_async(action.clone());
                None
            }
            ProcessResult::PassThrough => {
                log::debug!("PassThrough: keycode={}", event.code);
                Some(event)
            }
            ProcessResult::ModeChanged(_mode, action) => {
                log::debug!("ModeChanged: keycode={}", event.code);
                if let Some(action) = action {
                    execute_action_async(action);
                }
                None
            }
        }
    }
}
