//! Alacritty terminal spawner

use std::process::Command;

use super::applescript_utils::{
    find_alacritty_window_by_title, focus_alacritty_window_by_index, set_window_bounds_atomic,
};
use super::process_utils::{find_nvim_pid_for_file, resolve_command_path};
use super::{SpawnInfo, TerminalSpawner, TerminalType, WindowGeometry};

pub struct AlacrittySpawner;

impl TerminalSpawner for AlacrittySpawner {
    fn terminal_type(&self) -> TerminalType {
        TerminalType::Alacritty
    }

    fn spawn(
        &self,
        nvim_path: &str,
        file_path: &str,
        geometry: Option<WindowGeometry>,
    ) -> Result<SpawnInfo, String> {
        // Generate a unique window title so we can find it
        let unique_title = format!("ovim-edit-{}", std::process::id());

        // Resolve nvim path to absolute path (msg create-window doesn't inherit PATH)
        let resolved_nvim = resolve_command_path(nvim_path);
        log::info!("Resolved nvim path: {} -> {}", nvim_path, resolved_nvim);

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

        // Use `alacritty msg create-window` to create window in existing daemon
        let result = Command::new("alacritty")
            .args([
                "msg",
                "create-window",
                "-o",
                &format!("window.title=\"{}\"", unique_title),
                "-o",
                "window.dynamic_title=false",
                "-o",
                "window.startup_mode=\"Windowed\"",
                "-o",
                &format!("window.dimensions.columns={}", init_columns),
                "-o",
                &format!("window.dimensions.lines={}", init_lines),
                "-e",
                &resolved_nvim,
                "+normal G$",
                file_path,
            ])
            .spawn();

        // If msg create-window fails (no daemon running), fall back to regular spawn
        let cmd = match result {
            Ok(child) => child,
            Err(_) => {
                log::info!("msg create-window failed, falling back to regular spawn");
                Command::new("alacritty")
                    .args([
                        "-o",
                        &format!("window.title=\"{}\"", unique_title),
                        "-o",
                        "window.dynamic_title=false",
                        "-o",
                        "window.startup_mode=\"Windowed\"",
                        "-o",
                        &format!("window.dimensions.columns={}", init_columns),
                        "-o",
                        &format!("window.dimensions.lines={}", init_lines),
                        "-e",
                        &resolved_nvim,
                        "+normal G$",
                        file_path,
                    ])
                    .spawn()
                    .map_err(|e| format!("Failed to spawn alacritty: {}", e))?
            }
        };

        // Wait a bit for nvim to start, then find its PID by the file it's editing
        let pid = find_nvim_pid_for_file(file_path);
        log::info!("Found nvim PID: {:?} for file: {}", pid, file_path);

        Ok(SpawnInfo {
            terminal_type: TerminalType::Alacritty,
            process_id: pid,
            child: Some(cmd),
            window_title: Some(unique_title),
        })
    }
}
