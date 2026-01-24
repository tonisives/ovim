//! Alacritty terminal spawner

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use super::applescript_utils::{
    find_alacritty_window_by_title, focus_alacritty_window_by_index, get_alacritty_window_position,
    set_window_bounds_atomic,
};
use super::process_utils::{find_editor_pid_for_file, resolve_command_path, resolve_terminal_path};
use super::{SpawnInfo, TerminalSpawner, TerminalType, WindowGeometry};
use crate::config::NvimEditSettings;

pub struct AlacrittySpawner;

impl TerminalSpawner for AlacrittySpawner {
    fn terminal_type(&self) -> TerminalType {
        TerminalType::Alacritty
    }

    fn spawn(
        &self,
        settings: &NvimEditSettings,
        file_path: &str,
        geometry: Option<WindowGeometry>,
        socket_path: Option<&Path>,
        custom_env: Option<&HashMap<String, String>>,
    ) -> Result<SpawnInfo, String> {
        // Generate a unique window title so we can find it
        let unique_title = format!("ovim-edit-{}", std::process::id());

        // Get editor path and args from settings
        let editor_path = settings.editor_path();
        let editor_args = settings.editor_args();
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

        // Resolve editor path to absolute path (msg create-window doesn't inherit PATH)
        let resolved_editor = resolve_command_path(&editor_path);
        log::info!("Resolved editor path: {} -> {}", editor_path, resolved_editor);

        // Calculate initial window size
        let (init_columns, init_lines) = if let Some(ref geo) = geometry {
            ((geo.width / 8).max(40) as u32, (geo.height / 16).max(10) as u32)
        } else {
            (80, 24)
        };

        // Resolve terminal path (uses user setting or auto-detects)
        let terminal_cmd = settings.get_terminal_path();
        let resolved_terminal = resolve_terminal_path(&terminal_cmd);
        log::info!("Resolved terminal path: {} -> {}", terminal_cmd, resolved_terminal);

        // Build the editor command with all args
        let mut editor_cmd_parts: Vec<String> = vec![resolved_editor.clone()];
        editor_cmd_parts.extend(socket_args.iter().cloned());
        editor_cmd_parts.extend(editor_args.iter().map(|s| s.to_string()));
        editor_cmd_parts.push(file_path.to_string());

        // If we have custom env, wrap in a shell command that exports the env first
        // This allows using msg create-window (fast) while still setting env
        let shell_script = if let Some(env) = custom_env {
            if !env.is_empty() {
                let mut exports: Vec<String> = env.iter()
                    .map(|(k, v)| format!("export {}={}", k, shell_escape(v)))
                    .collect();
                exports.push(shell_escape_cmd(&editor_cmd_parts));
                let script = exports.join("; ");
                log::info!("Shell wrapper script: {}", script);
                Some(script)
            } else {
                None
            }
        } else {
            None
        };

        // Extract position from geometry for Alacritty's window.position option
        // Note: Alacritty uses Cocoa coordinates (y=0 at bottom), so we need to convert
        // We pass None for position here and let AppleScript handle positioning after spawn
        // because Alacritty's window.position has offset issues with menu bar
        let position: Option<(i32, i32)> = None;

        // Try msg create-window first (faster, reuses existing daemon)
        let msg_args = if let Some(ref script) = shell_script {
            // Use shell wrapper: bash -c 'export ...; cmd'
            self.build_msg_args_shell(&unique_title, init_columns, init_lines, "bash", "-c", script, position)
        } else {
            // Direct editor command
            self.build_msg_args(
                &unique_title,
                init_columns,
                init_lines,
                &resolved_editor,
                &socket_args,
                &editor_args,
                file_path,
                position,
            )
        };

        let msg_result = Command::new(&resolved_terminal)
            .args(&msg_args)
            .status();

        let child = match msg_result {
            Ok(status) if status.success() => {
                log::info!("msg create-window succeeded");
                None
            }
            _ => {
                log::info!("msg create-window failed, falling back to regular spawn");
                Some(self.spawn_new_process(
                    &resolved_terminal,
                    &unique_title,
                    init_columns,
                    init_lines,
                    &resolved_editor,
                    &socket_args,
                    &editor_args,
                    file_path,
                    custom_env,
                    position,
                )?)
            }
        };

        // Start a watcher thread to find the window, set bounds, and focus it
        // Alacritty's startup_mode config can override initial position, so we
        // keep setting bounds until the position matches (or timeout)
        {
            let title = unique_title.clone();
            let geo = geometry.clone();
            std::thread::spawn(move || {
                // Poll rapidly to catch the window as soon as it appears
                for _attempt in 0..200 {
                    if let Some(index) = find_alacritty_window_by_title(&title) {
                        log::info!("Found window '{}' at index {}", title, index);
                        if let Some(ref g) = geo {
                            // Keep setting bounds until position is correct (max 1 second)
                            for attempt in 0..20 {
                                // Check current position first
                                if let Some((actual_x, actual_y)) = get_alacritty_window_position(index) {
                                    let tolerance = 10;
                                    if (actual_x - g.x).abs() <= tolerance && (actual_y - g.y).abs() <= tolerance {
                                        log::info!("Position correct after {} attempts", attempt);
                                        break;
                                    }
                                }
                                // Position not correct, set it
                                set_window_bounds_atomic("Alacritty", index, g.x, g.y, g.width, g.height);
                                std::thread::sleep(std::time::Duration::from_millis(30));
                            }
                        }
                        // Focus the new window
                        focus_alacritty_window_by_index(index);
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                log::warn!("Timeout waiting for Alacritty window '{}'", title);
            });
        }

        // Wait a bit for editor to start, then find its PID by the file it's editing
        let pid = find_editor_pid_for_file(file_path, process_name);
        log::info!("Found editor PID: {:?} for file: {}", pid, file_path);

        Ok(SpawnInfo {
            terminal_type: TerminalType::Alacritty,
            process_id: pid,
            child,
            window_title: Some(unique_title),
        })
    }
}

/// Escape a string for use in shell
fn shell_escape(s: &str) -> String {
    // Use single quotes and escape any single quotes within
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Build a shell command from parts
fn shell_escape_cmd(parts: &[String]) -> String {
    parts.iter()
        .map(|s| shell_escape(s))
        .collect::<Vec<_>>()
        .join(" ")
}

impl AlacrittySpawner {
    fn build_msg_args_shell(
        &self,
        title: &str,
        columns: u32,
        lines: u32,
        shell: &str,
        shell_flag: &str,
        script: &str,
        position: Option<(i32, i32)>,
    ) -> Vec<String> {
        let mut args = vec![
            "msg".to_string(),
            "create-window".to_string(),
            "-o".to_string(),
            format!("window.title=\"{}\"", title),
            "-o".to_string(),
            "window.dynamic_title=false".to_string(),
            "-o".to_string(),
            "window.startup_mode=\"Windowed\"".to_string(),
            "-o".to_string(),
            format!("window.dimensions.columns={}", columns),
            "-o".to_string(),
            format!("window.dimensions.lines={}", lines),
        ];
        if let Some((x, y)) = position {
            args.push("-o".to_string());
            args.push(format!("window.position.x={}", x));
            args.push("-o".to_string());
            args.push(format!("window.position.y={}", y));
        }
        args.push("-e".to_string());
        args.push(shell.to_string());
        args.push(shell_flag.to_string());
        args.push(script.to_string());
        args
    }

    fn build_msg_args(
        &self,
        title: &str,
        columns: u32,
        lines: u32,
        editor: &str,
        socket_args: &[String],
        editor_args: &[&str],
        file_path: &str,
        position: Option<(i32, i32)>,
    ) -> Vec<String> {
        let mut args = vec![
            "msg".to_string(),
            "create-window".to_string(),
            "-o".to_string(),
            format!("window.title=\"{}\"", title),
            "-o".to_string(),
            "window.dynamic_title=false".to_string(),
            "-o".to_string(),
            "window.startup_mode=\"Windowed\"".to_string(),
            "-o".to_string(),
            format!("window.dimensions.columns={}", columns),
            "-o".to_string(),
            format!("window.dimensions.lines={}", lines),
        ];
        if let Some((x, y)) = position {
            args.push("-o".to_string());
            args.push(format!("window.position.x={}", x));
            args.push("-o".to_string());
            args.push(format!("window.position.y={}", y));
        }
        args.push("-e".to_string());
        args.push(editor.to_string());
        args.extend(socket_args.iter().cloned());
        args.extend(editor_args.iter().map(|s| s.to_string()));
        args.push(file_path.to_string());
        args
    }

    fn spawn_new_process(
        &self,
        terminal: &str,
        title: &str,
        columns: u32,
        lines: u32,
        editor: &str,
        socket_args: &[String],
        editor_args: &[&str],
        file_path: &str,
        custom_env: Option<&HashMap<String, String>>,
        position: Option<(i32, i32)>,
    ) -> Result<std::process::Child, String> {
        let mut args: Vec<String> = vec![
            "-o".to_string(),
            format!("window.title=\"{}\"", title),
            "-o".to_string(),
            "window.dynamic_title=false".to_string(),
            "-o".to_string(),
            "window.startup_mode=\"Windowed\"".to_string(),
            "-o".to_string(),
            format!("window.dimensions.columns={}", columns),
            "-o".to_string(),
            format!("window.dimensions.lines={}", lines),
        ];
        if let Some((x, y)) = position {
            args.push("-o".to_string());
            args.push(format!("window.position.x={}", x));
            args.push("-o".to_string());
            args.push(format!("window.position.y={}", y));
        }
        args.push("-e".to_string());
        args.push(editor.to_string());
        args.extend(socket_args.iter().cloned());
        args.extend(editor_args.iter().map(|s| s.to_string()));
        args.push(file_path.to_string());

        let mut cmd = Command::new(terminal);
        cmd.args(&args);

        if let Some(env) = custom_env {
            log::info!("Applying {} custom env vars to alacritty", env.len());
            cmd.envs(env.iter());
        }

        cmd.spawn()
            .map_err(|e| format!("Failed to spawn alacritty: {}", e))
    }
}
