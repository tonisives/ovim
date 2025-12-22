//! Ghostty terminal spawner

use std::process::Command;

use super::process_utils::{find_editor_pid_for_file, resolve_command_path};
use super::{SpawnInfo, TerminalSpawner, TerminalType, WindowGeometry};
use crate::config::NvimEditSettings;

pub struct GhosttySpawner;

impl TerminalSpawner for GhosttySpawner {
    fn terminal_type(&self) -> TerminalType {
        TerminalType::Ghostty
    }

    fn spawn(
        &self,
        settings: &NvimEditSettings,
        file_path: &str,
        geometry: Option<WindowGeometry>,
    ) -> Result<SpawnInfo, String> {
        // Generate a unique window title so we can find it
        let unique_title = format!("ovim-edit-{}", std::process::id());

        // Get editor path and args from settings
        let editor_path = settings.editor_path();
        let editor_args = settings.editor_args();
        let process_name = settings.editor_process_name();

        // Resolve editor path to absolute path
        let resolved_editor = resolve_command_path(&editor_path);
        log::info!("Resolved editor path: {} -> {}", editor_path, resolved_editor);

        // On macOS, Ghostty can be launched via `open -na Ghostty.app --args ...`
        // or directly via the binary if user provides custom path
        let terminal_path = settings.get_terminal_path();
        let use_direct_binary = !terminal_path.is_empty()
            && terminal_path != "ghostty"
            && terminal_path.starts_with('/');

        let mut cmd = if use_direct_binary {
            log::info!("Using direct Ghostty binary: {}", terminal_path);
            Command::new(&terminal_path)
        } else {
            let mut c = Command::new("open");
            c.args(["-na", "Ghostty.app", "--args"]);
            c
        };

        // Add window title
        cmd.args([&format!("--title={}", unique_title)]);

        // Add geometry if provided
        if let Some(ref geo) = geometry {
            // Ghostty window-width/height are in terminal grid cells, not pixels
            let cols = (geo.width / 8).max(10);
            let rows = (geo.height / 16).max(4);
            cmd.args([
                &format!("--window-width={}", cols),
                &format!("--window-height={}", rows),
                &format!("--window-position-x={}", geo.x),
                &format!("--window-position-y={}", geo.y),
            ]);
        }

        // Execute editor using -e flag
        cmd.arg("-e");
        cmd.arg(&resolved_editor);
        for arg in &editor_args {
            cmd.arg(arg);
        }
        cmd.arg(file_path);

        cmd.spawn()
            .map_err(|e| format!("Failed to spawn ghostty: {}", e))?;

        // Wait a bit for editor to start, then find its PID by the file it's editing
        let pid = find_editor_pid_for_file(file_path, process_name);
        log::info!("Found editor PID: {:?} for file: {}", pid, file_path);

        Ok(SpawnInfo {
            terminal_type: TerminalType::Ghostty,
            process_id: pid,
            child: None, // open command returns immediately
            window_title: Some(unique_title),
        })
    }
}
