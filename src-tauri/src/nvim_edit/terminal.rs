//! Terminal spawning for "Edit with Neovim" feature

use std::path::Path;
use std::process::{Child, Command};

/// Window position and size for popup mode
#[derive(Debug, Clone, Default)]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Terminal types supported
#[derive(Debug, Clone, PartialEq)]
pub enum TerminalType {
    Alacritty,
    Kitty,
    WezTerm,
    ITerm,
    Default, // Terminal.app
}

impl TerminalType {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "alacritty" => TerminalType::Alacritty,
            "kitty" => TerminalType::Kitty,
            "wezterm" => TerminalType::WezTerm,
            "iterm" | "iterm2" => TerminalType::ITerm,
            _ => TerminalType::Default,
        }
    }
}

/// Spawn info returned after launching terminal
pub struct SpawnInfo {
    pub terminal_type: TerminalType,
    pub process_id: Option<u32>,
    #[allow(dead_code)]
    pub child: Option<Child>,
    pub window_title: Option<String>,
}

/// Spawn a terminal with nvim editing the given file
pub fn spawn_terminal(
    terminal: &str,
    nvim_path: &str,
    temp_file: &Path,
    geometry: Option<WindowGeometry>,
) -> Result<SpawnInfo, String> {
    let terminal_type = TerminalType::from_string(terminal);
    let file_path = temp_file.to_string_lossy();

    match terminal_type {
        TerminalType::Alacritty => spawn_alacritty(nvim_path, &file_path, geometry),
        TerminalType::Kitty => spawn_kitty(nvim_path, &file_path, geometry),
        TerminalType::WezTerm => spawn_wezterm(nvim_path, &file_path, geometry),
        TerminalType::ITerm => spawn_iterm(nvim_path, &file_path, geometry),
        TerminalType::Default => spawn_terminal_app(nvim_path, &file_path, geometry),
    }
}

/// Wait for the terminal/nvim process to exit
pub fn wait_for_process(terminal_type: &TerminalType, process_id: Option<u32>) -> Result<(), String> {
    match terminal_type {
        TerminalType::Alacritty | TerminalType::Kitty | TerminalType::WezTerm => {
            // For direct terminals, we use sysctl to check if process is still running
            if let Some(pid) = process_id {
                wait_for_pid(pid)
            } else {
                Err("No process ID to wait for".to_string())
            }
        }
        TerminalType::ITerm | TerminalType::Default => {
            // For AppleScript-launched terminals, we poll for nvim process
            if let Some(pid) = process_id {
                wait_for_pid(pid)
            } else {
                // Fallback: wait a fixed time (not ideal)
                std::thread::sleep(std::time::Duration::from_secs(60));
                Ok(())
            }
        }
    }
}

/// Wait for a specific PID to exit
fn wait_for_pid(pid: u32) -> Result<(), String> {
    use std::thread;
    use std::time::Duration;

    loop {
        // Check if process is still running using kill(pid, 0)
        let result = unsafe { libc::kill(pid as i32, 0) };
        if result != 0 {
            // Process has exited
            break;
        }
        // Poll very fast (10ms) so we can restore focus before the window closes
        thread::sleep(Duration::from_millis(10));
    }

    Ok(())
}

/// Spawn Alacritty terminal
fn spawn_alacritty(nvim_path: &str, file_path: &str, geometry: Option<WindowGeometry>) -> Result<SpawnInfo, String> {
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

    // Calculate initial window size - use geometry if provided, otherwise quarter screen centered
    let (init_columns, init_lines) = if let Some(ref geo) = geometry {
        // Convert pixels to columns/lines (approximate: 8px per col, 16px per line)
        ((geo.width / 8).max(40) as u32, (geo.height / 16).max(10) as u32)
    } else {
        (80, 24) // Default terminal size
    };

    // Use `alacritty msg create-window` to create window in existing daemon
    // Override startup_mode to Windowed and set initial dimensions
    let result = Command::new("alacritty")
        .args([
            "msg", "create-window",
            "-o", &format!("window.title=\"{}\"", unique_title),
            "-o", "window.dynamic_title=false",
            "-o", "window.startup_mode=\"Windowed\"",
            "-o", &format!("window.dimensions.columns={}", init_columns),
            "-o", &format!("window.dimensions.lines={}", init_lines),
            "-e", &resolved_nvim, "+normal G$", file_path,
        ])
        .spawn();

    // If msg create-window fails (no daemon running), fall back to regular spawn
    let cmd = match result {
        Ok(child) => child,
        Err(_) => {
            log::info!("msg create-window failed, falling back to regular spawn");
            Command::new("alacritty")
                .args([
                    "-o", &format!("window.title=\"{}\"", unique_title),
                    "-o", "window.dynamic_title=false",
                    "-o", "window.startup_mode=\"Windowed\"",
                    "-o", &format!("window.dimensions.columns={}", init_columns),
                    "-o", &format!("window.dimensions.lines={}", init_lines),
                    "-e", &resolved_nvim, "+normal G$", file_path,
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

/// Spawn Kitty terminal
fn spawn_kitty(nvim_path: &str, file_path: &str, geometry: Option<WindowGeometry>) -> Result<SpawnInfo, String> {
    // Use "+normal G$" to move cursor to end of file
    let mut cmd = Command::new("kitty");

    // Add window position/size if provided
    // Kitty uses -o for config overrides
    if let Some(ref geo) = geometry {
        cmd.args([
            "-o", &format!("initial_window_width={}", geo.width),
            "-o", &format!("initial_window_height={}", geo.height),
            "-o", "remember_window_size=no",
        ]);
        // Note: Kitty doesn't have direct position args, but we can try osascript after
    }

    cmd.args([nvim_path, "+normal G$", file_path]);

    let child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn kitty: {}", e))?;

    let pid = child.id();

    // Move window to position if geometry specified (using AppleScript)
    if let Some(ref geo) = geometry {
        move_window_to_position("kitty", geo.x, geo.y);
    }

    Ok(SpawnInfo {
        terminal_type: TerminalType::Kitty,
        process_id: Some(pid),
        child: Some(child),
        window_title: None,
    })
}

/// Spawn WezTerm terminal
fn spawn_wezterm(nvim_path: &str, file_path: &str, geometry: Option<WindowGeometry>) -> Result<SpawnInfo, String> {
    // Use "+normal G$" to move cursor to end of file
    let mut cmd = Command::new("wezterm");

    // WezTerm supports --position for window placement
    if let Some(geo) = geometry {
        cmd.args([
            "start",
            "--position", &format!("{},{}", geo.x, geo.y),
            "--width", &format!("{}", geo.width),
            "--height", &format!("{}", geo.height),
            "--",
        ]);
    } else {
        cmd.args(["start", "--"]);
    }

    cmd.args([nvim_path, "+normal G$", file_path]);

    let child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn wezterm: {}", e))?;

    let pid = child.id();

    Ok(SpawnInfo {
        terminal_type: TerminalType::WezTerm,
        process_id: Some(pid),
        child: Some(child),
        window_title: None,
    })
}

/// Spawn iTerm2 using AppleScript
fn spawn_iterm(nvim_path: &str, file_path: &str, geometry: Option<WindowGeometry>) -> Result<SpawnInfo, String> {
    // Use AppleScript to open iTerm and run nvim with position/size
    // Use "+normal G$" to move cursor to end of file
    let script = if let Some(geo) = geometry {
        format!(
            r#"
            tell application "iTerm"
                activate
                set newWindow to (create window with default profile)
                set bounds of newWindow to {{{}, {}, {}, {}}}
                tell current session of newWindow
                    write text "{} '+normal G$' '{}'"
                end tell
            end tell
            "#,
            geo.x, geo.y, geo.x + geo.width as i32, geo.y + geo.height as i32,
            nvim_path, file_path
        )
    } else {
        format!(
            r#"
            tell application "iTerm"
                activate
                set newWindow to (create window with default profile)
                tell current session of newWindow
                    write text "{} '+normal G$' '{}'"
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

/// Spawn Terminal.app using AppleScript
fn spawn_terminal_app(nvim_path: &str, file_path: &str, geometry: Option<WindowGeometry>) -> Result<SpawnInfo, String> {
    // Use "+normal G$" to move cursor to end of file
    let script = if let Some(geo) = geometry {
        format!(
            r#"
            tell application "Terminal"
                activate
                do script "{} '+normal G$' '{}'"
                set bounds of front window to {{{}, {}, {}, {}}}
            end tell
            "#,
            nvim_path, file_path,
            geo.x, geo.y, geo.x + geo.width as i32, geo.y + geo.height as i32
        )
    } else {
        format!(
            r#"
            tell application "Terminal"
                activate
                do script "{} '+normal G$' '{}'"
            end tell
            "#,
            nvim_path, file_path
        )
    };

    Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("Failed to run Terminal AppleScript: {}", e))?;

    // Try to find the nvim process ID by the file it's editing
    let pid = find_nvim_pid_for_file(file_path);
    log::info!("Found nvim PID: {:?} for file: {}", pid, file_path);

    Ok(SpawnInfo {
        terminal_type: TerminalType::Default,
        process_id: pid,
        child: None,
        window_title: None,
    })
}

/// Find the nvim process editing a specific file
fn find_nvim_pid_for_file(file_path: &str) -> Option<u32> {
    // Small delay to let nvim start
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Use lsof to find the process that has our file open
    let output = Command::new("lsof")
        .args(["-t", file_path])
        .output()
        .ok()?;

    if output.status.success() {
        let pids = String::from_utf8_lossy(&output.stdout);
        // Take the first PID (there should only be one)
        for line in pids.lines() {
            if let Ok(pid) = line.trim().parse::<u32>() {
                return Some(pid);
            }
        }
    }

    // Fallback: find most recent nvim
    find_nvim_pid()
}

/// Find the most recently started nvim process ID
fn find_nvim_pid() -> Option<u32> {
    let output = Command::new("pgrep")
        .args(["-n", "nvim"])
        .output()
        .ok()?;

    if output.status.success() {
        let pid_str = String::from_utf8_lossy(&output.stdout);
        pid_str.trim().parse().ok()
    } else {
        None
    }
}

/// Move a window to a specific position using AppleScript
fn move_window_to_position(app_name: &str, x: i32, y: i32) {
    // Small delay to let the window appear
    std::thread::sleep(std::time::Duration::from_millis(200));

    let script = format!(
        r#"
        tell application "System Events"
            tell process "{}"
                try
                    set position of front window to {{{}, {}}}
                end try
            end tell
        end tell
        "#,
        app_name, x, y
    );

    let _ = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output();
}

/// Resolve a command name to its absolute path using `which`
fn resolve_command_path(cmd: &str) -> String {
    // If already absolute path, return as-is
    if cmd.starts_with('/') {
        return cmd.to_string();
    }

    // Try to resolve using `which`
    if let Ok(output) = Command::new("which").arg(cmd).output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return path;
            }
        }
    }

    // Fallback: return original (might work if PATH is set)
    cmd.to_string()
}

/// Find Alacritty window index by title (returns 1-based index)
fn find_alacritty_window_by_title(title: &str) -> Option<usize> {
    // First, let's list all window titles to debug
    let list_script = r#"
        tell application "System Events"
            tell process "Alacritty"
                set windowNames to {}
                repeat with i from 1 to (count of windows)
                    set end of windowNames to name of window i
                end repeat
                return windowNames
            end tell
        end tell
        "#;

    if let Ok(out) = Command::new("osascript").arg("-e").arg(list_script).output() {
        log::info!("Alacritty window titles: {}", String::from_utf8_lossy(&out.stdout).trim());
    }

    let script = format!(
        r#"
        tell application "System Events"
            tell process "Alacritty"
                set windowIndex to 0
                repeat with i from 1 to (count of windows)
                    set w to window i
                    if name of w contains "{}" then
                        return i
                    end if
                end repeat
                return 0
            end tell
        end tell
        "#,
        title
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output();

    if let Ok(out) = output {
        if out.status.success() {
            let index_str = String::from_utf8_lossy(&out.stdout);
            let index: usize = index_str.trim().parse().unwrap_or(0);
            log::info!("Search for '{}' returned index: {}", title, index);
            if index > 0 {
                return Some(index);
            }
        }
    }
    None
}

/// Set window bounds by window index (1-based)
#[allow(dead_code)]
fn set_window_bounds_by_index(app_name: &str, index: usize, x: i32, y: i32, width: u32, height: u32) {
    let script = format!(
        r#"
        tell application "System Events"
            tell process "{}"
                if (count of windows) >= {} then
                    set w to window {}
                    set position of w to {{{}, {}}}
                    set size of w to {{{}, {}}}
                end if
            end tell
        end tell
        "#,
        app_name, index, index, x, y, width, height
    );

    log::info!("Setting window {} index {} bounds: {}x{} at ({}, {})", app_name, index, width, height, x, y);

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output();

    if let Ok(out) = output {
        if !out.status.success() {
            log::error!("AppleScript failed: {}", String::from_utf8_lossy(&out.stderr));
        }
    }
}


/// Focus an Alacritty window by index (without bringing all app windows to front)
fn focus_alacritty_window_by_index(index: usize) {
    // Use AXRaise to bring the specific window to front, then set frontmost
    // to give it focus. Avoid "activate" which brings ALL app windows forward.
    let script = format!(
        r#"
        tell application "System Events"
            tell process "Alacritty"
                if (count of windows) >= {} then
                    set w to window {}
                    perform action "AXRaise" of w
                    set frontmost to true
                end if
            end tell
        end tell
        "#,
        index, index
    );

    log::info!("Focusing Alacritty window at index {}", index);

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output();

    if let Ok(out) = output {
        if !out.status.success() {
            log::error!("Failed to focus window: {}", String::from_utf8_lossy(&out.stderr));
        }
    }
}

/// Set window bounds atomically (position and size in one call)
fn set_window_bounds_atomic(app_name: &str, index: usize, x: i32, y: i32, width: u32, height: u32) {
    // Set both position and size in a single script for speed
    let script = format!(
        r#"
        tell application "System Events"
            tell process "{}"
                if (count of windows) >= {} then
                    set w to window {}
                    set position of w to {{{}, {}}}
                    set size of w to {{{}, {}}}
                end if
            end tell
        end tell
        "#,
        app_name, index, index, x, y, width, height
    );

    log::info!("Setting window {} index {} to {}x{} at ({}, {})", app_name, index, width, height, x, y);

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output();

    if let Ok(out) = output {
        if !out.status.success() {
            log::error!("AppleScript set bounds failed: {}", String::from_utf8_lossy(&out.stderr));
        }
    }
}
