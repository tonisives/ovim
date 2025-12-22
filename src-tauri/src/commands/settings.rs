//! Settings-related Tauri commands

use std::path::Path;
use std::process::Command;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::config::Settings;
use crate::nvim_edit::terminals::process_utils::{resolve_command_path, resolve_terminal_path};
use crate::AppState;

/// Result of validating terminal and editor paths
#[derive(Debug, Clone, Serialize)]
pub struct PathValidation {
    /// Whether the terminal exists and is executable
    pub terminal_valid: bool,
    /// The resolved terminal path (or empty if not found)
    pub terminal_resolved_path: String,
    /// Error message for terminal (if any)
    pub terminal_error: Option<String>,
    /// Whether the editor exists and is executable
    pub editor_valid: bool,
    /// The resolved editor path (or empty if not found)
    pub editor_resolved_path: String,
    /// Error message for editor (if any)
    pub editor_error: Option<String>,
}

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

/// Check if a path points to an executable file
fn is_executable(path: &str) -> bool {
    let path = Path::new(path);
    if !path.exists() {
        return false;
    }

    // On macOS, check if it's executable using std::fs::metadata
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(path) {
            let mode = metadata.permissions().mode();
            return mode & 0o111 != 0; // Check if any execute bit is set
        }
    }

    false
}

/// Check if a terminal can be run (handles AppleScript-based terminals)
fn validate_terminal(terminal_type: &str, terminal_path: &str) -> (bool, String, Option<String>) {
    // iTerm and Terminal.app use AppleScript, so they're always "available" via osascript
    if terminal_type == "iterm" || terminal_type == "default" {
        // Check if the app exists in /Applications
        let app_path = match terminal_type {
            "iterm" => "/Applications/iTerm.app",
            "default" => "/Applications/Utilities/Terminal.app",
            _ => "",
        };

        if !app_path.is_empty() && Path::new(app_path).exists() {
            return (true, app_path.to_string(), None);
        } else if terminal_type == "default" {
            // Terminal.app should always exist on macOS
            return (true, "Terminal.app".to_string(), None);
        }

        return (
            false,
            String::new(),
            Some(format!(
                "{} not found at {}",
                if terminal_type == "iterm" {
                    "iTerm2"
                } else {
                    "Terminal.app"
                },
                app_path
            )),
        );
    }

    // For other terminals, resolve and check the path
    let terminal_cmd = if terminal_path.is_empty() {
        terminal_type.to_string()
    } else {
        terminal_path.to_string()
    };

    let resolved = resolve_terminal_path(&terminal_cmd);

    // Check if the resolved path exists and is executable
    if is_executable(&resolved) {
        return (true, resolved, None);
    }

    // If not found, provide a helpful error message
    let terminal_name = match terminal_type {
        "alacritty" => "Alacritty",
        "kitty" => "Kitty",
        "wezterm" => "WezTerm",
        "ghostty" => "Ghostty",
        _ => terminal_type,
    };

    let error_msg = if terminal_path.is_empty() {
        format!(
            "{} not found. Install it or specify a custom path.",
            terminal_name
        )
    } else {
        format!("Path '{}' not found or not executable", terminal_path)
    };

    (false, String::new(), Some(error_msg))
}

/// Check if an editor can be run
fn validate_editor(editor_type: &str, editor_path: &str) -> (bool, String, Option<String>) {
    // Determine the command to check
    let editor_cmd = if editor_path.is_empty() {
        match editor_type {
            "neovim" => "nvim".to_string(),
            "vim" => "vim".to_string(),
            "helix" => "hx".to_string(),
            "custom" => {
                return (
                    false,
                    String::new(),
                    Some("Custom editor requires a path".to_string()),
                );
            }
            _ => editor_type.to_string(),
        }
    } else {
        editor_path.to_string()
    };

    let resolved = resolve_command_path(&editor_cmd);

    // Check if the resolved path exists and is executable
    if is_executable(&resolved) {
        return (true, resolved, None);
    }

    // Also check if it can be found via `which` (for shell-based resolution)
    if let Ok(output) = Command::new("which").arg(&editor_cmd).output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() && is_executable(&path) {
                return (true, path, None);
            }
        }
    }

    // Provide a helpful error message
    let editor_name = match editor_type {
        "neovim" => "Neovim (nvim)",
        "vim" => "Vim",
        "helix" => "Helix (hx)",
        "custom" => "Editor",
        _ => editor_type,
    };

    let error_msg = if editor_path.is_empty() {
        format!(
            "{} not found. Install it or specify a custom path.",
            editor_name
        )
    } else {
        format!("Path '{}' not found or not executable", editor_path)
    };

    (false, String::new(), Some(error_msg))
}

#[tauri::command]
pub fn validate_nvim_edit_paths(
    terminal_type: String,
    terminal_path: String,
    editor_type: String,
    editor_path: String,
) -> PathValidation {
    let (terminal_valid, terminal_resolved, terminal_error) =
        validate_terminal(&terminal_type, &terminal_path);

    let (editor_valid, editor_resolved, editor_error) =
        validate_editor(&editor_type, &editor_path);

    PathValidation {
        terminal_valid,
        terminal_resolved_path: terminal_resolved,
        terminal_error,
        editor_valid,
        editor_resolved_path: editor_resolved,
        editor_error,
    }
}
