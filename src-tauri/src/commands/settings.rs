//! Settings-related Tauri commands

use std::process::Command;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::config::Settings;
use crate::nvim_edit::terminals::ensure_launcher_script;
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
    // Update click mode timing settings
    crate::click_mode::accessibility::update_timing_settings(
        new_settings.click_mode.cache_ttl_ms,
        new_settings.click_mode.ax_stabilization_delay_ms,
    );

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

/// Open the launcher script in the user's configured terminal and editor
#[tauri::command]
pub fn open_launcher_script(state: State<AppState>) -> Result<(), String> {
    // Ensure the script exists
    let script_path = ensure_launcher_script()?;
    let script_path_str = script_path.to_string_lossy().to_string();

    // Get the user's configured settings
    let settings = state.settings.lock().unwrap();
    let mut nvim_edit_clone = settings.nvim_edit.clone();
    drop(settings);

    // Disable custom script for editing the script itself (avoid circular dependency)
    nvim_edit_clone.use_custom_script = false;

    log::info!(
        "Opening launcher script {:?} with terminal {}",
        script_path,
        nvim_edit_clone.terminal
    );

    // Use spawn_terminal to open the script in the configured terminal
    // We pass None for geometry (fullscreen) and None for socket (no RPC needed)
    match crate::nvim_edit::terminals::spawn_terminal(
        &nvim_edit_clone,
        &script_path,
        None, // No popup geometry - open fullscreen
        None, // No RPC socket needed
    ) {
        Ok(spawn_info) => {
            log::info!("Launched terminal with PID: {:?}", spawn_info.process_id);
            Ok(())
        }
        Err(e) => {
            log::error!("Failed to spawn terminal for script editing: {}", e);
            // Fallback to 'open -t' if terminal spawn fails
            Command::new("open")
                .arg("-t")
                .arg(&script_path_str)
                .spawn()
                .map_err(|e2| format!("Failed to open launcher script: {} (fallback: {})", e, e2))?;
            Ok(())
        }
    }
}
