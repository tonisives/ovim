//! Alacritty terminal spawner

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use super::process_utils::{find_editor_pid_for_file, resolve_command_path, resolve_terminal_path};
use super::{SpawnInfo, TerminalSpawner, TerminalType, WindowGeometry};
use crate::config::NvimEditSettings;


pub struct AlacrittySpawner;

/// Configuration for spawning an Alacritty window
struct SpawnConfig {
    title: String,
    columns: u32,
    lines: u32,
    x: Option<i32>,
    y: Option<i32>,
    width: Option<u32>,
    height: Option<u32>,
    editor_cmd: Vec<String>,
    terminal_path: String,
}

impl SpawnConfig {
    fn new(settings: &NvimEditSettings, file_path: &str, socket_path: Option<&Path>, text_is_empty: bool, filetype: Option<&str>) -> Self {
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

        // Add filetype command if provided (for nvim/vim)
        if let Some(ft) = filetype {
            if editor_path.contains("nvim") || editor_path.contains("vim") {
                editor_cmd.push("-c".to_string());
                editor_cmd.push(format!("set ft={}", ft));
            }
        }

        // Add editor args from settings (insert mode if text is empty)
        for arg in settings.editor_args(text_is_empty) {
            editor_cmd.push(arg.to_string());
        }

        // Add file path
        editor_cmd.push(file_path.to_string());

        Self {
            title: format!("ovim-edit-{}", std::process::id()),
            columns: 80,
            lines: 24,
            x: None,
            y: None,
            width: None,
            height: None,
            editor_cmd,
            terminal_path,
        }
    }

    fn with_geometry(mut self, geometry: Option<&WindowGeometry>) -> Self {
        if let Some(geo) = geometry {
            self.columns = (geo.width / 8).max(40);
            self.lines = (geo.height / 16).max(10);
            self.x = Some(geo.x);
            self.y = Some(geo.y);
            self.width = Some(geo.width);
            self.height = Some(geo.height);
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
        text_is_empty: bool,
        filetype: Option<&str>,
    ) -> Result<SpawnInfo, String> {
        let config = SpawnConfig::new(settings, file_path, socket_path, text_is_empty, filetype)
            .with_geometry(geometry.as_ref());

        // Try msg create-window first (faster, reuses existing daemon)
        // Note: skipped when geometry is specified since msg doesn't support position
        let spawn_result = self.try_msg_spawn(&config, custom_env)
            .or_else(|| self.fallback_spawn(&config, custom_env));

        let child = match spawn_result {
            Some(Ok(child)) => Some(child),
            Some(Err(e)) => return Err(e),
            None => None,
        };

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
    /// Note: msg create-window doesn't support window.position, so we skip it when positioning is needed
    fn try_msg_spawn(
        &self,
        config: &SpawnConfig,
        custom_env: Option<&HashMap<String, String>>,
    ) -> Option<Result<std::process::Child, String>> {
        // Skip msg create-window when position is specified - it doesn't support window.position
        if config.x.is_some() && config.y.is_some() {
            log::info!("Skipping msg create-window because position is specified");
            return None;
        }

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

        args.extend(self.window_options(config));

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
        let mut args = self.window_options(config);
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
    fn window_options(&self, config: &SpawnConfig) -> Vec<String> {
        let mut args = vec![
            "-o".to_string(),
            format!("window.title=\"{}\"", config.title),
            "-o".to_string(),
            "window.dynamic_title=false".to_string(),
            "-o".to_string(),
            "window.startup_mode=\"Windowed\"".to_string(),
            "-o".to_string(),
            format!("window.dimensions.columns={}", config.columns),
            "-o".to_string(),
            format!("window.dimensions.lines={}", config.lines),
        ];

        // Add position if available - this positions the window at spawn time
        // avoiding the slow AppleScript animation.
        // Must use object syntax to set both x and y together, otherwise only one axis applies.
        // See: https://github.com/alacritty/alacritty/issues/7518
        // Note: Alacritty uses physical pixels, but our coordinates are in points (macOS).
        // On Retina displays, we need to multiply by the scale factor (typically 2).
        if let (Some(x), Some(y)) = (config.x, config.y) {
            // TODO: Get actual scale factor instead of assuming 2x
            let scale = 2;
            let pos_opt = format!("window.position={{x={},y={}}}", x * scale, y * scale);
            log::info!("Alacritty position option: {}", pos_opt);
            args.push("-o".to_string());
            args.push(pos_opt);
        }

        args
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

}

/// Result of spawning a pre-warmed Alacritty window
pub struct PrewarmSpawnResult {
    pub process_id: Option<u32>,
    pub child: Option<std::process::Child>,
    pub window_title: String,
}

impl AlacrittySpawner {
    /// Spawn a hidden, off-screen Alacritty window with nvim listening on a socket.
    /// No file is loaded yet - nvim starts with an empty buffer.
    pub fn spawn_prewarm_window(
        settings: &NvimEditSettings,
        socket_path: &std::path::Path,
    ) -> Result<PrewarmSpawnResult, String> {
        let editor_path = settings.editor_path();
        let resolved_editor = resolve_command_path(&editor_path);
        let terminal_cmd = settings.get_terminal_path();
        let terminal_path = resolve_terminal_path(&terminal_cmd);

        let title = format!("ovim-prewarm-{}", std::process::id());

        // Build editor command: nvim --listen <socket> (no file)
        let editor_cmd = vec![
            resolved_editor,
            "--listen".to_string(),
            socket_path.to_string_lossy().to_string(),
        ];

        // Position off-screen so the window is invisible
        let scale = 2; // Retina
        let offscreen_x = -10000 * scale;
        let offscreen_y = -10000 * scale;

        let mut args = vec![
            "-o".to_string(),
            format!("window.title=\"{}\"", title),
            "-o".to_string(),
            "window.dynamic_title=false".to_string(),
            "-o".to_string(),
            "window.startup_mode=\"Windowed\"".to_string(),
            "-o".to_string(),
            "window.dimensions.columns=80".to_string(),
            "-o".to_string(),
            "window.dimensions.lines=24".to_string(),
            "-o".to_string(),
            format!("window.position={{x={},y={}}}", offscreen_x, offscreen_y),
            "-e".to_string(),
        ];
        args.extend(editor_cmd);

        log::info!("Spawning prewarm Alacritty: {} {:?}", terminal_path, args);

        let child = Command::new(&terminal_path)
            .args(&args)
            .spawn()
            .map_err(|e| format!("Failed to spawn prewarm alacritty: {}", e))?;

        let child_pid = child.id();

        // Wait for nvim to start and create the socket
        let mut waited = 0;
        while waited < 5000 {
            if socket_path.exists() {
                log::info!("Prewarm nvim socket ready after {}ms", waited);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
            waited += 50;
        }

        if !socket_path.exists() {
            log::warn!("Prewarm nvim socket not found after 5s, continuing anyway");
        }

        // Find the nvim PID inside the terminal
        let nvim_pid = find_nvim_pid_for_socket(socket_path);

        Ok(PrewarmSpawnResult {
            process_id: nvim_pid.or(Some(child_pid)),
            child: Some(child),
            window_title: title,
        })
    }
}

/// Find the nvim process listening on a given socket
fn find_nvim_pid_for_socket(socket_path: &std::path::Path) -> Option<u32> {
    let socket_str = socket_path.to_string_lossy();
    let output = Command::new("pgrep")
        .args(["-f", &format!("nvim.*--listen.*{}", socket_str)])
        .output()
        .ok()?;
    if output.status.success() {
        let pid_str = String::from_utf8_lossy(&output.stdout);
        // Take first matching PID
        pid_str.lines().next()?.trim().parse().ok()
    } else {
        None
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
