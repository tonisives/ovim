//! Terminal.app spawner (macOS default terminal)

use std::process::Command;

use super::process_utils::find_nvim_pid_for_file;
use super::{SpawnInfo, TerminalSpawner, TerminalType, WindowGeometry};

pub struct TerminalAppSpawner;

impl TerminalSpawner for TerminalAppSpawner {
    fn terminal_type(&self) -> TerminalType {
        TerminalType::Default
    }

    fn spawn(
        &self,
        nvim_path: &str,
        file_path: &str,
        geometry: Option<WindowGeometry>,
    ) -> Result<SpawnInfo, String> {
        let script = if let Some(geo) = geometry {
            format!(
                r#"
            tell application "Terminal"
                activate
                do script "{} '+normal G$' '{}'"
                set bounds of front window to {{{}, {}, {}, {}}}
            end tell
            "#,
                nvim_path,
                file_path,
                geo.x,
                geo.y,
                geo.x + geo.width as i32,
                geo.y + geo.height as i32
            )
        } else {
            format!(
                r#"
            tell application "Terminal"
                activate
                do script "{} '+normal G$' '{}'"
            end tell
            "#,
                nvim_path, file_path
            )
        };

        Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|e| format!("Failed to run Terminal AppleScript: {}", e))?;

        // Try to find the nvim process ID by the file it's editing
        let pid = find_nvim_pid_for_file(file_path);
        log::info!("Found nvim PID: {:?} for file: {}", pid, file_path);

        Ok(SpawnInfo {
            terminal_type: TerminalType::Default,
            process_id: pid,
            child: None,
            window_title: None,
        })
    }
}
