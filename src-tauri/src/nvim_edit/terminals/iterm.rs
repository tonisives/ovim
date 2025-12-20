//! iTerm2 terminal spawner

use std::path::Path;
use std::process::Command;

use super::process_utils::find_editor_pid_for_file;
use super::{SpawnInfo, TerminalSpawner, TerminalType, WindowGeometry};
use crate::config::NvimEditSettings;

pub struct ITermSpawner;

impl TerminalSpawner for ITermSpawner {
    fn terminal_type(&self) -> TerminalType {
        TerminalType::ITerm
    }

    fn spawn(
        &self,
        settings: &NvimEditSettings,
        file_path: &str,
        geometry: Option<WindowGeometry>,
        socket_path: Option<&Path>,
    ) -> Result<SpawnInfo, String> {
        // Get editor path and args from settings
        let editor_path = settings.editor_path();
        let mut editor_args = settings.editor_args();
        let process_name = settings.editor_process_name();

        // Add --listen for nvim RPC if socket_path provided and using nvim
        if let Some(socket) = socket_path {
            if editor_path.contains("nvim") || editor_path == "nvim" {
                editor_args.insert(0, socket.to_string_lossy().to_string());
                editor_args.insert(0, "--listen".to_string());
            }
        }

        // Build the command string for AppleScript
        let args_str = if editor_args.is_empty() {
            String::new()
        } else {
            format!(" {}", editor_args.join(" "))
        };

        // Use AppleScript to open iTerm and run editor with position/size
        let script = if let Some(geo) = geometry {
            format!(
                r#"
            tell application "iTerm"
                activate
                set newWindow to (create window with default profile)
                set bounds of newWindow to {{{}, {}, {}, {}}}
                tell current session of newWindow
                    write text "{}{} '{}'; exit"
                end tell
            end tell
            "#,
                geo.x,
                geo.y,
                geo.x + geo.width as i32,
                geo.y + geo.height as i32,
                editor_path,
                args_str,
                file_path
            )
        } else {
            format!(
                r#"
            tell application "iTerm"
                activate
                set newWindow to (create window with default profile)
                tell current session of newWindow
                    write text "{}{} '{}'; exit"
                end tell
            end tell
            "#,
                editor_path, args_str, file_path
            )
        };

        Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|e| format!("Failed to run iTerm AppleScript: {}", e))?;

        // Try to find the editor process ID by the file it's editing
        let pid = find_editor_pid_for_file(file_path, process_name);
        log::info!("Found editor PID: {:?} for file: {}", pid, file_path);

        Ok(SpawnInfo {
            terminal_type: TerminalType::ITerm,
            process_id: pid,
            child: None,
            window_title: None,
        })
    }
}
