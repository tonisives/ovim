//! Ghostty terminal spawner

use std::process::Command;

use super::process_utils::{find_nvim_pid_for_file, resolve_command_path};
use super::{SpawnInfo, TerminalSpawner, TerminalType, WindowGeometry};

pub struct GhosttySpawner;

impl TerminalSpawner for GhosttySpawner {
    fn terminal_type(&self) -> TerminalType {
        TerminalType::Ghostty
    }

    fn spawn(
        &self,
        nvim_path: &str,
        file_path: &str,
        geometry: Option<WindowGeometry>,
    ) -> Result<SpawnInfo, String> {
        // Generate a unique window title so we can find it
        let unique_title = format!("ovim-edit-{}", std::process::id());

        // Resolve nvim path to absolute path
        let resolved_nvim = resolve_command_path(nvim_path);
        log::info!("Resolved nvim path: {} -> {}", nvim_path, resolved_nvim);

        // On macOS, Ghostty must be launched via `open -na Ghostty.app --args ...`
        let mut cmd = Command::new("open");
        cmd.args(["-na", "Ghostty.app", "--args"]);

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

        // Execute nvim using -e flag
        cmd.args(["-e", &resolved_nvim, "+normal G$", file_path]);

        cmd.spawn()
            .map_err(|e| format!("Failed to spawn ghostty: {}", e))?;

        // Wait a bit for nvim to start, then find its PID by the file it's editing
        let pid = find_nvim_pid_for_file(file_path);
        log::info!("Found nvim PID: {:?} for file: {}", pid, file_path);

        Ok(SpawnInfo {
            terminal_type: TerminalType::Ghostty,
            process_id: pid,
            child: None, // open command returns immediately
            window_title: Some(unique_title),
        })
    }
}
