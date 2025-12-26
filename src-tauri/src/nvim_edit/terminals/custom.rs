//! Custom terminal spawner - uses user-defined launcher script
//!
//! When terminal=custom, this spawner runs the user's launcher script which
//! must handle all spawning logic (e.g., tmux popup, custom terminal, etc.)

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use super::process_utils::find_editor_pid_for_file;
use super::{ensure_launcher_script, SpawnInfo, TerminalSpawner, TerminalType, WindowGeometry};
use crate::config::NvimEditSettings;

pub struct CustomSpawner;

impl TerminalSpawner for CustomSpawner {
    fn terminal_type(&self) -> TerminalType {
        TerminalType::Custom
    }

    fn spawn(
        &self,
        settings: &NvimEditSettings,
        file_path: &str,
        geometry: Option<WindowGeometry>,
        socket_path: Option<&Path>,
        _custom_env: Option<&HashMap<String, String>>,
    ) -> Result<SpawnInfo, String> {
        // Ensure launcher script exists
        let script_path = ensure_launcher_script()?;

        let editor_path = settings.editor_path();
        let process_name = settings.editor_process_name();

        // Build environment variables
        let width = geometry.as_ref().map_or(800, |g| g.width);
        let height = geometry.as_ref().map_or(600, |g| g.height);
        let x = geometry.as_ref().map_or(0, |g| g.x);
        let y = geometry.as_ref().map_or(0, |g| g.y);
        let socket = socket_path
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        log::info!(
            "Spawning custom terminal with script: {:?}",
            script_path
        );
        log::info!(
            "Environment: OVIM_FILE={}, OVIM_EDITOR={}, OVIM_SOCKET={}",
            file_path,
            editor_path,
            socket
        );

        // Spawn the launcher script with environment variables
        let child = Command::new(&script_path)
            .env("OVIM_FILE", file_path)
            .env("OVIM_EDITOR", &editor_path)
            .env("OVIM_WIDTH", width.to_string())
            .env("OVIM_HEIGHT", height.to_string())
            .env("OVIM_X", x.to_string())
            .env("OVIM_Y", y.to_string())
            .env("OVIM_SOCKET", &socket)
            .env("OVIM_TERMINAL", "custom")
            .spawn()
            .map_err(|e| format!("Failed to spawn launcher script: {}", e))?;

        let script_pid = child.id();
        log::info!("Launcher script started with PID: {}", script_pid);

        // Wait a bit for the editor to start, then find its PID
        std::thread::sleep(std::time::Duration::from_millis(500));
        let editor_pid = find_editor_pid_for_file(file_path, process_name);
        log::info!("Found editor PID: {:?} for file: {}", editor_pid, file_path);

        // If we couldn't find the editor PID, use the script PID as fallback
        let process_id = editor_pid.or(Some(script_pid));

        Ok(SpawnInfo {
            terminal_type: TerminalType::Custom,
            process_id,
            child: Some(child),
            window_title: None,
        })
    }
}
