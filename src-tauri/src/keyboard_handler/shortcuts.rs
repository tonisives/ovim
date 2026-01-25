//! Shortcut key checking and handling

use std::sync::{Arc, Mutex};
use std::thread;

use tauri::Emitter;

use crate::click_mode::native_hints::{self, HintStyle};
use crate::click_mode::SharedClickModeManager;
use crate::config::Settings;
use crate::get_app_handle;
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

/// Check if the frontmost app is in the ignored apps list
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

/// Check if event modifiers match the configured modifiers
fn modifiers_match(event: &KeyEvent, mods: &crate::config::VimKeyModifiers) -> bool {
    event.modifiers.shift == mods.shift
        && event.modifiers.control == mods.control
        && event.modifiers.option == mods.option
        && event.modifiers.command == mods.command
}

/// Check if this is the configured nvim edit shortcut and handle it
pub fn check_nvim_edit_shortcut(
    event: &KeyEvent,
    settings: &Settings,
    edit_session_manager: Arc<EditSessionManager>,
    shared_settings: Arc<Mutex<Settings>>,
) -> Option<Option<KeyEvent>> {
    let nvim_settings = &settings.nvim_edit;

    if !nvim_settings.enabled {
        return None;
    }

    let nvim_key = KeyCode::from_name(&nvim_settings.shortcut_key)?;
    if event.keycode() != Some(nvim_key) {
        return None;
    }

    if !modifiers_match(event, &nvim_settings.shortcut_modifiers) {
        return None;
    }

    let nvim_settings_clone = nvim_settings.clone();
    thread::spawn(move || {
        if let Err(e) = nvim_edit::trigger_nvim_edit(edit_session_manager, nvim_settings_clone, Some(shared_settings)) {
            log::error!("Failed to trigger nvim edit: {}", e);
        }
    });

    Some(None) // Consume the event
}

/// Check if this is the configured click mode shortcut and handle it
pub fn check_click_mode_shortcut(
    event: &KeyEvent,
    settings: &Settings,
    click_mode_manager: SharedClickModeManager,
) -> Option<Option<KeyEvent>> {
    let click_settings = &settings.click_mode;

    if !click_settings.enabled {
        return None;
    }

    let click_key = KeyCode::from_name(&click_settings.shortcut_key)?;
    if event.keycode() != Some(click_key) {
        return None;
    }

    if !modifiers_match(event, &click_settings.shortcut_modifiers) {
        return None;
    }

    // Set click mode to activating state IMMEDIATELY
    {
        let mut mgr = click_mode_manager.lock().unwrap();
        mgr.set_activating();
    }

    // Activate click mode on a separate thread
    let manager = Arc::clone(&click_mode_manager);
    thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut mgr = manager.lock().unwrap();
            match mgr.activate() {
                Ok(elements) => {
                    log::info!("Click mode activated with {} elements", elements.len());
                    let style = HintStyle::default();
                    native_hints::show_hints(&elements, &style);
                    if let Some(app) = get_app_handle() {
                        let _ = app.emit("click-mode-activated", ());
                    }
                }
                Err(e) => {
                    log::error!("Failed to activate click mode: {}", e);
                    mgr.deactivate();
                }
            }
        }));

        if let Err(e) = result {
            log::error!("Panic in click mode activation: {:?}", e);
            if let Ok(mut mgr) = manager.lock() {
                mgr.deactivate();
            }
        }
    });

    Some(None) // Consume the event
}

/// Check if this is the configured vim key and handle it
pub fn check_vim_key(
    event: &KeyEvent,
    settings: &Settings,
    vim_state: Arc<Mutex<VimState>>,
) -> Option<Option<KeyEvent>> {
    if !settings.enabled {
        return None;
    }

    let vim_key = KeyCode::from_name(&settings.vim_key)?;
    if event.keycode() != Some(vim_key) {
        return None;
    }

    if !modifiers_match(event, &settings.vim_key_modifiers) {
        return None;
    }

    let ignored_apps = settings.ignored_apps.clone();
    let current_mode = vim_state.lock().unwrap().mode();

    if current_mode == VimMode::Insert && is_frontmost_app_ignored(&ignored_apps) {
        log::debug!("Vim key: ignored app, passing through");
        return Some(Some(event.clone()));
    }

    let result = {
        let mut state = vim_state.lock().unwrap();
        state.handle_vim_key()
    };

    match result {
        ProcessResult::ModeChanged(_mode, action) => {
            log::debug!("Vim key: ModeChanged");
            if let Some(action) = action {
                execute_action_async(action);
            }
            Some(None)
        }
        _ => Some(None),
    }
}

/// Process vim input for non-shortcut keys
pub fn process_vim_input(
    event: KeyEvent,
    settings: &Arc<Mutex<Settings>>,
    vim_state: &Arc<Mutex<VimState>>,
) -> Option<KeyEvent> {
    // Check if vim mode is disabled
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
            log::debug!("SuppressWithAction: keycode={}, action={:?}", event.code, action);
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
