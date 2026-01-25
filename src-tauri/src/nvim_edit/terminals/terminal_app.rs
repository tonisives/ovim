//! Terminal.app spawner (macOS default terminal)

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use super::process_utils::find_editor_pid_for_file;
use super::{SpawnInfo, TerminalSpawner, TerminalType, WindowGeometry};
use crate::config::NvimEditSettings;

/// Escape a string for use in shell (single-quote escaping)
fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

pub struct TerminalAppSpawner;

impl TerminalSpawner for TerminalAppSpawner {
    fn terminal_type(&self) -> TerminalType {
        TerminalType::Default
    }

    fn spawn(
        &self,
        settings: &NvimEditSettings,
        file_path: &str,
        geometry: Option<WindowGeometry>,
        socket_path: Option<&Path>,
        custom_env: Option<&HashMap<String, String>>,
        text_is_empty: bool,
    ) -> Result<SpawnInfo, String> {
        // Get editor path and args from settings (insert mode if text is empty)
        let editor_path = settings.editor_path();
        let editor_args = settings.editor_args(text_is_empty);
        let process_name = settings.editor_process_name();

        // Build socket args for nvim RPC if socket_path provided and using nvim
        let socket_args: Vec<String> = if let Some(socket) = socket_path {
            if editor_path.contains("nvim") || editor_path == "nvim" {
                vec!["--listen".to_string(), socket.to_string_lossy().to_string()]
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        // Build the command string for AppleScript (socket args + editor args)
        // Each argument must be shell-escaped to preserve integrity through AppleScript
        let mut all_args: Vec<String> = socket_args;
        all_args.extend(editor_args.iter().map(|s| s.to_string()));
        let args_str = if all_args.is_empty() {
            String::new()
        } else {
            let escaped_args: Vec<String> = all_args.iter().map(|s| shell_escape(s)).collect();
            format!(" {}", escaped_args.join(" "))
        };

        // Build environment export commands for custom env
        let env_exports = if let Some(env) = custom_env {
            env.iter()
                .map(|(k, v)| format!("export {}='{}'", k, v.replace('\'', "'\\''")))
                .collect::<Vec<_>>()
                .join("; ")
        } else {
            String::new()
        };
        let env_prefix = if env_exports.is_empty() {
            String::new()
        } else {
            format!("{}; ", env_exports)
        };

        let script = if let Some(geo) = geometry {
            format!(
                r#"
            tell application "Terminal"
                do script "{}{}{} '{}'"
                set bounds of front window to {{{}, {}, {}, {}}}
                activate
            end tell
            "#,
                env_prefix,
                editor_path,
                args_str,
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
                do script "{}{}{} '{}'"
                activate
            end tell
            "#,
                env_prefix, editor_path, args_str, file_path
            )
        };

        Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|e| format!("Failed to run Terminal AppleScript: {}", e))?;

        // Try to find the editor process ID by the file it's editing
        let pid = find_editor_pid_for_file(file_path, process_name);
        log::info!("Found editor PID: {:?} for file: {}", pid, file_path);

        Ok(SpawnInfo {
            terminal_type: TerminalType::Default,
            process_id: pid,
            child: None,
            window_title: None,
        })
    }
}
