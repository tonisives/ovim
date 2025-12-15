mod config;
pub mod ipc;
mod keyboard;
mod vim;
mod window;

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{
    menu::{Menu, MenuItem},
    AppHandle, Emitter, Manager, State,
};

use config::Settings;
use ipc::{IpcCommand, IpcResponse};
use keyboard::{check_accessibility_permission, request_accessibility_permission, KeyboardCapture, KeyCode, KeyEvent};
use vim::{ProcessResult, VimAction, VimMode, VimState};
use window::setup_indicator_window;

/// Execute a VimAction on a separate thread with a small delay
fn execute_action_async(action: VimAction) {
    thread::spawn(move || {
        thread::sleep(Duration::from_micros(500));
        if let Err(e) = action.execute() {
            log::error!("Failed to execute vim action: {}", e);
        }
    });
}

/// Application state shared across commands
pub struct AppState {
    settings: Mutex<Settings>,
    vim_state: Arc<Mutex<VimState>>,
    keyboard_capture: KeyboardCapture,
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

// Helper functions

fn create_keyboard_callback(
    vim_state: Arc<Mutex<VimState>>,
) -> impl Fn(KeyEvent) -> Option<KeyEvent> + Send + 'static {
    move |event| {
        // Check for hyper+0 (Cmd+Ctrl+Option+Shift+0) to toggle vim mode
        if event.is_key_down {
            let is_hyper = event.modifiers.command
                && event.modifiers.control
                && event.modifiers.option
                && event.modifiers.shift;

            if is_hyper && event.keycode() == Some(KeyCode::Num0) {
                let mut state = vim_state.lock().unwrap();
                let new_mode = state.toggle_mode();
                log::info!("Hyper+0: toggled vim mode to {:?}", new_mode);
                return None;
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
    env_logger::init();

    let (vim_state, mode_rx) = VimState::new();
    let vim_state = Arc::new(Mutex::new(vim_state));

    let keyboard_capture = KeyboardCapture::new();
    keyboard_capture.set_callback(create_keyboard_callback(Arc::clone(&vim_state)));

    let settings = Settings::load();
    let app_state = AppState {
        settings: Mutex::new(settings),
        vim_state: Arc::clone(&vim_state),
        keyboard_capture,
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
        ])
        .setup(move |app| {
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
                tray.on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click { button, .. } = event {
                        if button == tauri::tray::MouseButton::Left {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window("settings") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
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
