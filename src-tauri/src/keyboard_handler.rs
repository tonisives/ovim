//! Keyboard event handler for vim mode processing

use std::sync::{Arc, Mutex};
use std::thread;

use tauri::{Emitter, Manager};

use crate::click_mode::SharedClickModeManager;
use crate::commands::{RecordedKey, RecordedModifiers};
use crate::config::Settings;
use crate::keyboard::{KeyCode, KeyEvent};
use crate::nvim_edit::{self, EditSessionManager};
use crate::vim::{ProcessResult, VimAction, VimMode, VimState};
use crate::window::{position_click_overlay_fullscreen, setup_click_overlay_window};
use crate::get_app_handle;

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

        // Check if this is the configured click mode shortcut
        if event.is_key_down {
            let settings_guard = settings.lock().unwrap();
            let click_settings = &settings_guard.click_mode;

            if click_settings.enabled {
                let click_key = KeyCode::from_name(&click_settings.shortcut_key);
                let mods = &click_settings.shortcut_modifiers;

                let modifiers_match = event.modifiers.shift == mods.shift
                    && event.modifiers.control == mods.control
                    && event.modifiers.option == mods.option
                    && event.modifiers.command == mods.command;

                if let Some(configured_key) = click_key {
                    if event.keycode() == Some(configured_key) && modifiers_match {
                        drop(settings_guard);

                        // Activate click mode
                        let manager = Arc::clone(&click_mode_manager);
                        thread::spawn(move || {
                            let mut mgr = manager.lock().unwrap();
                            match mgr.activate() {
                                Ok(elements) => {
                                    log::info!(
                                        "Click mode activated with {} elements",
                                        elements.len()
                                    );
                                    // Show and position the overlay window, then send elements
                                    if let Some(app) = get_app_handle() {
                                        if let Some(overlay) = app.get_webview_window("click-overlay") {
                                            // Position overlay to cover all screens
                                            if let Err(e) = position_click_overlay_fullscreen(&overlay) {
                                                log::warn!("Failed to position overlay: {}", e);
                                            }
                                            // Ensure window is set up correctly
                                            if let Err(e) = setup_click_overlay_window(&overlay) {
                                                log::warn!("Failed to setup overlay: {}", e);
                                            }
                                            // Show the overlay
                                            if let Err(e) = overlay.show() {
                                                log::error!("Failed to show click overlay: {}", e);
                                            }
                                            if let Err(e) = overlay.set_focus() {
                                                log::warn!("Failed to focus click overlay: {}", e);
                                            }
                                        }
                                        // Send elements to frontend
                                        if let Err(e) = app.emit("click-mode-activated", &elements) {
                                            log::error!("Failed to emit click-mode-activated: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!("Failed to activate click mode: {}", e);
                                    mgr.deactivate();
                                }
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

/// Hide the click overlay window
fn hide_click_overlay() {
    if let Some(app) = get_app_handle() {
        if let Some(overlay) = app.get_webview_window("click-overlay") {
            if let Err(e) = overlay.hide() {
                log::error!("Failed to hide click overlay: {}", e);
            }
        }
        let _ = app.emit("click-mode-deactivated", ());
    }
}

/// Handle keyboard input when click mode is active
fn handle_click_mode_key(event: KeyEvent, manager: SharedClickModeManager) -> Option<KeyEvent> {
    // Only handle key down events
    if !event.is_key_down {
        return None; // Suppress key up events in click mode
    }

    let keycode = match event.keycode() {
        Some(kc) => kc,
        None => return None,
    };

    // Handle Escape to cancel click mode
    if keycode == KeyCode::Escape {
        let mut mgr = manager.lock().unwrap();
        mgr.deactivate();
        log::info!("Click mode cancelled via Escape");
        hide_click_overlay();
        return None;
    }

    // Handle Delete (Backspace on macOS) to clear last input
    if keycode == KeyCode::Delete {
        let mut mgr = manager.lock().unwrap();
        mgr.clear_last_input();
        log::debug!("Click mode: cleared last input");
        // Send updated filtered elements to frontend
        let filtered = mgr.get_filtered_elements();
        if let Some(app) = get_app_handle() {
            let _ = app.emit("click-mode-filtered", &filtered);
        }
        return None;
    }

    // Handle Enter to confirm selection (in search mode)
    if keycode == KeyCode::Return {
        // TODO: Implement selection confirmation
        return None;
    }

    // Handle letter/number keys for hint input
    if let Some(c) = keycode.to_char() {
        if c.is_alphanumeric() {
            let mut mgr = manager.lock().unwrap();
            let shift_held = event.modifiers.shift;

            match mgr.handle_hint_input(c) {
                Ok(Some(element)) => {
                    // Exact match found - perform click (or right-click if Shift held)
                    let click_type = if shift_held { "right-click" } else { "click" };
                    log::info!(
                        "Click mode: {} on element '{}' ({})",
                        click_type,
                        element.hint,
                        element.title
                    );
                    let element_id = element.id;

                    // Perform click or right-click based on Shift modifier
                    let result = if shift_held {
                        mgr.right_click_element(element_id)
                    } else {
                        mgr.click_element(element_id)
                    };

                    if let Err(e) = result {
                        log::error!("Failed to {} element: {}", click_type, e);
                    }

                    // Deactivate click mode after successful click
                    mgr.deactivate();
                    hide_click_overlay();
                }
                Ok(None) => {
                    // Partial match - continue waiting for more input
                    log::debug!("Click mode: partial match, waiting for more input");
                    // Send updated filtered elements to frontend
                    let filtered = mgr.get_filtered_elements();
                    if let Some(app) = get_app_handle() {
                        let _ = app.emit("click-mode-filtered", &filtered);
                    }
                }
                Err(e) => {
                    // No match - could beep or flash
                    log::debug!("Click mode: no match - {}", e);
                }
            }

            return None;
        }
    }

    // Suppress all other keys while in click mode
    None
}
