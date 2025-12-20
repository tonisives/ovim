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
    Ghostty,
    Kitty,
    WezTerm,
    ITerm,
    Default, // Terminal.app
}

impl TerminalType {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "alacritty" => TerminalType::Alacritty,
            "ghostty" => TerminalType::Ghostty,
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
        TerminalType::Ghostty => spawn_ghostty(nvim_path, &file_path, geometry),
        TerminalType::Kitty => spawn_kitty(nvim_path, &file_path, geometry),
        TerminalType::WezTerm => spawn_wezterm(nvim_path, &file_path, geometry),
        TerminalType::ITerm => spawn_iterm(nvim_path, &file_path, geometry),
        TerminalType::Default => spawn_terminal_app(nvim_path, &file_path, geometry),
    }
}

/// Wait for the terminal/nvim process to exit
pub fn wait_for_process(terminal_type: &TerminalType, process_id: Option<u32>) -> Result<(), String> {
    match terminal_type {
        TerminalType::Alacritty | TerminalType::Ghostty | TerminalType::Kitty | TerminalType::WezTerm => {
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
        // First try waitpid with WNOHANG to reap zombie children (for processes we spawned)
        let mut status: libc::c_int = 0;
        let wait_result = unsafe { libc::waitpid(pid as i32, &mut status, libc::WNOHANG) };

        if wait_result == pid as i32 {
            // Process has exited and been reaped
            log::info!("Process {} reaped via waitpid", pid);
            break;
        } else if wait_result == -1 {
            // Error - check errno
            let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
            if errno == libc::ECHILD {
                // Not our child - fall back to kill(pid, 0) check
                let kill_result = unsafe { libc::kill(pid as i32, 0) };
                if kill_result != 0 {
                    // Process doesn't exist
                    log::info!("Process {} no longer exists (kill check)", pid);
                    break;
                }
            } else {
                // Other error
                log::warn!("waitpid error for {}: errno={}", pid, errno);
                break;
            }
        }
        // wait_result == 0 means process still running, continue polling

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

/// Spawn Ghostty terminal
fn spawn_ghostty(nvim_path: &str, file_path: &str, geometry: Option<WindowGeometry>) -> Result<SpawnInfo, String> {
    // Generate a unique window title so we can find it
    let unique_title = format!("ovim-edit-{}", std::process::id());

    // Resolve nvim path to absolute path
    let resolved_nvim = resolve_command_path(nvim_path);
    log::info!("Resolved nvim path: {} -> {}", nvim_path, resolved_nvim);

    // On macOS, Ghostty must be launched via `open -na Ghostty.app --args ...`
    let mut cmd = Command::new("open");
    cmd.args(["-na", "Ghostty.app", "--args"]);

    // Add window title
    cmd.args([&format!("--title={}", unique_title)]);

    // Add geometry if provided
    if let Some(ref geo) = geometry {
        // Ghostty window-width/height are in terminal grid cells, not pixels
        // Approximate: 8px per column, 16px per row
        let cols = (geo.width / 8).max(10);
        let rows = (geo.height / 16).max(4);
        cmd.args([
            &format!("--window-width={}", cols),
            &format!("--window-height={}", rows),
            &format!("--window-position-x={}", geo.x),
            &format!("--window-position-y={}", geo.y),
        ]);
    }

    // Execute nvim using -e flag
    cmd.args(["-e", &resolved_nvim, "+normal G$", file_path]);

    cmd.spawn()
        .map_err(|e| format!("Failed to spawn ghostty: {}", e))?;

    // Wait a bit for nvim to start, then find its PID by the file it's editing
    let pid = find_nvim_pid_for_file(file_path);
    log::info!("Found nvim PID: {:?} for file: {}", pid, file_path);

    Ok(SpawnInfo {
        terminal_type: TerminalType::Ghostty,
        process_id: pid,
        child: None, // open command returns immediately
        window_title: Some(unique_title),
    })
}

/// Spawn Kitty terminal
fn spawn_kitty(nvim_path: &str, file_path: &str, geometry: Option<WindowGeometry>) -> Result<SpawnInfo, String> {
    // Generate a unique window title
    let unique_title = format!("ovim-edit-{}", std::process::id());

    // Resolve nvim path
    let resolved_nvim = resolve_command_path(nvim_path);
    log::info!("Resolved nvim path: {} -> {}", nvim_path, resolved_nvim);

    // Try to find kitty - check common locations on macOS
    let kitty_path = if std::path::Path::new("/Applications/kitty.app/Contents/MacOS/kitty").exists() {
        "/Applications/kitty.app/Contents/MacOS/kitty"
    } else {
        "kitty" // Fall back to PATH
    };

    let mut cmd = Command::new(kitty_path);

    // Use single instance to avoid multiple dock icons, close window when nvim exits
    cmd.args(["--single-instance", "--wait-for-single-instance-window-close"]);
    cmd.args(["--title", &unique_title]);
    cmd.args(["-o", "close_on_child_death=yes"]);

    // Add window position/size if provided
    // Kitty uses -o for config overrides and --position for placement
    if let Some(ref geo) = geometry {
        cmd.args([
            "--position", &format!("{}x{}", geo.x, geo.y),
            "-o", &format!("initial_window_width={}c", geo.width / 8), // Convert to cells (approx 8px per char)
            "-o", &format!("initial_window_height={}c", geo.height / 16), // Convert to cells (approx 16px per line)
            "-o", "remember_window_size=no",
        ]);
    }

    // Kitty runs the command directly (no -e flag needed)
    cmd.args([&resolved_nvim, "+normal G$", file_path]);

    let child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn kitty: {}", e))?;

    // Wait a bit for nvim to start, then find its PID by the file it's editing
    // We need to track nvim's PID, not kitty's, to know when editing is done
    let pid = find_nvim_pid_for_file(file_path);
    log::info!("Found nvim PID: {:?} for file: {}", pid, file_path);

    Ok(SpawnInfo {
        terminal_type: TerminalType::Kitty,
        process_id: pid,
        child: Some(child),
        window_title: Some(unique_title),
    })
}

/// Spawn WezTerm terminal
fn spawn_wezterm(nvim_path: &str, file_path: &str, geometry: Option<WindowGeometry>) -> Result<SpawnInfo, String> {
    // Resolve nvim path
    let resolved_nvim = resolve_command_path(nvim_path);
    log::info!("Resolved nvim path: {} -> {}", nvim_path, resolved_nvim);

    let mut cmd = Command::new("wezterm");

    // Use --always-new-process so wezterm blocks until the command exits.
    // Without this flag, wezterm start returns immediately by connecting to
    // an existing GUI instance, and we can't wait for nvim to finish.
    // WezTerm only supports --position for window placement (no --width/--height)
    // Size must be set via config (initial_rows/initial_cols)
    // Use "screen:" prefix for absolute screen coordinates (matching macOS accessibility API)
    if let Some(ref geo) = geometry {
        cmd.args([
            "start",
            "--always-new-process",
            "--position", &format!("screen:{},{}", geo.x, geo.y),
            "--",
        ]);
    } else {
        cmd.args(["start", "--always-new-process", "--"]);
    }

    cmd.args([&resolved_nvim, "+normal G$", file_path]);

    let child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn wezterm: {}", e))?;

    // Get the wezterm process PID - with --always-new-process, the wezterm
    // process itself will block until nvim exits, so we can track it directly
    let wezterm_pid = child.id();
    log::info!("WezTerm process PID: {}", wezterm_pid);

    // If geometry specified, try to resize using AppleScript after window appears
    if let Some(ref geo) = geometry {
        let width = geo.width;
        let height = geo.height;
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            set_window_size_applescript("WezTerm", width, height);
        });
    }

    Ok(SpawnInfo {
        terminal_type: TerminalType::WezTerm,
        process_id: Some(wezterm_pid),
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

/// Set window size using AppleScript
fn set_window_size_applescript(app_name: &str, width: u32, height: u32) {
    let script = format!(
        r#"
        tell application "System Events"
            tell process "{}"
                try
                    set size of front window to {{{}, {}}}
                end try
            end tell
        end tell
        "#,
        app_name, width, height
    );

    let _ = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output();
}

/// Move a window to a specific position using AppleScript
#[allow(dead_code)]
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
    // Use AXRaise to bring the specific window to front and give it keyboard focus.
    // We avoid "activate" and "set frontmost to true" on the process which bring ALL windows forward.
    // Instead, we use AXRaise and then simulate a mouse click using CGEvent to focus just this window.
    let script = format!(
        r#"
        tell application "System Events"
            tell process "Alacritty"
                if (count of windows) >= {} then
                    set w to window {}
                    -- Raise just this window to the front
                    perform action "AXRaise" of w
                    -- Return the window position for clicking
                    set winPos to position of w
                    return (item 1 of winPos) & "," & (item 2 of winPos)
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
        if out.status.success() {
            // Parse the position and click to give keyboard focus
            let pos_str = String::from_utf8_lossy(&out.stdout);
            let pos_str = pos_str.trim();
            if let Some((x_str, y_str)) = pos_str.split_once(',') {
                if let (Ok(x), Ok(y)) = (x_str.trim().parse::<i32>(), y_str.trim().parse::<i32>()) {
                    // Click inside the window to give it keyboard focus
                    click_at_position(x + 50, y + 50);
                }
            }
        } else {
            log::error!("Failed to focus window: {}", String::from_utf8_lossy(&out.stderr));
        }
    }
}

/// Click at a screen position using CGEvent (gives keyboard focus to the window)
fn click_at_position(x: i32, y: i32) {
    use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use core_graphics::geometry::CGPoint;

    let point = CGPoint::new(x as f64, y as f64);

    if let Ok(source) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
        // Mouse down
        if let Ok(event) = CGEvent::new_mouse_event(
            source.clone(),
            CGEventType::LeftMouseDown,
            point,
            CGMouseButton::Left,
        ) {
            event.post(CGEventTapLocation::HID);
        }

        // Mouse up
        if let Ok(event) = CGEvent::new_mouse_event(
            source,
            CGEventType::LeftMouseUp,
            point,
            CGMouseButton::Left,
        ) {
            event.post(CGEventTapLocation::HID);
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
