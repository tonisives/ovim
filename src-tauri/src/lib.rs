// Allow unexpected_cfgs from the objc crate's macros which use cfg(feature = "cargo-clippy")
#![allow(unexpected_cfgs)]

mod config;
pub mod ipc;
mod keyboard;
mod nvim_edit;
mod vim;
mod widgets;
mod window;

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{
    menu::{Menu, MenuItem},
    AppHandle, Emitter, Manager, State,
};

#[cfg(target_os = "macos")]
use objc::{class, msg_send, sel, sel_impl};

use config::Settings;
use ipc::{IpcCommand, IpcResponse};
use keyboard::{check_accessibility_permission, request_accessibility_permission, KeyboardCapture, KeyCode, KeyEvent};
use nvim_edit::EditSessionManager;
use vim::{ProcessResult, VimAction, VimMode, VimState};
use window::setup_indicator_window;

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::OnceLock;

static LOG_FILE: OnceLock<Mutex<std::fs::File>> = OnceLock::new();

fn init_file_logger() {
    // Create/truncate the log file
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open("/tmp/ovim-rust.log")
        .expect("Failed to create log file");

    LOG_FILE.set(Mutex::new(file)).ok();

    // Set up env_logger to also write to our file
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            let timestamp = chrono::Local::now().format("%H:%M:%S%.3f");
            let line = format!("[{}] {} - {}\n", timestamp, record.level(), record.args());

            // Write to file
            if let Some(file_mutex) = LOG_FILE.get() {
                if let Ok(mut file) = file_mutex.lock() {
                    let _ = file.write_all(line.as_bytes());
                    let _ = file.flush();
                }
            }

            // Also write to stderr
            write!(buf, "{}", line)
        })
        .init();
}

/// Execute a VimAction on a separate thread with a small delay
fn execute_action_async(action: VimAction) {
    thread::spawn(move || {
        thread::sleep(Duration::from_micros(500));
        if let Err(e) = action.execute() {
            log::error!("Failed to execute vim action: {}", e);
        }
    });
}

/// Recorded key info returned to frontend
#[derive(Debug, Clone, serde::Serialize)]
pub struct RecordedKey {
    pub name: String,
    pub display_name: String,
    pub modifiers: RecordedModifiers,
}

/// Modifier state for recorded key
#[derive(Debug, Clone, serde::Serialize)]
pub struct RecordedModifiers {
    pub shift: bool,
    pub control: bool,
    pub option: bool,
    pub command: bool,
}

/// Application state shared across commands
pub struct AppState {
    settings: Arc<Mutex<Settings>>,
    vim_state: Arc<Mutex<VimState>>,
    keyboard_capture: KeyboardCapture,
    /// One-shot channel to receive recorded key
    record_key_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<RecordedKey>>>>,
    /// Edit session manager for "Edit with Neovim" feature
    #[allow(dead_code)]
    edit_session_manager: Arc<EditSessionManager>,
}

// Tauri commands

#[tauri::command]
fn check_permission() -> bool {
    check_accessibility_permission()
}

#[tauri::command]
fn request_permission() -> bool {
    request_accessibility_permission()
}

#[tauri::command]
fn get_vim_mode(state: State<AppState>) -> String {
    let vim_state = state.vim_state.lock().unwrap();
    vim_state.mode().as_str().to_string()
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    let settings = state.settings.lock().unwrap();
    settings.clone()
}

#[tauri::command]
fn set_settings(app: AppHandle, state: State<AppState>, new_settings: Settings) -> Result<(), String> {
    let mut settings = state.settings.lock().unwrap();
    *settings = new_settings.clone();
    settings.save()?;

    let _ = app.emit("settings-changed", new_settings);
    Ok(())
}

#[tauri::command]
fn open_settings_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn pick_app() -> Result<Option<String>, String> {
    use std::process::Command;

    // Use osascript to open file dialog and get bundle ID
    let script = r#"
        set appPath to choose file of type {"app"} with prompt "Select an application" default location "/Applications"
        set appPath to POSIX path of appPath
        return appPath
    "#;

    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| format!("Failed to run osascript: {}", e))?;

    if !output.status.success() {
        // User cancelled
        return Ok(None);
    }

    let app_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if app_path.is_empty() {
        return Ok(None);
    }

    // Get bundle ID using mdls
    let bundle_output = Command::new("mdls")
        .args(["-name", "kMDItemCFBundleIdentifier", "-raw", &app_path])
        .output()
        .map_err(|e| format!("Failed to get bundle ID: {}", e))?;

    let bundle_id = String::from_utf8_lossy(&bundle_output.stdout).trim().to_string();
    if bundle_id.is_empty() || bundle_id == "(null)" {
        return Err("Could not determine bundle identifier".to_string());
    }

    Ok(Some(bundle_id))
}

#[tauri::command]
fn start_capture(state: State<AppState>) -> Result<(), String> {
    state.keyboard_capture.start()
}

#[tauri::command]
fn stop_capture(state: State<AppState>) {
    state.keyboard_capture.stop()
}

#[tauri::command]
fn is_capture_running(state: State<AppState>) -> bool {
    state.keyboard_capture.is_running()
}

#[tauri::command]
fn get_selection_info() -> widgets::selection::SelectionInfo {
    widgets::selection::get_selection_info()
}

#[tauri::command]
fn get_battery_info() -> Option<widgets::battery::BatteryInfo> {
    widgets::battery::get_battery_info()
}

#[tauri::command]
fn get_caps_lock_state() -> bool {
    widgets::capslock::is_caps_lock_on()
}

#[tauri::command]
fn get_pending_keys(state: State<AppState>) -> String {
    let vim_state = state.vim_state.lock().unwrap();
    vim_state.get_pending_keys()
}

#[tauri::command]
fn get_key_display_name(key_name: String) -> Option<String> {
    KeyCode::from_name(&key_name).map(|k| k.to_display_name().to_string())
}

#[tauri::command]
async fn record_key(state: State<'_, AppState>) -> Result<RecordedKey, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    {
        let mut record_tx = state.record_key_tx.lock().unwrap();
        *record_tx = Some(tx);
    }

    rx.await.map_err(|_| "Key recording cancelled".to_string())
}

#[tauri::command]
fn cancel_record_key(state: State<AppState>) {
    let mut record_tx = state.record_key_tx.lock().unwrap();
    *record_tx = None;
}

/// Log message from webview to /tmp/ovim-webview.log
#[tauri::command]
fn webview_log(level: String, message: String) {
    use std::fs::OpenOptions;
    use std::io::Write;

    let timestamp = chrono::Local::now().format("%H:%M:%S%.3f");
    let line = format!("[{}] {} - {}\n", timestamp, level.to_uppercase(), message);

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/ovim-webview.log")
    {
        let _ = file.write_all(line.as_bytes());
    }

    // Also log to rust logger
    match level.to_lowercase().as_str() {
        "error" => log::error!("[webview] {}", message),
        "warn" => log::warn!("[webview] {}", message),
        "debug" => log::debug!("[webview] {}", message),
        _ => log::info!("[webview] {}", message),
    }
}

// Helper functions

/// Get the bundle identifier of the frontmost (currently focused) application
#[cfg(target_os = "macos")]
fn get_frontmost_app_bundle_id() -> Option<String> {
    unsafe {
        let workspace: *mut objc::runtime::Object = msg_send![class!(NSWorkspace), sharedWorkspace];
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
        Some(std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned())
    }
}

/// Check if the frontmost app is in the ignored apps list.
/// Returns false early if the list is empty (no NSWorkspace call needed).
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

fn create_keyboard_callback(
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
                    // Suppress the key so it doesn't trigger vim mode or other actions
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

                        // Trigger nvim edit
                        let manager = Arc::clone(&edit_session_manager);
                        thread::spawn(move || {
                            if let Err(e) = nvim_edit::trigger_nvim_edit(manager, nvim_settings_clone) {
                                log::error!("Failed to trigger nvim edit: {}", e);
                            }
                        });

                        return None; // Suppress the key
                    }
                }
            }
        }

        // Check if this is the configured vim key with matching modifiers
        if event.is_key_down {
            let settings_guard = settings.lock().unwrap();
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

                    // Handle vim key toggle
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
}

fn handle_ipc_command(
    state: &mut VimState,
    app_handle: &AppHandle,
    cmd: IpcCommand,
) -> IpcResponse {
    match cmd {
        IpcCommand::GetMode => {
            IpcResponse::Mode(state.mode().as_str().to_string())
        }
        IpcCommand::Toggle => {
            let new_mode = state.toggle_mode();
            let _ = app_handle.emit("mode-change", new_mode.as_str());
            IpcResponse::Mode(new_mode.as_str().to_string())
        }
        IpcCommand::Insert => {
            state.set_mode_external(VimMode::Insert);
            let _ = app_handle.emit("mode-change", "insert");
            IpcResponse::Ok
        }
        IpcCommand::Normal => {
            state.set_mode_external(VimMode::Normal);
            let _ = app_handle.emit("mode-change", "normal");
            IpcResponse::Ok
        }
        IpcCommand::Visual => {
            state.set_mode_external(VimMode::Visual);
            let _ = app_handle.emit("mode-change", "visual");
            IpcResponse::Ok
        }
        IpcCommand::SetMode(mode_str) => {
            handle_set_mode(state, app_handle, &mode_str)
        }
    }
}

fn handle_set_mode(state: &mut VimState, app_handle: &AppHandle, mode_str: &str) -> IpcResponse {
    match mode_str.to_lowercase().as_str() {
        "insert" | "i" => {
            state.set_mode_external(VimMode::Insert);
            let _ = app_handle.emit("mode-change", "insert");
            IpcResponse::Ok
        }
        "normal" | "n" => {
            state.set_mode_external(VimMode::Normal);
            let _ = app_handle.emit("mode-change", "normal");
            IpcResponse::Ok
        }
        "visual" | "v" => {
            state.set_mode_external(VimMode::Visual);
            let _ = app_handle.emit("mode-change", "visual");
            IpcResponse::Ok
        }
        _ => IpcResponse::Error(format!("Unknown mode: {}", mode_str)),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize file-based logging to /tmp/ovim-rust.log
    init_file_logger();
    log::info!("ovim-rust started");

    let (vim_state, mode_rx) = VimState::new();
    let vim_state = Arc::new(Mutex::new(vim_state));

    let settings = Arc::new(Mutex::new(Settings::load()));
    let record_key_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<RecordedKey>>>> =
        Arc::new(Mutex::new(None));
    let edit_session_manager = Arc::new(EditSessionManager::new());

    let keyboard_capture = KeyboardCapture::new();
    keyboard_capture.set_callback(create_keyboard_callback(
        Arc::clone(&vim_state),
        Arc::clone(&settings),
        Arc::clone(&record_key_tx),
        Arc::clone(&edit_session_manager),
    ));

    let app_state = AppState {
        settings,
        vim_state: Arc::clone(&vim_state),
        keyboard_capture,
        record_key_tx,
        edit_session_manager,
    };

    let mode_rx = Arc::new(Mutex::new(mode_rx));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            check_permission,
            request_permission,
            get_vim_mode,
            get_settings,
            set_settings,
            start_capture,
            stop_capture,
            is_capture_running,
            open_settings_window,
            pick_app,
            get_selection_info,
            get_battery_info,
            get_caps_lock_state,
            get_pending_keys,
            get_key_display_name,
            record_key,
            cancel_record_key,
            webview_log,
        ])
        .setup(move |app| {
            // Hide dock icon - this is a menu bar app
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Set up tray menu on existing tray icon from tauri.conf.json
            let settings_item = MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&settings_item, &quit_item])?;

            if let Some(tray) = app.tray_by_id("main") {
                tray.set_menu(Some(menu))?;
                tray.on_menu_event(|app, event| match event.id.as_ref() {
                    "settings" => {
                        if let Some(window) = app.get_webview_window("settings") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                });
            }

            if let Some(indicator_window) = app.get_webview_window("indicator") {
                if let Err(e) = setup_indicator_window(&indicator_window) {
                    log::error!("Failed to setup indicator window: {}", e);
                }
            }

            // Prevent settings window from being destroyed on close - just hide it
            if let Some(settings_window) = app.get_webview_window("settings") {
                let window = settings_window.clone();
                settings_window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                });
            }

            let app_handle = app.handle().clone();
            let mut rx = mode_rx.lock().unwrap().resubscribe();

            tauri::async_runtime::spawn(async move {
                while let Ok(mode) = rx.recv().await {
                    log::info!("Mode changed to: {:?}", mode);
                    let _ = app_handle.emit("mode-change", mode.as_str());
                }
            });

            if check_accessibility_permission() {
                let state: State<AppState> = app.state();
                if let Err(e) = state.keyboard_capture.start() {
                    log::error!("Failed to start keyboard capture: {}", e);
                } else {
                    log::info!("Keyboard capture started automatically");
                }
            } else {
                log::warn!("Accessibility permission not granted, requesting...");
                request_accessibility_permission();
            }

            let vim_state_for_ipc = Arc::clone(&vim_state);
            let app_handle_for_ipc = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let handler = move |cmd: IpcCommand| -> IpcResponse {
                    let mut state = vim_state_for_ipc.lock().unwrap();
                    handle_ipc_command(&mut state, &app_handle_for_ipc, cmd)
                };

                if let Err(e) = ipc::start_ipc_server(handler).await {
                    log::error!("IPC server error: {}", e);
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
