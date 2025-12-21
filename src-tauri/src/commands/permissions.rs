//! Permission-related Tauri commands

use tauri::State;

use crate::keyboard::{check_accessibility_permission, request_accessibility_permission};
use crate::AppState;

#[derive(Debug, Clone, serde::Serialize)]
pub struct PermissionStatus {
    pub accessibility: bool,
    pub capture_running: bool,
}

#[tauri::command]
pub fn check_permission() -> bool {
    check_accessibility_permission()
}

#[tauri::command]
pub fn request_permission() -> bool {
    request_accessibility_permission()
}

#[tauri::command]
pub fn get_permission_status(state: State<AppState>) -> PermissionStatus {
    PermissionStatus {
        accessibility: check_accessibility_permission(),
        capture_running: state.keyboard_capture.is_running(),
    }
}

#[tauri::command]
pub fn open_accessibility_settings() {
    use std::process::Command;
    let _ = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();
}

#[tauri::command]
pub fn open_input_monitoring_settings() {
    use std::process::Command;
    let _ = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent")
        .spawn();
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
