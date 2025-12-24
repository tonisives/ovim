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
pub use custom::CustomSpawner;
pub use ghostty::GhosttySpawner;
pub use iterm::ITermSpawner;
pub use kitty::KittySpawner;
pub use terminal_app::TerminalAppSpawner;
pub use wezterm::WezTermSpawner;

use crate::config::{NvimEditSettings, Settings};
use std::path::Path;
use std::process::Child;

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
    fn spawn(
        &self,
        settings: &NvimEditSettings,
        file_path: &str,
        geometry: Option<WindowGeometry>,
        socket_path: Option<&Path>,
    ) -> Result<SpawnInfo, String>;
}

/// Spawn a terminal with the configured editor editing the given file
///
/// If `socket_path` is provided, the editor will be started with RPC enabled
/// for live buffer sync.
///
/// If `use_custom_script` is enabled, the launcher script handles the entire
/// spawn process, allowing custom PATH, tmux popups, etc.
pub fn spawn_terminal(
    settings: &NvimEditSettings,
    temp_file: &Path,
    geometry: Option<WindowGeometry>,
    socket_path: Option<&Path>,
) -> Result<SpawnInfo, String> {
    let terminal_type = TerminalType::from_string(&settings.terminal);
    let file_path = temp_file.to_string_lossy();

    // If custom script is enabled, use the CustomSpawner which runs the launcher script
    if settings.use_custom_script {
        return CustomSpawner.spawn(settings, &file_path, geometry, socket_path);
    }

    // For built-in terminals, run the launcher script first (if exists) to set up environment
    run_launcher_script_for_env(settings, &file_path, &geometry, socket_path, &terminal_type);

    match terminal_type {
        TerminalType::Alacritty => AlacrittySpawner.spawn(settings, &file_path, geometry, socket_path),
        TerminalType::Ghostty => GhosttySpawner.spawn(settings, &file_path, geometry, socket_path),
        TerminalType::Kitty => KittySpawner.spawn(settings, &file_path, geometry, socket_path),
        TerminalType::WezTerm => WezTermSpawner.spawn(settings, &file_path, geometry, socket_path),
        TerminalType::ITerm => ITermSpawner.spawn(settings, &file_path, geometry, socket_path),
        TerminalType::Custom => CustomSpawner.spawn(settings, &file_path, geometry, socket_path),
        TerminalType::Default => TerminalAppSpawner.spawn(settings, &file_path, geometry, socket_path),
    }
}

/// Run the launcher script to set up environment (PATH, etc.) for built-in terminals.
/// The script's environment changes won't affect the parent process, but we can
/// capture them and apply to the terminal spawn.
fn run_launcher_script_for_env(
    _settings: &NvimEditSettings,
    _file_path: &str,
    _geometry: &Option<WindowGeometry>,
    _socket_path: Option<&Path>,
    _terminal_type: &TerminalType,
) {
    // For now, built-in terminals don't inherit from the launcher script
    // because each spawner creates its own process.
    // The launcher script is primarily for Custom terminal type.
    // Future enhancement: could source the script and capture env vars.
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
# Customize PATH, environment, or terminal behavior here.

# Example: Add homebrew and local bins to PATH
# export PATH="/opt/homebrew/bin:$HOME/.local/bin:$PATH"

# Available environment variables:
#   OVIM_FILE     - temp file path to edit
#   OVIM_EDITOR   - configured editor executable
#   OVIM_WIDTH    - popup width in pixels
#   OVIM_HEIGHT   - popup height in pixels
#   OVIM_X        - popup x position
#   OVIM_Y        - popup y position
#   OVIM_SOCKET   - RPC socket path (for live sync)
#   OVIM_TERMINAL - selected terminal type

# For custom terminal (OVIM_TERMINAL=custom), handle spawning yourself:
if [ "$OVIM_TERMINAL" = "custom" ]; then
    # Example: tmux popup
    # tmux popup -E -w 80% -h 80% "$OVIM_EDITOR $OVIM_FILE"

    # Default: just run editor directly
    exec $OVIM_EDITOR $OVIM_FILE
fi

# For built-in terminals, exit 0 to let ovim handle spawning
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
