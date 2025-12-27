// Allow unexpected_cfgs from the objc crate's macros which use cfg(feature = "cargo-clippy")
#![allow(unexpected_cfgs)]

mod commands;
mod config;
pub mod ipc;
mod keyboard;
mod keyboard_handler;
mod nvim_edit;
mod updater;
mod vim;
mod widgets;
mod window;

use std::sync::{Arc, Mutex};

use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::TrayIcon,
    AppHandle, Emitter, Listener, Manager, State,
};

use commands::RecordedKey;
use config::Settings;
use ipc::{IpcCommand, IpcResponse};
use keyboard::{check_accessibility_permission, request_accessibility_permission, KeyboardCapture};
use keyboard_handler::create_keyboard_callback;
use nvim_edit::EditSessionManager;
use vim::{VimMode, VimState};
use window::setup_indicator_window;

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::OnceLock;

static LOG_FILE: OnceLock<Mutex<std::fs::File>> = OnceLock::new();

fn init_file_logger() {
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open("/tmp/ovim-rust.log")
        .expect("Failed to create log file");

    LOG_FILE.set(Mutex::new(file)).ok();

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            let timestamp = chrono::Local::now().format("%H:%M:%S%.3f");
            let line = format!(
                "[{}] {} - {}\n",
                timestamp,
                record.level(),
                record.args()
            );

            if let Some(file_mutex) = LOG_FILE.get() {
                if let Ok(mut file) = file_mutex.lock() {
                    let _ = file.write_all(line.as_bytes());
                    let _ = file.flush();
                }
            }

            write!(buf, "{}", line)
        })
        .init();
}

/// Application state shared across commands
pub struct AppState {
    pub settings: Arc<Mutex<Settings>>,
    pub vim_state: Arc<Mutex<VimState>>,
    pub keyboard_capture: KeyboardCapture,
    pub record_key_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<RecordedKey>>>>,
    #[allow(dead_code)]
    edit_session_manager: Arc<EditSessionManager>,
}

fn handle_ipc_command(
    state: &mut VimState,
    app_handle: &AppHandle,
    cmd: IpcCommand,
) -> IpcResponse {
    match cmd {
        IpcCommand::GetMode => IpcResponse::Mode(state.mode().as_str().to_string()),
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
        IpcCommand::SetMode(mode_str) => handle_set_mode(state, app_handle, &mode_str),
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

fn update_tray_icon(tray: &TrayIcon, mode: &str, show_mode: bool) {
    let icon_bytes: &[u8] = if show_mode {
        match mode {
            "insert" => include_bytes!("../icons/tray-icon-insert.png"),
            "normal" => include_bytes!("../icons/tray-icon-normal.png"),
            "visual" => include_bytes!("../icons/tray-icon-visual.png"),
            _ => include_bytes!("../icons/tray-icon.png"),
        }
    } else {
        include_bytes!("../icons/tray-icon.png")
    };

    match image::load_from_memory(icon_bytes) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            let icon = Image::new_owned(rgba.into_raw(), width, height);
            if let Err(e) = tray.set_icon(Some(icon)) {
                log::error!("Failed to set tray icon: {}", e);
            }
        }
        Err(e) => {
            log::error!("Failed to decode tray icon: {}", e);
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::check_permission,
            commands::request_permission,
            commands::get_permission_status,
            commands::open_accessibility_settings,
            commands::open_input_monitoring_settings,
            commands::get_vim_mode,
            commands::get_settings,
            commands::set_settings,
            commands::start_capture,
            commands::stop_capture,
            commands::is_capture_running,
            commands::open_settings_window,
            commands::pick_app,
            commands::get_selection_info,
            commands::get_battery_info,
            commands::get_caps_lock_state,
            commands::get_pending_keys,
            commands::get_key_display_name,
            commands::record_key,
            commands::cancel_record_key,
            commands::webview_log,
            commands::validate_nvim_edit_paths,
            commands::set_indicator_ignores_mouse,
            commands::is_command_key_pressed,
            commands::is_mouse_over_indicator,
            commands::get_version,
            commands::check_for_update,
            commands::restart_app,
            commands::set_indicator_clickable,
        ])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let settings_item =
                MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
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

                let initial_settings = Settings::load();
                if let Err(e) = tray.set_visible(initial_settings.show_in_menu_bar) {
                    log::error!("Failed to set initial tray visibility: {}", e);
                }

                let tray_clone = tray.clone();
                app.listen("settings-changed", move |event| {
                    if let Ok(new_settings) = serde_json::from_str::<Settings>(event.payload()) {
                        if let Err(e) = tray_clone.set_visible(new_settings.show_in_menu_bar) {
                            log::error!("Failed to update tray visibility: {}", e);
                        }
                        // Update tray icon when show_mode_in_menu_bar changes
                        update_tray_icon(&tray_clone, "insert", new_settings.show_mode_in_menu_bar);
                    }
                });

                // Listen for mode changes to update tray icon
                let tray_for_mode = tray.clone();
                let app_handle_for_tray = app.handle().clone();
                app.listen("mode-change", move |event| {
                    let mode = event.payload().trim_matches('"');
                    let state: State<AppState> = app_handle_for_tray.state();
                    let show_mode = state.settings.lock().map(|s| s.show_mode_in_menu_bar).unwrap_or(false);
                    update_tray_icon(&tray_for_mode, mode, show_mode);
                });
            }

            if let Some(indicator_window) = app.get_webview_window("indicator") {
                if let Err(e) = setup_indicator_window(&indicator_window) {
                    log::error!("Failed to setup indicator window: {}", e);
                }
            }

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

            // Start periodic update checker
            let state: State<AppState> = app.state();
            updater::start_update_checker(app.handle().clone(), Arc::clone(&state.settings));

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
