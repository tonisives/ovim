//! Alacritty terminal spawner

use std::process::Command;

use super::applescript_utils::{
    find_alacritty_window_by_title, focus_alacritty_window_by_index, set_window_bounds_atomic,
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
    ) -> Result<SpawnInfo, String> {
        // Generate a unique window title so we can find it
        let unique_title = format!("ovim-edit-{}", std::process::id());

        // Get editor path and args from settings
        let editor_path = settings.editor_path();
        let editor_args = settings.editor_args();
        let process_name = settings.editor_process_name();

        // Resolve editor path to absolute path (msg create-window doesn't inherit PATH)
        let resolved_editor = resolve_command_path(&editor_path);
        log::info!("Resolved editor path: {} -> {}", editor_path, resolved_editor);

        // Start a watcher thread to find the window, set bounds, and focus it
        {
            let title = unique_title.clone();
            let geo = geometry.clone();
            std::thread::spawn(move || {
                // Poll rapidly to catch the window as soon as it appears
                for _attempt in 0..200 {
                    if let Some(index) = find_alacritty_window_by_title(&title) {
                        log::info!("Found window '{}' at index {}", title, index);
                        if let Some(ref g) = geo {
                            set_window_bounds_atomic("Alacritty", index, g.x, g.y, g.width, g.height);
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

        // Calculate initial window size
        let (init_columns, init_lines) = if let Some(ref geo) = geometry {
            ((geo.width / 8).max(40) as u32, (geo.height / 16).max(10) as u32)
        } else {
            (80, 24)
        };

        // Build the command arguments: editor path, editor args, file path
        let mut cmd_args: Vec<String> = vec![
            "msg".to_string(),
            "create-window".to_string(),
            "-o".to_string(),
            format!("window.title=\"{}\"", unique_title),
            "-o".to_string(),
            "window.dynamic_title=false".to_string(),
            "-o".to_string(),
            "window.startup_mode=\"Windowed\"".to_string(),
            "-o".to_string(),
            format!("window.dimensions.columns={}", init_columns),
            "-o".to_string(),
            format!("window.dimensions.lines={}", init_lines),
            "-e".to_string(),
            resolved_editor.clone(),
        ];
        for arg in &editor_args {
            cmd_args.push(arg.to_string());
        }
        cmd_args.push(file_path.to_string());

        // Resolve terminal path (uses user setting or auto-detects)
        let terminal_cmd = settings.get_terminal_path();
        let resolved_terminal = resolve_terminal_path(&terminal_cmd);
        log::info!("Resolved terminal path: {} -> {}", terminal_cmd, resolved_terminal);

        // Use `alacritty msg create-window` to create window in existing daemon
        let result = Command::new(&resolved_terminal)
            .args(&cmd_args)
            .spawn();

        // If msg create-window fails (no daemon running), fall back to regular spawn
        let cmd = match result {
            Ok(child) => child,
            Err(_) => {
                log::info!("msg create-window failed, falling back to regular spawn");
                // Build fallback args (without msg create-window)
                let mut fallback_args: Vec<String> = vec![
                    "-o".to_string(),
                    format!("window.title=\"{}\"", unique_title),
                    "-o".to_string(),
                    "window.dynamic_title=false".to_string(),
                    "-o".to_string(),
                    "window.startup_mode=\"Windowed\"".to_string(),
                    "-o".to_string(),
                    format!("window.dimensions.columns={}", init_columns),
                    "-o".to_string(),
                    format!("window.dimensions.lines={}", init_lines),
                    "-e".to_string(),
                    resolved_editor.clone(),
                ];
                for arg in &editor_args {
                    fallback_args.push(arg.to_string());
                }
                fallback_args.push(file_path.to_string());

                Command::new(&resolved_terminal)
                    .args(&fallback_args)
                    .spawn()
                    .map_err(|e| format!("Failed to spawn alacritty: {}", e))?
            }
        };

        // Wait a bit for editor to start, then find its PID by the file it's editing
        let pid = find_editor_pid_for_file(file_path, process_name);
        log::info!("Found editor PID: {:?} for file: {}", pid, file_path);

        Ok(SpawnInfo {
            terminal_type: TerminalType::Alacritty,
            process_id: pid,
            child: Some(cmd),
            window_title: Some(unique_title),
        })
    }
}
