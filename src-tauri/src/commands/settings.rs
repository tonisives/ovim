//! Settings-related Tauri commands

use tauri::{AppHandle, Emitter, Manager, State};

use crate::config::Settings;
use crate::AppState;

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Settings {
    let settings = state.settings.lock().unwrap();
    settings.clone()
}

#[tauri::command]
pub fn set_settings(
    app: AppHandle,
    state: State<AppState>,
    new_settings: Settings,
) -> Result<(), String> {
    let mut settings = state.settings.lock().unwrap();
    *settings = new_settings.clone();
    settings.save()?;

    let _ = app.emit("settings-changed", new_settings);
    Ok(())
}

#[tauri::command]
pub fn open_settings_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn pick_app() -> Result<Option<String>, String> {
    use std::process::Command;

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
        return Ok(None);
    }

    let app_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if app_path.is_empty() {
        return Ok(None);
    }

    let bundle_output = Command::new("mdls")
        .args(["-name", "kMDItemCFBundleIdentifier", "-raw", &app_path])
        .output()
        .map_err(|e| format!("Failed to get bundle ID: {}", e))?;

    let bundle_id = String::from_utf8_lossy(&bundle_output.stdout)
        .trim()
        .to_string();
    if bundle_id.is_empty() || bundle_id == "(null)" {
        return Err("Could not determine bundle identifier".to_string());
    }

    Ok(Some(bundle_id))
}
