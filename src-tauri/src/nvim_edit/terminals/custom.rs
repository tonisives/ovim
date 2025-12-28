//! Custom terminal spawner - uses user-defined launcher script
//!
//! When use_custom_script is enabled, this spawner runs the user's launcher script
//! which handles all spawning logic (e.g., tmux popup, custom terminal, etc.)
//!
//! The script signals its intent via IPC:
//! - `ovim launcher-handled --session <id>` - Script handled spawning
//! - `ovim launcher-fallthrough --session <id>` - Use normal terminal flow
//!
//! If no IPC callback is received within timeout, falls back to PID detection.

use std::collections::HashMap;
use std::path::Path;
use std::process::{Child, Command};
use std::time::Duration;

use super::process_utils::find_editor_pid_for_file_no_delay;
use super::{ensure_launcher_script, SpawnInfo, TerminalSpawner, TerminalType, WindowGeometry};
use crate::config::NvimEditSettings;
use crate::launcher_callback::{self, LauncherCallback};

/// Result of running the launcher script
pub enum LauncherResult {
    /// Script handled spawning - contains the SpawnInfo
    Handled(SpawnInfo),
    /// Script wants normal terminal flow
    Fallthrough,
    /// Script failed with an error
    Error(String),
}

/// Run the launcher script and wait for IPC callback
///
/// The script should call either:
/// - `ovim launcher-handled --session $OVIM_SESSION_ID [--pid <pid>]`
/// - `ovim launcher-fallthrough --session $OVIM_SESSION_ID`
///
/// If no callback within 10 seconds, times out with error.
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

    // Generate unique session ID
    let session_id = uuid::Uuid::new_v4().to_string();

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

    log::info!("Running launcher script: {:?}", script_path);
    log::info!(
        "Session ID: {}, OVIM_FILE={}, OVIM_EDITOR={}, OVIM_SOCKET={}, OVIM_TERMINAL={}",
        session_id,
        file_path,
        editor_path,
        socket,
        terminal
    );

    // Register callback channel before spawning script
    let callback_rx = launcher_callback::register(session_id.clone());

    // Get the path to the ovim CLI binary from config directory
    let ovim_cli = super::get_ovim_cli_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "ovim".to_string());

    // Spawn the script with session ID
    let mut child = match Command::new(&script_path)
        .env("OVIM_CLI", &ovim_cli)
        .env("OVIM_SESSION_ID", &session_id)
        .env("OVIM_FILE", file_path)
        .env("OVIM_EDITOR", &editor_path)
        .env("OVIM_WIDTH", width.to_string())
        .env("OVIM_HEIGHT", height.to_string())
        .env("OVIM_X", x.to_string())
        .env("OVIM_Y", y.to_string())
        .env("OVIM_SOCKET", &socket)
        .env("OVIM_TERMINAL", terminal)
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            launcher_callback::unregister(&session_id);
            return LauncherResult::Error(format!("Failed to spawn launcher script: {}", e));
        }
    };

    let script_pid = child.id();
    log::info!("Launcher script started with PID: {}", script_pid);

    // Wait for IPC callback with timeout
    let timeout = Duration::from_secs(10);
    let result = wait_for_callback_or_exit(
        callback_rx,
        &mut child,
        &session_id,
        file_path,
        process_name,
        timeout,
    );

    // Cleanup
    launcher_callback::unregister(&session_id);

    result
}

/// Wait for either IPC callback or script exit
fn wait_for_callback_or_exit(
    callback_rx: tokio::sync::oneshot::Receiver<LauncherCallback>,
    child: &mut Child,
    session_id: &str,
    file_path: &str,
    process_name: &str,
    timeout: Duration,
) -> LauncherResult {
    use std::sync::mpsc;
    use std::thread;

    // We need to poll both the oneshot channel and child process
    // Since we're in sync context, use a background thread for the channel
    let (tx, rx) = mpsc::channel();
    let session_id_clone = session_id.to_string();

    thread::spawn(move || {
        // Block on the oneshot receiver
        // This is a bit awkward but works for our use case
        match callback_rx.blocking_recv() {
            Ok(callback) => {
                let _ = tx.send(Some(callback));
            }
            Err(_) => {
                // Channel closed without value
                let _ = tx.send(None);
            }
        }
    });

    let poll_interval = Duration::from_millis(50);
    let start = std::time::Instant::now();

    loop {
        // Check for IPC callback
        if let Ok(Some(callback)) = rx.try_recv() {
            log::info!("Received IPC callback for session {}", session_id_clone);
            return match callback {
                LauncherCallback::Handled { editor_pid } => {
                    log::info!("Script signaled handled, editor_pid: {:?}", editor_pid);
                    LauncherResult::Handled(SpawnInfo {
                        terminal_type: TerminalType::Custom,
                        process_id: editor_pid,
                        child: None,
                        window_title: None,
                    })
                }
                LauncherCallback::Fallthrough => {
                    log::info!("Script signaled fallthrough");
                    LauncherResult::Fallthrough
                }
            };
        }

        // Check if script has exited without sending callback
        if let Ok(Some(status)) = child.try_wait() {
            let exit_code = status.code().unwrap_or(-1);
            log::info!(
                "Launcher script exited with code {} without IPC callback",
                exit_code
            );

            if exit_code != 0 {
                return LauncherResult::Error(format!(
                    "Launcher script exited with code {}",
                    exit_code
                ));
            }

            // Script exited 0 without callback - try to detect editor PID
            if let Some(pid) = find_editor_pid_for_file_no_delay(file_path, process_name) {
                log::info!("Found editor PID {} after script exited", pid);
                return LauncherResult::Handled(SpawnInfo {
                    terminal_type: TerminalType::Custom,
                    process_id: Some(pid),
                    child: None,
                    window_title: None,
                });
            }

            // No editor found - assume fallthrough for backwards compatibility
            log::info!("Script exited 0 with no callback and no editor - falling through");
            return LauncherResult::Fallthrough;
        }

        // Check timeout
        if start.elapsed() > timeout {
            log::warn!("Launcher script timed out waiting for IPC callback");
            return LauncherResult::Error(
                "Launcher script timed out without sending callback".to_string(),
            );
        }

        thread::sleep(poll_interval);
    }
}

use super::process_utils::find_editor_pid_for_file;

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
        // When terminal=custom, use run_launcher_script which handles IPC callbacks
        match run_launcher_script(settings, file_path, geometry.as_ref(), socket_path) {
            LauncherResult::Handled(info) => Ok(info),
            LauncherResult::Fallthrough => {
                // Fallthrough doesn't make sense for terminal=custom
                // Fall back to spawning script directly without IPC
                spawn_script_directly(settings, file_path, geometry, socket_path)
            }
            LauncherResult::Error(e) => Err(e),
        }
    }
}

/// Spawn script directly without IPC (fallback for terminal=custom with fallthrough)
fn spawn_script_directly(
    settings: &NvimEditSettings,
    file_path: &str,
    geometry: Option<WindowGeometry>,
    socket_path: Option<&Path>,
) -> Result<SpawnInfo, String> {
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

    log::info!("Spawning custom terminal with script (direct): {:?}", script_path);

    let child = Command::new(&script_path)
        .env("OVIM_SESSION_ID", uuid::Uuid::new_v4().to_string())
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
