//! Terminal spawning for Edit Popup feature
//!
//! This module provides a unified interface for spawning different terminal emulators
//! with various text editors (Neovim, Vim, Helix, etc.)

mod alacritty;
pub mod applescript_utils;
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
    ///
    /// If `text_is_empty` is true, the editor should start in insert mode.
    fn spawn(
        &self,
        settings: &NvimEditSettings,
        file_path: &str,
        geometry: Option<WindowGeometry>,
        socket_path: Option<&Path>,
        custom_env: Option<&HashMap<String, String>>,
        text_is_empty: bool,
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
///
/// If `text_is_empty` is true, the editor should start in insert mode.
pub fn spawn_terminal(
    settings: &NvimEditSettings,
    temp_file: &Path,
    geometry: Option<WindowGeometry>,
    socket_path: Option<&Path>,
    text_is_empty: bool,
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
        TerminalType::Alacritty => AlacrittySpawner.spawn(settings, &file_path, geometry, socket_path, None, text_is_empty),
        TerminalType::Ghostty => GhosttySpawner.spawn(settings, &file_path, geometry, socket_path, None, text_is_empty),
        TerminalType::Kitty => KittySpawner.spawn(settings, &file_path, geometry, socket_path, None, text_is_empty),
        TerminalType::WezTerm => WezTermSpawner.spawn(settings, &file_path, geometry, socket_path, None, text_is_empty),
        TerminalType::ITerm => ITermSpawner.spawn(settings, &file_path, geometry, socket_path, None, text_is_empty),
        TerminalType::Custom => CustomSpawner.spawn(settings, &file_path, geometry, socket_path, None, text_is_empty),
        TerminalType::Default => TerminalAppSpawner.spawn(settings, &file_path, geometry, socket_path, None, text_is_empty),
    }
}

/// Get the launcher script path, ensuring it exists
/// The script is copied from bundled resources on first use
pub fn ensure_launcher_script() -> Result<std::path::PathBuf, String> {
    let script_path = Settings::launcher_script_path()
        .ok_or("Could not determine config directory")?;

    if !script_path.exists() {
        return Err(format!(
            "Launcher script not found at {:?}. It should be installed on app startup.",
            script_path
        ));
    }

    Ok(script_path)
}

/// Copy a script file and make it executable
#[cfg(unix)]
fn copy_script(source: &std::path::Path, dest: &std::path::Path) -> Result<(), String> {
    std::fs::copy(source, dest)
        .map_err(|e| format!("Failed to copy {:?}: {}", source.file_name().unwrap_or_default(), e))?;

    use std::os::unix::fs::PermissionsExt;
    if let Ok(metadata) = std::fs::metadata(dest) {
        let mut perms = metadata.permissions();
        perms.set_mode(0o755);
        let _ = std::fs::set_permissions(dest, perms);
    }
    Ok(())
}

#[cfg(not(unix))]
fn copy_script(source: &std::path::Path, dest: &std::path::Path) -> Result<(), String> {
    std::fs::copy(source, dest)
        .map_err(|e| format!("Failed to copy {:?}: {}", source.file_name().unwrap_or_default(), e))?;
    Ok(())
}

/// Install scripts from app bundle to config directory
/// Called on app startup to ensure users have access to launcher and sample scripts
pub fn install_scripts(app_handle: &tauri::AppHandle) -> Result<(), String> {
    let config_dir = dirs::config_dir()
        .ok_or("Could not determine config directory")?
        .join("ovim");

    // Create config directory if needed
    std::fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create config directory: {}", e))?;

    // Get the resource path from the app bundle
    let resource_path = app_handle
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to get resource directory: {}", e))?;

    let scripts_source = resource_path.join("scripts");

    // Install the default launcher script if it doesn't exist
    let launcher_source = scripts_source.join("terminal-launcher.sh");
    let launcher_dest = config_dir.join("terminal-launcher.sh");
    if launcher_source.exists() && !launcher_dest.exists() {
        match copy_script(&launcher_source, &launcher_dest) {
            Ok(()) => log::info!("Installed launcher script: {:?}", launcher_dest),
            Err(e) => log::warn!("{}", e),
        }
    }

    // Install sample scripts
    let samples_source = scripts_source.join("samples");
    let samples_dest = config_dir.join("samples");

    if samples_source.exists() {
        std::fs::create_dir_all(&samples_dest)
            .map_err(|e| format!("Failed to create samples directory: {}", e))?;

        if let Ok(entries) = std::fs::read_dir(&samples_source) {
            for entry in entries.flatten() {
                let source = entry.path();
                if source.is_file() {
                    let filename = source.file_name().unwrap();
                    let dest = samples_dest.join(filename);

                    // Only copy if destination doesn't exist (don't overwrite user modifications)
                    if !dest.exists() {
                        match copy_script(&source, &dest) {
                            Ok(()) => log::info!("Installed sample script: {:?}", dest),
                            Err(e) => log::warn!("{}", e),
                        }
                    }
                }
            }
        }
    } else {
        log::debug!("No bundled sample scripts found at {:?}", samples_source);
    }

    // Copy the ovim CLI binary to config dir (always overwrite to keep in sync)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let cli_source = exe_dir.join("ovim");
            let cli_dest = config_dir.join("ovim");

            if cli_source.exists() {
                match std::fs::copy(&cli_source, &cli_dest) {
                    Ok(_) => {
                        // Make executable
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            if let Ok(metadata) = std::fs::metadata(&cli_dest) {
                                let mut perms = metadata.permissions();
                                perms.set_mode(0o755);
                                let _ = std::fs::set_permissions(&cli_dest, perms);
                            }
                        }
                        log::info!("Installed ovim CLI: {:?}", cli_dest);
                    }
                    Err(e) => log::warn!("Failed to copy ovim CLI: {}", e),
                }
            }
        }
    }

    Ok(())
}

/// Get the path to the ovim CLI in the config directory
pub fn get_ovim_cli_path() -> Option<std::path::PathBuf> {
    dirs::config_dir()
        .map(|p| p.join("ovim").join("ovim"))
        .filter(|p| p.exists())
}
