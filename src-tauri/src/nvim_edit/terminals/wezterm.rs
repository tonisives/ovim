//! WezTerm terminal spawner

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use super::applescript_utils::set_window_size;
use super::process_utils::{resolve_command_path, resolve_terminal_path};
use super::{SpawnInfo, TerminalSpawner, TerminalType, WindowGeometry};
use crate::config::NvimEditSettings;

pub struct WezTermSpawner;

impl TerminalSpawner for WezTermSpawner {
    fn terminal_type(&self) -> TerminalType {
        TerminalType::WezTerm
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

        // Resolve editor path
        let resolved_editor = resolve_command_path(&editor_path);
        log::info!("Resolved editor path: {} -> {}", editor_path, resolved_editor);

        // Resolve terminal path (uses user setting or auto-detects)
        let terminal_cmd = settings.get_terminal_path();
        let resolved_terminal = resolve_terminal_path(&terminal_cmd);
        log::info!("Resolved terminal path: {} -> {}", terminal_cmd, resolved_terminal);

        let mut cmd = Command::new(&resolved_terminal);

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

        cmd.arg(&resolved_editor);
        for arg in &socket_args {
            cmd.arg(arg);
        }
        for arg in &editor_args {
            cmd.arg(arg);
        }
        cmd.arg(file_path);

        // Apply custom environment variables
        if let Some(env) = custom_env {
            cmd.envs(env.iter());
        }

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn wezterm: {}", e))?;

        // Get the wezterm process PID - with --always-new-process, the wezterm
        // process itself will block until editor exits, so we can track it directly
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
