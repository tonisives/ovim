//! iTerm2 terminal spawner

use std::process::Command;

use super::process_utils::find_nvim_pid_for_file;
use super::{SpawnInfo, TerminalSpawner, TerminalType, WindowGeometry};

pub struct ITermSpawner;

impl TerminalSpawner for ITermSpawner {
    fn terminal_type(&self) -> TerminalType {
        TerminalType::ITerm
    }

    fn spawn(
        &self,
        nvim_path: &str,
        file_path: &str,
        geometry: Option<WindowGeometry>,
    ) -> Result<SpawnInfo, String> {
        // Use AppleScript to open iTerm and run nvim with position/size
        let script = if let Some(geo) = geometry {
            format!(
                r#"
            tell application "iTerm"
                activate
                set newWindow to (create window with default profile)
                set bounds of newWindow to {{{}, {}, {}, {}}}
                tell current session of newWindow
                    write text "{} '+normal G$' '{}'; exit"
                end tell
            end tell
            "#,
                geo.x,
                geo.y,
                geo.x + geo.width as i32,
                geo.y + geo.height as i32,
                nvim_path,
                file_path
            )
        } else {
            format!(
                r#"
            tell application "iTerm"
                activate
                set newWindow to (create window with default profile)
                tell current session of newWindow
                    write text "{} '+normal G$' '{}'; exit"
                end tell
            end tell
            "#,
                nvim_path, file_path
            )
        };

        Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|e| format!("Failed to run iTerm AppleScript: {}", e))?;

        // Try to find the nvim process ID by the file it's editing
        let pid = find_nvim_pid_for_file(file_path);
        log::info!("Found nvim PID: {:?} for file: {}", pid, file_path);

        Ok(SpawnInfo {
            terminal_type: TerminalType::ITerm,
            process_id: pid,
            child: None,
            window_title: None,
        })
    }
}
