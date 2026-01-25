//! Kitty terminal spawner

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use super::process_utils::{find_editor_pid_for_file, resolve_command_path, resolve_terminal_path};
use super::{SpawnInfo, TerminalSpawner, TerminalType, WindowGeometry};
use crate::config::NvimEditSettings;

pub struct KittySpawner;

impl TerminalSpawner for KittySpawner {
    fn terminal_type(&self) -> TerminalType {
        TerminalType::Kitty
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
        // Generate a unique window title
        let unique_title = format!("ovim-edit-{}", std::process::id());

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

        // Resolve editor path
        let resolved_editor = resolve_command_path(&editor_path);
        log::info!("Resolved editor path: {} -> {}", editor_path, resolved_editor);

        // Resolve terminal path (uses user setting or auto-detects)
        let terminal_cmd = settings.get_terminal_path();
        let resolved_terminal = resolve_terminal_path(&terminal_cmd);
        log::info!("Resolved terminal path: {} -> {}", terminal_cmd, resolved_terminal);

        let mut cmd = Command::new(&resolved_terminal);

        // Use single instance to avoid multiple dock icons, close window when editor exits
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
            .map_err(|e| format!("Failed to spawn kitty: {}", e))?;

        // Wait a bit for editor to start, then find its PID by the file it's editing
        let pid = find_editor_pid_for_file(file_path, process_name);
        log::info!("Found editor PID: {:?} for file: {}", pid, file_path);

        Ok(SpawnInfo {
            terminal_type: TerminalType::Kitty,
            process_id: pid,
            child: Some(child),
            window_title: Some(unique_title),
        })
    }
}
