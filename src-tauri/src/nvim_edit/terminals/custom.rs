//! Custom terminal spawner - uses user-defined launcher script
//!
//! When use_custom_script is enabled, this spawner runs the user's launcher script
//! which handles all spawning logic (e.g., tmux popup, custom terminal, etc.)
//!
//! The script is responsible for:
//! - Focusing the correct window
//! - Spawning the editor
//! - Any custom window management
//!
//! If the script exits with code 0 and doesn't spawn an editor, the normal
//! terminal flow continues (fallback behavior).

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use super::process_utils::{find_editor_pid_for_file, find_pid_with_file_open};
use super::{ensure_launcher_script, SpawnInfo, TerminalSpawner, TerminalType, WindowGeometry};
use crate::config::NvimEditSettings;

/// Result of running the launcher script
pub enum LauncherResult {
    /// Script handled spawning - contains the SpawnInfo
    Handled(SpawnInfo),
    /// Script exited with 0 and didn't spawn editor - fall through to normal flow
    Fallthrough,
    /// Script failed with an error
    Error(String),
}

/// Run the launcher script and check if it spawned an editor
///
/// Returns `LauncherResult::Fallthrough` if the script exits with 0 and no editor is found,
/// allowing the normal terminal spawning to proceed.
pub fn run_launcher_script(
    settings: &NvimEditSettings,
    file_path: &str,
    geometry: Option<&WindowGeometry>,
    socket_path: Option<&Path>,
) -> LauncherResult {
    let script_path = match ensure_launcher_script() {
        Ok(p) => p,
        Err(e) => return LauncherResult::Error(e),
    };

    let editor_path = settings.editor_path();
    let terminal = &settings.terminal;

    let width = geometry.map_or(800, |g| g.width);
    let height = geometry.map_or(600, |g| g.height);
    let x = geometry.map_or(0, |g| g.x);
    let y = geometry.map_or(0, |g| g.y);
    let socket = socket_path
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    log::info!(
        "Running launcher script: {:?}",
        script_path
    );
    log::info!(
        "Environment: OVIM_FILE={}, OVIM_EDITOR={}, OVIM_SOCKET={}, OVIM_TERMINAL={}",
        file_path,
        editor_path,
        socket,
        terminal
    );

    // Run the script and wait for it to complete
    let result = Command::new(&script_path)
        .env("OVIM_FILE", file_path)
        .env("OVIM_EDITOR", &editor_path)
        .env("OVIM_WIDTH", width.to_string())
        .env("OVIM_HEIGHT", height.to_string())
        .env("OVIM_X", x.to_string())
        .env("OVIM_Y", y.to_string())
        .env("OVIM_SOCKET", &socket)
        .env("OVIM_TERMINAL", terminal)
        .output();

    match result {
        Ok(output) => {
            let exit_code = output.status.code().unwrap_or(-1);
            log::info!("Launcher script exited with code: {}", exit_code);

            if exit_code != 0 {
                // Script failed or explicitly signaled it handled things
                let stderr = String::from_utf8_lossy(&output.stderr);
                if !stderr.is_empty() {
                    log::warn!("Launcher script stderr: {}", stderr);
                }
                return LauncherResult::Error(format!(
                    "Launcher script exited with code {}",
                    exit_code
                ));
            }

            // Script exited with 0 - check if it spawned an editor
            // No delay - if the script spawned something synchronously, it should have the file open
            let editor_pid = find_pid_with_file_open(file_path);

            if let Some(pid) = editor_pid {
                log::info!("Launcher script spawned editor with PID: {}", pid);
                LauncherResult::Handled(SpawnInfo {
                    terminal_type: TerminalType::Custom,
                    process_id: Some(pid),
                    child: None,
                    window_title: None,
                })
            } else {
                log::info!("Launcher script exited 0 with no editor - falling through to normal flow");
                LauncherResult::Fallthrough
            }
        }
        Err(e) => LauncherResult::Error(format!("Failed to run launcher script: {}", e)),
    }
}

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
        let terminal = &settings.terminal;

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
            "Environment: OVIM_FILE={}, OVIM_EDITOR={}, OVIM_SOCKET={}, OVIM_TERMINAL={}",
            file_path,
            editor_path,
            socket,
            terminal
        );

        // Spawn the launcher script with environment variables
        // The script handles focusing, spawning, and window management
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
