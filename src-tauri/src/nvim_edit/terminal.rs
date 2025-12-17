//! Terminal spawning for "Edit with Neovim" feature

use std::path::Path;
use std::process::{Child, Command};

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
}

/// Spawn a terminal with nvim editing the given file
pub fn spawn_terminal(
    terminal: &str,
    nvim_path: &str,
    temp_file: &Path,
) -> Result<SpawnInfo, String> {
    let terminal_type = TerminalType::from_string(terminal);
    let file_path = temp_file.to_string_lossy();

    match terminal_type {
        TerminalType::Alacritty => spawn_alacritty(nvim_path, &file_path),
        TerminalType::Kitty => spawn_kitty(nvim_path, &file_path),
        TerminalType::WezTerm => spawn_wezterm(nvim_path, &file_path),
        TerminalType::ITerm => spawn_iterm(nvim_path, &file_path),
        TerminalType::Default => spawn_terminal_app(nvim_path, &file_path),
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
        thread::sleep(Duration::from_millis(100));
    }

    Ok(())
}

/// Spawn Alacritty terminal
fn spawn_alacritty(nvim_path: &str, file_path: &str) -> Result<SpawnInfo, String> {
    // Alacritty may be running as a daemon - the `-e` command connects to it
    // and returns immediately. We need to find the actual nvim process.
    // Use "+normal G$" to move cursor to end of file (G = last line, $ = end of line)
    let _child = Command::new("alacritty")
        .args(["-e", nvim_path, "+normal G$", file_path])
        .spawn()
        .map_err(|e| format!("Failed to spawn alacritty: {}", e))?;

    // Wait a bit for nvim to start, then find its PID by the file it's editing
    let pid = find_nvim_pid_for_file(file_path);
    log::info!("Found nvim PID: {:?} for file: {}", pid, file_path);

    Ok(SpawnInfo {
        terminal_type: TerminalType::Alacritty,
        process_id: pid,
        child: None, // Don't track the wrapper process
    })
}

/// Spawn Kitty terminal
fn spawn_kitty(nvim_path: &str, file_path: &str) -> Result<SpawnInfo, String> {
    // Use "+normal G$" to move cursor to end of file
    let child = Command::new("kitty")
        .args([nvim_path, "+normal G$", file_path])
        .spawn()
        .map_err(|e| format!("Failed to spawn kitty: {}", e))?;

    let pid = child.id();

    Ok(SpawnInfo {
        terminal_type: TerminalType::Kitty,
        process_id: Some(pid),
        child: Some(child),
    })
}

/// Spawn WezTerm terminal
fn spawn_wezterm(nvim_path: &str, file_path: &str) -> Result<SpawnInfo, String> {
    // Use "+normal G$" to move cursor to end of file
    let child = Command::new("wezterm")
        .args(["start", "--", nvim_path, "+normal G$", file_path])
        .spawn()
        .map_err(|e| format!("Failed to spawn wezterm: {}", e))?;

    let pid = child.id();

    Ok(SpawnInfo {
        terminal_type: TerminalType::WezTerm,
        process_id: Some(pid),
        child: Some(child),
    })
}

/// Spawn iTerm2 using AppleScript
fn spawn_iterm(nvim_path: &str, file_path: &str) -> Result<SpawnInfo, String> {
    // Use AppleScript to open iTerm and run nvim
    // Use "+" to move cursor to end of file
    let script = format!(
        r#"
        tell application "iTerm"
            activate
            set newWindow to (create window with default profile)
            tell current session of newWindow
                write text "{} + '{}'"
            end tell
        end tell
        "#,
        nvim_path, file_path
    );

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
    })
}

/// Spawn Terminal.app using AppleScript
fn spawn_terminal_app(nvim_path: &str, file_path: &str) -> Result<SpawnInfo, String> {
    // Use "+" to move cursor to end of file
    let script = format!(
        r#"
        tell application "Terminal"
            activate
            do script "{} + '{}'"
        end tell
        "#,
        nvim_path, file_path
    );

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
