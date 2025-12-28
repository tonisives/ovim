//! Update-related commands

use tauri::{AppHandle, Manager};

use crate::updater;
use crate::window;

/// Get the current application version
#[tauri::command]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Check for updates and install if available
/// Returns the new version if an update was installed
#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> Result<Option<String>, String> {
    updater::check_and_install_update(&app).await
}

/// Restart the application to apply the installed update
#[tauri::command]
pub fn restart_app(app: AppHandle) {
    app.restart();
}

/// Set whether the indicator window ignores mouse events
#[tauri::command]
pub fn set_indicator_clickable(app: AppHandle, clickable: bool) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("indicator") {
        window::set_indicator_ignores_mouse(&window, !clickable)
    } else {
        Err("Indicator window not found".to_string())
    }
}
