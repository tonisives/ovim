//! Permission-related Tauri commands

use tauri::{AppHandle, Manager, State};

use crate::keyboard::{check_accessibility_permission, check_input_monitoring_permission};
use crate::AppState;

#[derive(Debug, Clone, serde::Serialize)]
pub struct PermissionStatus {
    pub accessibility: bool,
    pub input_monitoring: bool,
    pub capture_running: bool,
}

#[tauri::command]
pub fn check_permission() -> bool {
    check_accessibility_permission()
}

#[tauri::command]
pub fn get_permission_status(state: State<AppState>) -> PermissionStatus {
    PermissionStatus {
        accessibility: check_accessibility_permission(),
        input_monitoring: check_input_monitoring_permission(),
        capture_running: state.keyboard_capture.is_running(),
    }
}

#[tauri::command]
pub fn complete_permission_setup(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    if !check_accessibility_permission() || !check_input_monitoring_permission() {
        return Err("Both permissions must be allowed before setup can finish".to_string());
    }

    state.keyboard_capture.start()?;
    app.run_on_main_thread(crate::window::hide_permission_drag_helper)
        .map_err(|error| error.to_string())?;
    if let Some(window) = app.get_webview_window("permissions") {
        window.hide().map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn open_permission_settings(
    app: &AppHandle,
    settings_url: &str,
    permission_name: &'static str,
) -> Result<(), String> {
    use std::process::Command;

    Command::new("open")
        .arg(settings_url)
        .spawn()
        .map_err(|error| error.to_string())?;

    app.run_on_main_thread(move || {
        if let Err(error) = crate::window::show_permission_drag_helper(permission_name) {
            log::warn!("Could not show permission drag helper: {error}");
        }
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn open_accessibility_settings(app: AppHandle) -> Result<(), String> {
    open_permission_settings(
        &app,
        "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
        "Accessibility",
    )
}

#[tauri::command]
pub fn open_input_monitoring_settings(app: AppHandle) -> Result<(), String> {
    open_permission_settings(
        &app,
        "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent",
        "Input Monitoring",
    )
}

#[tauri::command]
pub fn start_capture(state: State<AppState>) -> Result<(), String> {
    state.keyboard_capture.start()
}

#[tauri::command]
pub fn stop_capture(state: State<AppState>) {
    state.keyboard_capture.stop()
}

#[tauri::command]
pub fn is_capture_running(state: State<AppState>) -> bool {
    state.keyboard_capture.is_running()
}
