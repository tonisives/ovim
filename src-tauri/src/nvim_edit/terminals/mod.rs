//! Terminal spawning for "Edit with Neovim" feature
//!
//! This module provides a unified interface for spawning different terminal emulators.

mod alacritty;
mod applescript_utils;
mod ghostty;
mod iterm;
mod kitty;
mod process_utils;
mod terminal_app;
mod wezterm;

pub use alacritty::AlacrittySpawner;
pub use ghostty::GhosttySpawner;
pub use iterm::ITermSpawner;
pub use kitty::KittySpawner;
pub use terminal_app::TerminalAppSpawner;
pub use wezterm::WezTermSpawner;

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
            _ => TerminalType::Default,
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

    /// Spawn a terminal with nvim editing the given file
    fn spawn(
        &self,
        nvim_path: &str,
        file_path: &str,
        geometry: Option<WindowGeometry>,
    ) -> Result<SpawnInfo, String>;
}

/// Spawn a terminal with nvim editing the given file
pub fn spawn_terminal(
    terminal: &str,
    nvim_path: &str,
    temp_file: &Path,
    geometry: Option<WindowGeometry>,
) -> Result<SpawnInfo, String> {
    let terminal_type = TerminalType::from_string(terminal);
    let file_path = temp_file.to_string_lossy();

    match terminal_type {
        TerminalType::Alacritty => AlacrittySpawner.spawn(nvim_path, &file_path, geometry),
        TerminalType::Ghostty => GhosttySpawner.spawn(nvim_path, &file_path, geometry),
        TerminalType::Kitty => KittySpawner.spawn(nvim_path, &file_path, geometry),
        TerminalType::WezTerm => WezTermSpawner.spawn(nvim_path, &file_path, geometry),
        TerminalType::ITerm => ITermSpawner.spawn(nvim_path, &file_path, geometry),
        TerminalType::Default => TerminalAppSpawner.spawn(nvim_path, &file_path, geometry),
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
        | TerminalType::WezTerm => {
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
