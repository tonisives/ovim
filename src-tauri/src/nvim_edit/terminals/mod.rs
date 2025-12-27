//! Terminal spawning for Edit Popup feature
//!
//! This module provides a unified interface for spawning different terminal emulators
//! with various text editors (Neovim, Vim, Helix, etc.)

mod alacritty;
mod applescript_utils;
mod custom;
mod ghostty;
mod iterm;
mod kitty;
pub mod process_utils;
mod terminal_app;
mod wezterm;

pub use alacritty::AlacrittySpawner;
pub use custom::{CustomSpawner, LauncherResult, run_launcher_script};
pub use ghostty::GhosttySpawner;
pub use iterm::ITermSpawner;
pub use kitty::KittySpawner;
pub use terminal_app::TerminalAppSpawner;
pub use wezterm::WezTermSpawner;

use crate::config::{NvimEditSettings, Settings};
use std::collections::HashMap;
use std::path::Path;
use std::process::Child;
use tauri::Manager;

/// Window position and size for popup mode
#[derive(Debug, Clone, Default)]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Terminal types supported
#[derive(Debug, Clone, PartialEq)]
pub enum TerminalType {
    Alacritty,
    Ghostty,
    Kitty,
    WezTerm,
    ITerm,
    Custom,
    Default, // Terminal.app
}

impl TerminalType {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "alacritty" => TerminalType::Alacritty,
            "ghostty" => TerminalType::Ghostty,
            "kitty" => TerminalType::Kitty,
            "wezterm" => TerminalType::WezTerm,
            "iterm" | "iterm2" => TerminalType::ITerm,
            "custom" => TerminalType::Custom,
            _ => TerminalType::Default,
        }
    }

    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            TerminalType::Alacritty => "alacritty",
            TerminalType::Ghostty => "ghostty",
            TerminalType::Kitty => "kitty",
            TerminalType::WezTerm => "wezterm",
            TerminalType::ITerm => "iterm",
            TerminalType::Custom => "custom",
            TerminalType::Default => "default",
        }
    }
}

/// Spawn info returned after launching terminal
pub struct SpawnInfo {
    pub terminal_type: TerminalType,
    pub process_id: Option<u32>,
    #[allow(dead_code)]
    pub child: Option<Child>,
    pub window_title: Option<String>,
}

/// Trait for terminal spawners
pub trait TerminalSpawner {
    /// The terminal type this spawner handles
    #[allow(dead_code)]
    fn terminal_type(&self) -> TerminalType;

    /// Spawn a terminal with the configured editor editing the given file
    ///
    /// If `socket_path` is provided, the editor will be started with RPC enabled
    /// (e.g., nvim --listen <socket_path>) for live buffer sync.
    ///
    /// If `custom_env` is provided, these environment variables will be applied
    /// to the spawned process (from the launcher script).
    fn spawn(
        &self,
        settings: &NvimEditSettings,
        file_path: &str,
        geometry: Option<WindowGeometry>,
        socket_path: Option<&Path>,
        custom_env: Option<&HashMap<String, String>>,
    ) -> Result<SpawnInfo, String>;
}

/// Spawn a terminal with the configured editor editing the given file
///
/// If `socket_path` is provided, the editor will be started with RPC enabled
/// for live buffer sync.
///
/// If `use_custom_script` is enabled, the launcher script runs first:
/// - If the script spawns an editor (detected by PID), we use that
/// - If the script exits with 0 and no editor spawned, fall through to normal terminal
/// - If the script fails (non-zero exit), return error
///
/// The terminal selection is passed via OVIM_TERMINAL env var for the script to use if needed.
pub fn spawn_terminal(
    settings: &NvimEditSettings,
    temp_file: &Path,
    geometry: Option<WindowGeometry>,
    socket_path: Option<&Path>,
) -> Result<SpawnInfo, String> {
    let terminal_type = TerminalType::from_string(&settings.terminal);
    let file_path = temp_file.to_string_lossy();

    // If custom script is enabled, run it first
    if settings.use_custom_script {
        match run_launcher_script(settings, &file_path, geometry.as_ref(), socket_path) {
            LauncherResult::Handled(info) => return Ok(info),
            LauncherResult::Fallthrough => {
                log::info!("Launcher script returned fallthrough, continuing with normal terminal spawn");
                // Continue to normal terminal spawning below
            }
            LauncherResult::Error(e) => return Err(e),
        }
    }

    match terminal_type {
        TerminalType::Alacritty => AlacrittySpawner.spawn(settings, &file_path, geometry, socket_path, None),
        TerminalType::Ghostty => GhosttySpawner.spawn(settings, &file_path, geometry, socket_path, None),
        TerminalType::Kitty => KittySpawner.spawn(settings, &file_path, geometry, socket_path, None),
        TerminalType::WezTerm => WezTermSpawner.spawn(settings, &file_path, geometry, socket_path, None),
        TerminalType::ITerm => ITermSpawner.spawn(settings, &file_path, geometry, socket_path, None),
        TerminalType::Custom => CustomSpawner.spawn(settings, &file_path, geometry, socket_path, None),
        TerminalType::Default => TerminalAppSpawner.spawn(settings, &file_path, geometry, socket_path, None),
    }
}

/// Wait for the terminal/nvim process to exit
pub fn wait_for_process(
    terminal_type: &TerminalType,
    process_id: Option<u32>,
) -> Result<(), String> {
    match terminal_type {
        TerminalType::Alacritty
        | TerminalType::Ghostty
        | TerminalType::Kitty
        | TerminalType::WezTerm
        | TerminalType::Custom => {
            if let Some(pid) = process_id {
                process_utils::wait_for_pid(pid)
            } else {
                Err("No process ID to wait for".to_string())
            }
        }
        TerminalType::ITerm | TerminalType::Default => {
            if let Some(pid) = process_id {
                process_utils::wait_for_pid(pid)
            } else {
                // Fallback: wait a fixed time (not ideal)
                std::thread::sleep(std::time::Duration::from_secs(60));
                Ok(())
            }
        }
    }
}

/// Get the default launcher script content
pub fn default_launcher_script() -> &'static str {
    r#"#!/bin/bash
# ovim terminal launcher script
#
# This script runs before the terminal/editor spawns. You can use it to:
# 1. Set up environment variables (PATH, etc.) for the editor
# 2. Optionally spawn the editor yourself (for custom terminals like tmux)
#
# Exit codes:
#   exit 0  - If no editor spawned: fall through to normal terminal flow
#           - If editor spawned: ovim will wait for that editor process
#   exit 1+ - Error, abort the edit popup
#
# Available environment variables:
#   OVIM_FILE     - temp file path to edit
#   OVIM_EDITOR   - configured editor executable
#   OVIM_SOCKET   - RPC socket path (for live sync)
#   OVIM_TERMINAL - selected terminal type
#   OVIM_WIDTH    - popup width in pixels
#   OVIM_HEIGHT   - popup height in pixels
#   OVIM_X        - popup x position
#   OVIM_Y        - popup y position

# Example: Add homebrew and local bins to PATH
# export PATH="/opt/homebrew/bin:$HOME/.local/bin:$PATH"

# Example: Spawn in tmux popup (blocks until editor closes)
# tmux popup -E -w 80% -h 80% "$OVIM_EDITOR --listen $OVIM_SOCKET $OVIM_FILE"
# exit 0

# Example: Focus existing tmux window and spawn there
# tmux send-keys -t :editor "$OVIM_EDITOR --listen $OVIM_SOCKET $OVIM_FILE" Enter
# exit 0

# Default: exit 0 to continue with normal terminal flow
exit 0
"#
}

/// Ensure the launcher script exists, creating it with default content if not
pub fn ensure_launcher_script() -> Result<std::path::PathBuf, String> {
    let script_path = Settings::launcher_script_path()
        .ok_or("Could not determine config directory")?;

    if !script_path.exists() {
        // Create parent directory if needed
        if let Some(parent) = script_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        // Write default script
        std::fs::write(&script_path, default_launcher_script())
            .map_err(|e| format!("Failed to write launcher script: {}", e))?;

        // Make executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&script_path)
                .map_err(|e| format!("Failed to get script permissions: {}", e))?
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&script_path, perms)
                .map_err(|e| format!("Failed to set script permissions: {}", e))?;
        }

        log::info!("Created launcher script at {:?}", script_path);
    }

    Ok(script_path)
}

/// Copy sample scripts from app bundle to config directory
/// Called on app startup to ensure users have access to sample scripts
pub fn install_sample_scripts(app_handle: &tauri::AppHandle) -> Result<(), String> {
    let config_dir = dirs::config_dir()
        .ok_or("Could not determine config directory")?
        .join("ovim")
        .join("samples");

    // Create samples directory if needed
    std::fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create samples directory: {}", e))?;

    // Get the resource path from the app bundle
    let resource_path = app_handle
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to get resource directory: {}", e))?;

    let samples_source = resource_path.join("scripts").join("samples");

    // Copy each sample script if it doesn't exist
    if samples_source.exists() {
        if let Ok(entries) = std::fs::read_dir(&samples_source) {
            for entry in entries.flatten() {
                let source = entry.path();
                if source.is_file() {
                    let filename = source.file_name().unwrap();
                    let dest = config_dir.join(filename);

                    // Only copy if destination doesn't exist (don't overwrite user modifications)
                    if !dest.exists() {
                        if let Err(e) = std::fs::copy(&source, &dest) {
                            log::warn!("Failed to copy sample script {:?}: {}", filename, e);
                        } else {
                            // Make executable
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                if let Ok(metadata) = std::fs::metadata(&dest) {
                                    let mut perms = metadata.permissions();
                                    perms.set_mode(0o755);
                                    let _ = std::fs::set_permissions(&dest, perms);
                                }
                            }
                            log::info!("Installed sample script: {:?}", dest);
                        }
                    }
                }
            }
        }
    } else {
        log::debug!("No bundled sample scripts found at {:?}", samples_source);
    }

    Ok(())
}
