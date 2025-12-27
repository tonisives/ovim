//! Custom terminal spawner - uses user-defined launcher script
//!
//! When use_custom_script is enabled, this spawner first runs the launcher script.
//! The script can either:
//! - Handle spawning itself (exit with non-zero or block until done)
//! - Exit with 0 to tell ovim to use built-in terminal spawning
//!
//! This allows scripts to customize PATH/env while still using built-in terminals.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use super::process_utils::find_editor_pid_for_file;
use super::{ensure_launcher_script, SpawnInfo, TerminalSpawner, TerminalType, WindowGeometry};
use crate::config::NvimEditSettings;

pub struct CustomSpawner;

/// Result of running the custom script
pub enum CustomScriptResult {
    /// Script handled spawning, here's the spawn info
    Handled(SpawnInfo),
    /// Script exited with 0, use built-in terminal spawning
    UseBuiltIn,
}

impl CustomSpawner {
    /// Run the launcher script and determine what to do next
    pub fn run_script(
        settings: &NvimEditSettings,
        file_path: &str,
        geometry: Option<&WindowGeometry>,
        socket_path: Option<&Path>,
    ) -> Result<CustomScriptResult, String> {
        let script_path = ensure_launcher_script()?;

        let editor_path = settings.editor_path();
        let process_name = settings.editor_process_name();
        let terminal = &settings.terminal;

        let width = geometry.map_or(800, |g| g.width);
        let height = geometry.map_or(600, |g| g.height);
        let x = geometry.map_or(0, |g| g.x);
        let y = geometry.map_or(0, |g| g.y);
        let socket = socket_path
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        log::info!(
            "Running custom launcher script: {:?}",
            script_path
        );

        // Run the script and wait for it to complete
        let output = Command::new(&script_path)
            .env("OVIM_FILE", file_path)
            .env("OVIM_EDITOR", &editor_path)
            .env("OVIM_WIDTH", width.to_string())
            .env("OVIM_HEIGHT", height.to_string())
            .env("OVIM_X", x.to_string())
            .env("OVIM_Y", y.to_string())
            .env("OVIM_SOCKET", &socket)
            .env("OVIM_TERMINAL", terminal)
            .output()
            .map_err(|e| format!("Failed to run launcher script: {}", e))?;

        // If script exits with 0, use built-in terminal spawning
        if output.status.success() {
            log::info!("Launcher script exited with 0, using built-in terminal spawning");
            return Ok(CustomScriptResult::UseBuiltIn);
        }

        // Script handled spawning (exited non-zero or spawned something)
        // Try to find the editor process
        log::info!("Launcher script exited with {:?}, looking for editor process", output.status.code());

        std::thread::sleep(std::time::Duration::from_millis(200));
        let editor_pid = find_editor_pid_for_file(file_path, process_name);
        log::info!("Found editor PID: {:?} for file: {}", editor_pid, file_path);

        Ok(CustomScriptResult::Handled(SpawnInfo {
            terminal_type: TerminalType::Custom,
            process_id: editor_pid,
            child: None,
            window_title: None,
        }))
    }
}

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
        // This is called when terminal=custom in settings (not use_custom_script)
        // In this case, we expect the script to handle everything
        let script_path = ensure_launcher_script()?;

        let editor_path = settings.editor_path();
        let process_name = settings.editor_process_name();
        let terminal = &settings.terminal;

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

        // Spawn the launcher script (don't wait)
        let child = Command::new(&script_path)
            .env("OVIM_FILE", file_path)
            .env("OVIM_EDITOR", &editor_path)
            .env("OVIM_WIDTH", width.to_string())
            .env("OVIM_HEIGHT", height.to_string())
            .env("OVIM_X", x.to_string())
            .env("OVIM_Y", y.to_string())
            .env("OVIM_SOCKET", &socket)
            .env("OVIM_TERMINAL", terminal)
            .spawn()
            .map_err(|e| format!("Failed to spawn launcher script: {}", e))?;

        let script_pid = child.id();
        log::info!("Launcher script started with PID: {}", script_pid);

        std::thread::sleep(std::time::Duration::from_millis(500));
        let editor_pid = find_editor_pid_for_file(file_path, process_name);
        log::info!("Found editor PID: {:?} for file: {}", editor_pid, file_path);

        let process_id = editor_pid.or(Some(script_pid));

        Ok(SpawnInfo {
            terminal_type: TerminalType::Custom,
            process_id,
            child: Some(child),
            window_title: None,
        })
    }
}
