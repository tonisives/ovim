//! Pre-warmed terminal management for faster Edit Popup
//!
//! Spawns a hidden Alacritty window with nvim listening on a socket at app startup.
//! When the user triggers an edit, the pre-warmed terminal is claimed, the file is
//! loaded via RPC, and the window is repositioned and shown.

use std::path::PathBuf;
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::terminals::applescript_utils;
use super::terminals::{AlacrittySpawner, WindowGeometry};
use crate::config::NvimEditSettings;

/// State of a pre-warmed terminal instance
struct PrewarmState {
    /// PID of the nvim process (or alacritty child)
    process_id: Option<u32>,
    /// Alacritty child process handle
    _child: Option<Child>,
    /// Window title used to find/move the window
    window_title: String,
    /// Socket path for nvim RPC
    socket_path: PathBuf,
}

/// Manages pre-warmed terminal instances
pub struct PrewarmManager {
    state: Arc<Mutex<Option<PrewarmState>>>,
}

impl PrewarmManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(None)),
        }
    }

    /// Spawn a hidden pre-warmed terminal. Call from app startup.
    pub fn spawn_prewarm(&self, settings: &NvimEditSettings) {
        if settings.terminal != "alacritty" {
            log::info!("Prewarm only supports alacritty, skipping");
            return;
        }

        let socket_path = self.prewarm_socket_path();

        // Clean up any stale socket
        let _ = std::fs::remove_file(&socket_path);

        match AlacrittySpawner::spawn_prewarm_window(settings, &socket_path) {
            Ok(result) => {
                log::info!(
                    "Pre-warmed terminal spawned: title={}, pid={:?}",
                    result.window_title,
                    result.process_id,
                );
                let mut state = self.state.lock().unwrap();
                *state = Some(PrewarmState {
                    process_id: result.process_id,
                    _child: result.child,
                    window_title: result.window_title,
                    socket_path,
                });
            }
            Err(e) => {
                log::warn!("Failed to spawn pre-warmed terminal: {}", e);
            }
        }
    }

    /// Try to claim the pre-warmed terminal for an edit session.
    /// Returns (socket_path, process_id, window_title) if available.
    pub fn try_claim(&self) -> Option<(PathBuf, Option<u32>, String)> {
        let mut state = self.state.lock().unwrap();
        let s = state.take()?;

        // Verify the process is still alive
        if let Some(pid) = s.process_id {
            if !process_alive(pid) {
                log::warn!("Pre-warmed process {} is dead, discarding", pid);
                let _ = std::fs::remove_file(&s.socket_path);
                return None;
            }
        }

        // Verify socket still exists
        if !s.socket_path.exists() {
            log::warn!("Pre-warmed socket {:?} gone, discarding", s.socket_path);
            return None;
        }

        Some((s.socket_path, s.process_id, s.window_title))
    }

    /// Schedule respawning a new pre-warmed terminal in the background.
    pub fn schedule_respawn(&self, settings: NvimEditSettings) {
        let manager_state = Arc::clone(&self.state);
        std::thread::spawn(move || {
            // Delay before respawning so the current edit session can settle
            std::thread::sleep(Duration::from_secs(2));

            // Check we don't already have a prewarm
            {
                let state = manager_state.lock().unwrap();
                if state.is_some() {
                    log::info!("Prewarm already exists, skipping respawn");
                    return;
                }
            }

            let manager = PrewarmManager {
                state: manager_state,
            };
            manager.spawn_prewarm(&settings);
        });
    }

    /// Check if a pre-warmed terminal is available and healthy
    #[allow(dead_code)]
    pub fn is_available(&self) -> bool {
        let state = self.state.lock().unwrap();
        if let Some(ref s) = *state {
            if let Some(pid) = s.process_id {
                return process_alive(pid) && s.socket_path.exists();
            }
            return s.socket_path.exists();
        }
        false
    }

    /// Clean up the pre-warmed terminal (kill process, remove socket)
    pub fn cleanup(&self) {
        let mut state = self.state.lock().unwrap();
        if let Some(s) = state.take() {
            if let Some(pid) = s.process_id {
                log::info!("Cleaning up pre-warmed terminal, killing pid {}", pid);
                unsafe {
                    libc::kill(pid as i32, libc::SIGTERM);
                }
            }
            let _ = std::fs::remove_file(&s.socket_path);
        }
    }

    /// Get the socket path used for pre-warming
    fn prewarm_socket_path(&self) -> PathBuf {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("ovim");
        let _ = std::fs::create_dir_all(&cache_dir);
        cache_dir.join(format!("nvim_prewarm_{}.sock", std::process::id()))
    }
}

impl Drop for PrewarmManager {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// Load a file into the pre-warmed nvim instance via RPC
pub fn load_file_via_rpc(
    socket_path: &std::path::Path,
    file_path: &std::path::Path,
    filetype: Option<&str>,
    text_is_empty: bool,
) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Failed to create tokio runtime: {}", e))?;

    rt.block_on(async {
        load_file_via_rpc_async(socket_path, file_path, filetype, text_is_empty).await
    })
}

async fn load_file_via_rpc_async(
    socket_path: &std::path::Path,
    file_path: &std::path::Path,
    filetype: Option<&str>,
    text_is_empty: bool,
) -> Result<(), String> {
    use nvim_rs::create::tokio::new_path;

    // Simple no-op handler for this one-shot RPC call
    let handler = super::rpc::BufferHandler::new(Arc::new(|_| {}));

    let (neovim, io_handler) = new_path(socket_path, handler)
        .await
        .map_err(|e| format!("Failed to connect to prewarm nvim: {}", e))?;

    tokio::spawn(async move {
        let _ = io_handler.await;
    });

    // Open the file
    let file_str = file_path.to_string_lossy();
    neovim
        .command(&format!("edit {}", file_str))
        .await
        .map_err(|e| format!("Failed to open file in prewarm nvim: {}", e))?;

    // Set filetype if provided
    if let Some(ft) = filetype {
        neovim
            .command(&format!("set ft={}", ft))
            .await
            .map_err(|e| format!("Failed to set filetype: {}", e))?;
    }

    // Start insert mode if text is empty
    if text_is_empty {
        neovim
            .command("startinsert")
            .await
            .map_err(|e| format!("Failed to start insert mode: {}", e))?;
    } else {
        // Position cursor at end of file
        neovim
            .command("normal G$")
            .await
            .map_err(|e| format!("Failed to position cursor: {}", e))?;
    }

    Ok(())
}

/// Show and position the pre-warmed window using AppleScript
pub fn show_and_position(window_title: &str, geometry: &WindowGeometry) {
    applescript_utils::show_and_position_window_by_title(
        &["alacritty", "Alacritty"],
        window_title,
        geometry.x,
        geometry.y,
        geometry.width,
        geometry.height,
    );
}

/// Check if a process is alive
fn process_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}
