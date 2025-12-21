//! WezTerm terminal spawner

use std::process::Command;

use super::applescript_utils::set_window_size;
use super::process_utils::resolve_command_path;
use super::{SpawnInfo, TerminalSpawner, TerminalType, WindowGeometry};

pub struct WezTermSpawner;

impl TerminalSpawner for WezTermSpawner {
    fn terminal_type(&self) -> TerminalType {
        TerminalType::WezTerm
    }

    fn spawn(
        &self,
        nvim_path: &str,
        file_path: &str,
        geometry: Option<WindowGeometry>,
    ) -> Result<SpawnInfo, String> {
        // Resolve nvim path
        let resolved_nvim = resolve_command_path(nvim_path);
        log::info!("Resolved nvim path: {} -> {}", nvim_path, resolved_nvim);

        let mut cmd = Command::new("wezterm");

        // Use --always-new-process so wezterm blocks until the command exits.
        // WezTerm only supports --position for window placement (no --width/--height)
        if let Some(ref geo) = geometry {
            cmd.args([
                "start",
                "--always-new-process",
                "--position",
                &format!("screen:{},{}", geo.x, geo.y),
                "--",
            ]);
        } else {
            cmd.args(["start", "--always-new-process", "--"]);
        }

        cmd.args([&resolved_nvim, "+normal G$", file_path]);

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn wezterm: {}", e))?;

        // Get the wezterm process PID - with --always-new-process, the wezterm
        // process itself will block until nvim exits, so we can track it directly
        let wezterm_pid = child.id();
        log::info!("WezTerm process PID: {}", wezterm_pid);

        // If geometry specified, try to resize using AppleScript after window appears
        if let Some(ref geo) = geometry {
            let width = geo.width;
            let height = geo.height;
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(300));
                set_window_size("WezTerm", width, height);
            });
        }

        Ok(SpawnInfo {
            terminal_type: TerminalType::WezTerm,
            process_id: Some(wezterm_pid),
            child: Some(child),
            window_title: None,
        })
    }
}
