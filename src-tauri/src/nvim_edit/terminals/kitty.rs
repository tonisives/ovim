//! Kitty terminal spawner

use std::process::Command;

use super::process_utils::{find_nvim_pid_for_file, resolve_command_path};
use super::{SpawnInfo, TerminalSpawner, TerminalType, WindowGeometry};

pub struct KittySpawner;

impl TerminalSpawner for KittySpawner {
    fn terminal_type(&self) -> TerminalType {
        TerminalType::Kitty
    }

    fn spawn(
        &self,
        nvim_path: &str,
        file_path: &str,
        geometry: Option<WindowGeometry>,
    ) -> Result<SpawnInfo, String> {
        // Generate a unique window title
        let unique_title = format!("ovim-edit-{}", std::process::id());

        // Resolve nvim path
        let resolved_nvim = resolve_command_path(nvim_path);
        log::info!("Resolved nvim path: {} -> {}", nvim_path, resolved_nvim);

        // Try to find kitty - check common locations on macOS
        let kitty_path =
            if std::path::Path::new("/Applications/kitty.app/Contents/MacOS/kitty").exists() {
                "/Applications/kitty.app/Contents/MacOS/kitty"
            } else {
                "kitty" // Fall back to PATH
            };

        let mut cmd = Command::new(kitty_path);

        // Use single instance to avoid multiple dock icons, close window when nvim exits
        cmd.args(["--single-instance", "--wait-for-single-instance-window-close"]);
        cmd.args(["--title", &unique_title]);
        cmd.args(["-o", "close_on_child_death=yes"]);

        // Add window position/size if provided
        if let Some(ref geo) = geometry {
            cmd.args([
                "--position",
                &format!("{}x{}", geo.x, geo.y),
                "-o",
                &format!("initial_window_width={}c", geo.width / 8),
                "-o",
                &format!("initial_window_height={}c", geo.height / 16),
                "-o",
                "remember_window_size=no",
            ]);
        }

        // Kitty runs the command directly (no -e flag needed)
        cmd.args([&resolved_nvim, "+normal G$", file_path]);

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn kitty: {}", e))?;

        // Wait a bit for nvim to start, then find its PID by the file it's editing
        let pid = find_nvim_pid_for_file(file_path);
        log::info!("Found nvim PID: {:?} for file: {}", pid, file_path);

        Ok(SpawnInfo {
            terminal_type: TerminalType::Kitty,
            process_id: pid,
            child: Some(child),
            window_title: Some(unique_title),
        })
    }
}
