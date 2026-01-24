//! Alacritty terminal spawner

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use super::applescript_utils::{
    find_window_by_title, focus_window_by_index, get_window_position, set_window_bounds,
};
use super::process_utils::{find_editor_pid_for_file, resolve_command_path, resolve_terminal_path};
use super::{SpawnInfo, TerminalSpawner, TerminalType, WindowGeometry};
use crate::config::NvimEditSettings;

/// Process names to search for Alacritty windows (spawned processes are lowercase)
const ALACRITTY_PROCESS_NAMES: &[&str] = &["alacritty", "Alacritty"];

pub struct AlacrittySpawner;

/// Configuration for spawning an Alacritty window
struct SpawnConfig {
    title: String,
    columns: u32,
    lines: u32,
    editor_cmd: Vec<String>,
    terminal_path: String,
}

impl SpawnConfig {
    fn new(settings: &NvimEditSettings, file_path: &str, socket_path: Option<&Path>) -> Self {
        let editor_path = settings.editor_path();
        let resolved_editor = resolve_command_path(&editor_path);
        log::info!("Resolved editor path: {} -> {}", editor_path, resolved_editor);

        let terminal_cmd = settings.get_terminal_path();
        let terminal_path = resolve_terminal_path(&terminal_cmd);
        log::info!("Resolved terminal path: {} -> {}", terminal_cmd, terminal_path);

        // Build editor command with all args
        let mut editor_cmd = vec![resolved_editor.clone()];

        // Add socket args for nvim RPC if applicable
        if let Some(socket) = socket_path {
            if editor_path.contains("nvim") || editor_path == "nvim" {
                editor_cmd.push("--listen".to_string());
                editor_cmd.push(socket.to_string_lossy().to_string());
            }
        }

        // Add editor args from settings
        for arg in settings.editor_args() {
            editor_cmd.push(arg.to_string());
        }

        // Add file path
        editor_cmd.push(file_path.to_string());

        Self {
            title: format!("ovim-edit-{}", std::process::id()),
            columns: 80,
            lines: 24,
            editor_cmd,
            terminal_path,
        }
    }

    fn with_geometry(mut self, geometry: Option<&WindowGeometry>) -> Self {
        if let Some(geo) = geometry {
            self.columns = (geo.width / 8).max(40);
            self.lines = (geo.height / 16).max(10);
        }
        self
    }
}

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
        let config = SpawnConfig::new(settings, file_path, socket_path)
            .with_geometry(geometry.as_ref());

        // Try msg create-window first (faster, reuses existing daemon)
        let spawn_result = self.try_msg_spawn(&config, custom_env)
            .or_else(|| self.fallback_spawn(&config, custom_env));

        let child = match spawn_result {
            Some(Ok(child)) => Some(child),
            Some(Err(e)) => return Err(e),
            None => None,
        };

        // Start window positioning thread
        self.spawn_position_watcher(&config.title, geometry);

        // Find editor PID
        let pid = find_editor_pid_for_file(file_path, settings.editor_process_name());
        log::info!("Found editor PID: {:?} for file: {}", pid, file_path);

        Ok(SpawnInfo {
            terminal_type: TerminalType::Alacritty,
            process_id: pid,
            child,
            window_title: Some(config.title),
        })
    }
}

impl AlacrittySpawner {
    /// Try to spawn using `alacritty msg create-window` (faster)
    fn try_msg_spawn(
        &self,
        config: &SpawnConfig,
        custom_env: Option<&HashMap<String, String>>,
    ) -> Option<Result<std::process::Child, String>> {
        let args = self.build_msg_args(config, custom_env);

        match Command::new(&config.terminal_path).args(&args).status() {
            Ok(status) if status.success() => {
                log::info!("msg create-window succeeded");
                None // No child process to return
            }
            _ => {
                log::info!("msg create-window failed, falling back to regular spawn");
                Some(self.spawn_new_process(config, custom_env))
            }
        }
    }

    /// Fallback: spawn a new Alacritty process
    fn fallback_spawn(
        &self,
        config: &SpawnConfig,
        custom_env: Option<&HashMap<String, String>>,
    ) -> Option<Result<std::process::Child, String>> {
        Some(self.spawn_new_process(config, custom_env))
    }

    /// Build args for msg create-window command
    fn build_msg_args(
        &self,
        config: &SpawnConfig,
        custom_env: Option<&HashMap<String, String>>,
    ) -> Vec<String> {
        let mut args = vec![
            "msg".to_string(),
            "create-window".to_string(),
        ];

        args.extend(self.window_options(&config.title, config.columns, config.lines));

        args.push("-e".to_string());

        // If we have custom env, wrap in a shell command
        if let Some(env) = custom_env {
            if !env.is_empty() {
                let shell_cmd = self.wrap_with_env(&config.editor_cmd, env);
                args.extend(["bash".to_string(), "-c".to_string(), shell_cmd]);
                return args;
            }
        }

        args.extend(config.editor_cmd.clone());
        args
    }

    /// Spawn a new Alacritty process directly
    fn spawn_new_process(
        &self,
        config: &SpawnConfig,
        custom_env: Option<&HashMap<String, String>>,
    ) -> Result<std::process::Child, String> {
        let mut args = self.window_options(&config.title, config.columns, config.lines);
        args.push("-e".to_string());
        args.extend(config.editor_cmd.clone());

        let mut cmd = Command::new(&config.terminal_path);
        cmd.args(&args);

        if let Some(env) = custom_env {
            log::info!("Applying {} custom env vars to alacritty", env.len());
            cmd.envs(env.iter());
        }

        cmd.spawn()
            .map_err(|e| format!("Failed to spawn alacritty: {}", e))
    }

    /// Common window options for Alacritty
    fn window_options(&self, title: &str, columns: u32, lines: u32) -> Vec<String> {
        vec![
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
        ]
    }

    /// Wrap editor command with environment variable exports
    fn wrap_with_env(&self, editor_cmd: &[String], env: &HashMap<String, String>) -> String {
        let mut exports: Vec<String> = env
            .iter()
            .map(|(k, v)| format!("export {}={}", k, shell_escape(v)))
            .collect();
        exports.push(shell_escape_cmd(editor_cmd));
        let script = exports.join("; ");
        log::info!("Shell wrapper script: {}", script);
        script
    }

    /// Spawn a thread to find, position, and focus the new window
    fn spawn_position_watcher(&self, title: &str, geometry: Option<WindowGeometry>) {
        let title = title.to_string();
        let geo = geometry;

        std::thread::spawn(move || {
            // Poll rapidly to catch the window as soon as it appears
            for _attempt in 0..200 {
                if let Some(index) = find_window_by_title(ALACRITTY_PROCESS_NAMES, &title) {
                    log::info!("Found window '{}' at index {}", title, index);

                    if let Some(ref g) = geo {
                        position_window_with_retry(index, g);
                    }

                    focus_window_by_index(ALACRITTY_PROCESS_NAMES, index);
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            log::warn!("Timeout waiting for Alacritty window '{}'", title);
        });
    }
}

/// Position a window, retrying until position is correct (handles startup_mode override)
fn position_window_with_retry(index: usize, geo: &WindowGeometry) {
    const MAX_ATTEMPTS: u32 = 20;
    const TOLERANCE: i32 = 10;

    for attempt in 0..MAX_ATTEMPTS {
        // Check current position
        if let Some((actual_x, actual_y)) = get_window_position(ALACRITTY_PROCESS_NAMES, index) {
            if (actual_x - geo.x).abs() <= TOLERANCE && (actual_y - geo.y).abs() <= TOLERANCE {
                log::info!("Position correct after {} attempts", attempt);
                return;
            }
        }

        // Position not correct, set it
        set_window_bounds(ALACRITTY_PROCESS_NAMES, index, geo.x, geo.y, geo.width, geo.height);
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
}

/// Escape a string for use in shell
fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Build a shell command from parts
fn shell_escape_cmd(parts: &[String]) -> String {
    parts
        .iter()
        .map(|s| shell_escape(s))
        .collect::<Vec<_>>()
        .join(" ")
}
