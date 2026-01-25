// Allow unexpected_cfgs from the objc crate's macros which use cfg(feature = "cargo-clippy")
#![allow(unexpected_cfgs)]

mod click_mode;
mod commands;
mod config;
pub mod ipc;
mod keyboard;
mod keyboard_handler;
pub mod launcher_callback;
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

use click_mode::SharedClickModeManager;
use commands::RecordedKey;
use config::click_mode::DoubleTapModifier;
use config::Settings;
use ipc::{IpcCommand, IpcResponse};
use keyboard::{check_accessibility_permission, request_accessibility_permission, KeyboardCapture};
use keyboard_handler::create_keyboard_callback;
use keyboard_handler::double_tap::{DoubleTapKey, DoubleTapManager};
use nvim_edit::terminals::install_scripts;
use nvim_edit::EditSessionManager;
use vim::{VimMode, VimState};
use window::{setup_click_overlay_window, setup_indicator_window};

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::OnceLock;

static LOG_FILE: OnceLock<Mutex<std::fs::File>> = OnceLock::new();
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// Get a reference to the global app handle (for emitting events from keyboard handler)
pub fn get_app_handle() -> Option<&'static AppHandle> {
    APP_HANDLE.get()
}

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
    pub click_mode_manager: SharedClickModeManager,
}

fn handle_ipc_command(
    state: &mut VimState,
    app_handle: &AppHandle,
    settings: &Arc<Mutex<Settings>>,
    edit_session_manager: &Arc<EditSessionManager>,
    click_mode_manager: &SharedClickModeManager,
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
        IpcCommand::EditPopup => {
            let nvim_settings = {
                let s = settings.lock().unwrap();
                if !s.nvim_edit.enabled {
                    return IpcResponse::Error("Edit Popup is disabled".to_string());
                }
                s.nvim_edit.clone()
            };
            let manager = Arc::clone(edit_session_manager);
            std::thread::spawn(move || {
                if let Err(e) = nvim_edit::trigger_nvim_edit(manager, nvim_settings) {
                    log::error!("Failed to trigger nvim edit via IPC: {}", e);
                }
            });
            IpcResponse::Ok
        }
        IpcCommand::ClickMode => {
            let is_enabled = {
                let s = settings.lock().unwrap();
                s.click_mode.enabled
            };
            if !is_enabled {
                return IpcResponse::Error("Click Mode is disabled".to_string());
            }

            // Set click mode to activating state
            {
                let mut mgr = click_mode_manager.lock().unwrap();
                if mgr.is_active() {
                    return IpcResponse::Error("Click Mode is already active".to_string());
                }
                mgr.set_activating();
            }

            let manager = Arc::clone(click_mode_manager);
            std::thread::spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut mgr = manager.lock().unwrap();
                    match mgr.activate() {
                        Ok(elements) => {
                            log::info!("Click mode activated via IPC with {} elements", elements.len());
                            let style = click_mode::native_hints::HintStyle::default();
                            click_mode::native_hints::show_hints(&elements, &style);
                            if let Some(app) = get_app_handle() {
                                let _ = app.emit("click-mode-activated", ());
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to activate click mode via IPC: {}", e);
                            mgr.deactivate();
                        }
                    }
                }));

                if let Err(e) = result {
                    log::error!("Panic in click mode activation via IPC: {:?}", e);
                    if let Ok(mut mgr) = manager.lock() {
                        mgr.deactivate();
                    }
                }
            });
            IpcResponse::Ok
        }
        IpcCommand::LauncherHandled {
            session_id,
            editor_pid,
        } => {
            if launcher_callback::signal_handled(&session_id, editor_pid) {
                log::info!(
                    "Launcher signaled handled for session {}, pid: {:?}",
                    session_id,
                    editor_pid
                );
                IpcResponse::Ok
            } else {
                log::warn!("Unknown launcher session: {}", session_id);
                IpcResponse::Error(format!("Unknown session: {}", session_id))
            }
        }
        IpcCommand::LauncherFallthrough { session_id } => {
            if launcher_callback::signal_fallthrough(&session_id) {
                log::info!("Launcher signaled fallthrough for session {}", session_id);
                IpcResponse::Ok
            } else {
                log::warn!("Unknown launcher session: {}", session_id);
                IpcResponse::Error(format!("Unknown session: {}", session_id))
            }
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

/// Helper to check if a double-tap key matches a setting
fn matches_double_tap_setting(setting: &DoubleTapModifier, key: &DoubleTapKey) -> bool {
    match (setting, key) {
        (DoubleTapModifier::Command, DoubleTapKey::Command) => true,
        (DoubleTapModifier::Option, DoubleTapKey::Option) => true,
        (DoubleTapModifier::Control, DoubleTapKey::Control) => true,
        (DoubleTapModifier::Shift, DoubleTapKey::Shift) => true,
        (DoubleTapModifier::Escape, DoubleTapKey::Escape) => true,
        _ => false,
    }
}

/// Handle double-tap activation for click mode or nvim edit
fn handle_double_tap_activation(
    double_tap_key: DoubleTapKey,
    settings: &Arc<Mutex<Settings>>,
    click_mode_manager: &SharedClickModeManager,
    edit_session_manager: &Arc<EditSessionManager>,
) {
    let settings_guard = settings.lock().unwrap();

    // Check if this double-tap should trigger click mode
    let click_mode_trigger = matches_double_tap_setting(
        &settings_guard.click_mode.double_tap_modifier,
        &double_tap_key,
    );

    // Check if this double-tap should trigger nvim edit mode
    let nvim_edit_trigger = matches_double_tap_setting(
        &settings_guard.nvim_edit.double_tap_modifier,
        &double_tap_key,
    );

    // Don't allow both to be triggered by the same key
    // Click mode takes priority if both are set to the same key
    if click_mode_trigger && settings_guard.click_mode.enabled {
        log::info!("Double-tap {:?} detected - activating click mode", double_tap_key);
        drop(settings_guard);

        // Activate click mode
        {
            let mut mgr = click_mode_manager.lock().unwrap();
            if !mgr.is_active() {
                mgr.set_activating();
            }
        }

        let manager = Arc::clone(click_mode_manager);
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut mgr = manager.lock().unwrap();
                match mgr.activate() {
                    Ok(elements) => {
                        log::info!("Click mode activated via double-tap with {} elements", elements.len());
                        let style = click_mode::native_hints::HintStyle::default();
                        click_mode::native_hints::show_hints(&elements, &style);
                        if let Some(app) = get_app_handle() {
                            let _ = app.emit("click-mode-activated", ());
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to activate click mode via double-tap: {}", e);
                        mgr.deactivate();
                    }
                }
            }));

            if let Err(e) = result {
                log::error!("Panic in click mode activation via double-tap: {:?}", e);
                if let Ok(mut mgr) = manager.lock() {
                    mgr.deactivate();
                }
            }
        });
    } else if nvim_edit_trigger && settings_guard.nvim_edit.enabled {
        log::info!("Double-tap {:?} detected - activating nvim edit", double_tap_key);
        let nvim_settings = settings_guard.nvim_edit.clone();
        drop(settings_guard);

        // Trigger nvim edit
        let manager = Arc::clone(edit_session_manager);
        std::thread::spawn(move || {
            if let Err(e) = nvim_edit::trigger_nvim_edit(manager, nvim_settings) {
                log::error!("Failed to trigger nvim edit via double-tap: {}", e);
            }
        });
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

    // Initialize the accessibility helper binary
    click_mode::accessibility::init_helper();

    let (vim_state, mode_rx) = VimState::new();
    let vim_state = Arc::new(Mutex::new(vim_state));

    let settings = Arc::new(Mutex::new(Settings::load()));

    // Initialize click mode settings from loaded settings
    {
        let s = settings.lock().unwrap();
        click_mode::accessibility::update_timing_settings(
            s.click_mode.cache_ttl_ms,
            s.click_mode.ax_stabilization_delay_ms,
            s.click_mode.max_depth,
            s.click_mode.max_elements,
        );
    }

    let record_key_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<RecordedKey>>>> =
        Arc::new(Mutex::new(None));
    let edit_session_manager = Arc::new(EditSessionManager::new());
    let click_mode_manager = click_mode::create_manager();
    let double_tap_manager = Arc::new(Mutex::new(DoubleTapManager::new()));

    // Create double-tap callback that handles mode activation
    let double_tap_callback = {
        let settings_for_dt = Arc::clone(&settings);
        let click_manager_for_dt = Arc::clone(&click_mode_manager);
        let edit_session_manager_for_dt = Arc::clone(&edit_session_manager);

        Box::new(move |double_tap_key: DoubleTapKey| {
            handle_double_tap_activation(
                double_tap_key,
                &settings_for_dt,
                &click_manager_for_dt,
                &edit_session_manager_for_dt,
            );
        }) as keyboard_handler::DoubleTapCallback
    };

    let keyboard_capture = KeyboardCapture::new();
    keyboard_capture.set_callback(create_keyboard_callback(
        Arc::clone(&vim_state),
        Arc::clone(&settings),
        Arc::clone(&record_key_tx),
        Arc::clone(&edit_session_manager),
        Arc::clone(&click_mode_manager),
        Arc::clone(&double_tap_manager),
        double_tap_callback,
    ));

    // Set up mouse click callback to hide click mode on any mouse click
    {
        let click_manager_for_mouse = Arc::clone(&click_mode_manager);
        keyboard_capture.set_mouse_callback(move |_event| {
            // Check if click mode is active and deactivate it
            if let Ok(mut mgr) = click_manager_for_mouse.try_lock() {
                if mgr.is_active() {
                    log::info!("Mouse click detected - deactivating click mode");
                    mgr.deactivate();
                    click_mode::native_hints::hide_hints();
                }
            }
            true // Always pass through mouse events
        });
    }

    // Set up scroll callback to hide click mode on scroll
    {
        let click_manager_for_scroll = Arc::clone(&click_mode_manager);
        keyboard_capture.set_scroll_callback(move || {
            // Check if click mode is active and deactivate it
            if let Ok(mut mgr) = click_manager_for_scroll.try_lock() {
                if mgr.is_active() {
                    log::info!("Scroll detected - deactivating click mode");
                    mgr.deactivate();
                    click_mode::native_hints::hide_hints();
                    // Notify frontend to update indicator
                    if let Some(app) = get_app_handle() {
                        let _ = app.emit("click-mode-deactivated", ());
                    }
                }
            }
        });
    }

    // Set up flags changed callback for double-tap modifier shortcuts
    {
        let settings_for_flags = Arc::clone(&settings);
        let click_manager_for_flags = Arc::clone(&click_mode_manager);
        let edit_session_manager_for_flags = Arc::clone(&edit_session_manager);
        let double_tap_manager_for_flags = Arc::clone(&double_tap_manager);

        keyboard_capture.set_flags_changed_callback(move |modifiers| {
            let mut dt_manager = double_tap_manager_for_flags.lock().unwrap();

            // Process the flags change and check for double-tap
            if let Some(double_tap_key) = dt_manager.process_flags_changed(
                modifiers.command,
                modifiers.option,
                modifiers.control,
                modifiers.shift,
            ) {
                drop(dt_manager);
                handle_double_tap_activation(
                    double_tap_key,
                    &settings_for_flags,
                    &click_manager_for_flags,
                    &edit_session_manager_for_flags,
                );
            }
        });
    }

    // Set up focus change observer to hide click mode when app loses focus
    // and prefetch elements for the new app
    {
        let click_manager_for_focus = Arc::clone(&click_mode_manager);
        click_mode::start_focus_observer(move || {
            // Invalidate cache since we're now on a different app
            click_mode::accessibility::invalidate_cache();

            // Check if click mode is active and deactivate it
            if let Ok(mut mgr) = click_manager_for_focus.try_lock() {
                if mgr.is_active() {
                    log::info!("App focus changed - deactivating click mode");
                    mgr.deactivate();
                    click_mode::native_hints::hide_hints();
                }
            }

            // Prefetch elements for the new app in background
            // This warms the cache so click mode activation is faster
            click_mode::accessibility::prefetch_elements();
        });
    }

    // Clone for IPC handler before moving into app_state
    let settings_for_ipc = Arc::clone(&settings);
    let edit_session_manager_for_ipc = Arc::clone(&edit_session_manager);
    let click_mode_manager_for_ipc = Arc::clone(&click_mode_manager);

    let app_state = AppState {
        settings,
        vim_state: Arc::clone(&vim_state),
        keyboard_capture,
        record_key_tx,
        edit_session_manager,
        click_mode_manager,
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
            commands::open_launcher_script,
            commands::set_indicator_ignores_mouse,
            commands::is_command_key_pressed,
            commands::is_mouse_over_indicator,
            commands::get_version,
            commands::check_for_update,
            commands::restart_app,
            commands::set_indicator_clickable,
            // Click mode commands
            commands::activate_click_mode,
            commands::deactivate_click_mode,
            commands::get_click_mode_state,
            commands::click_mode_click_element,
            commands::click_mode_right_click_element,
            commands::click_mode_input_hint,
            commands::get_click_mode_elements,
        ])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Store app handle for global access (used by keyboard handler for events)
            let _ = APP_HANDLE.set(app.handle().clone());

            // Initialize launcher callback registry
            launcher_callback::init();

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

            // Set up click overlay window (hidden initially)
            if let Some(click_overlay) = app.get_webview_window("click-overlay") {
                if let Err(e) = setup_click_overlay_window(&click_overlay) {
                    log::error!("Failed to setup click overlay window: {}", e);
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

            // Install launcher and sample scripts to config directory
            if let Err(e) = install_scripts(app.handle()) {
                log::warn!("Failed to install scripts: {}", e);
            }

            let vim_state_for_ipc2 = Arc::clone(&vim_state);
            let app_handle_for_ipc = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let handler = move |cmd: IpcCommand| -> IpcResponse {
                    let mut state = vim_state_for_ipc2.lock().unwrap();
                    handle_ipc_command(
                        &mut state,
                        &app_handle_for_ipc,
                        &settings_for_ipc,
                        &edit_session_manager_for_ipc,
                        &click_mode_manager_for_ipc,
                        cmd,
                    )
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
