//! Process management utilities for terminal spawning

use std::process::Command;
use std::thread;
use std::time::Duration;

/// Wait for a specific PID to exit
pub fn wait_for_pid(pid: u32) -> Result<(), String> {
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

/// Find the nvim process editing a specific file
pub fn find_nvim_pid_for_file(file_path: &str) -> Option<u32> {
    // Small delay to let nvim start
    thread::sleep(Duration::from_millis(500));

    // Use lsof to find the process that has our file open
    let output = Command::new("lsof").args(["-t", file_path]).output().ok()?;

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
    let output = Command::new("pgrep").args(["-n", "nvim"]).output().ok()?;

    if output.status.success() {
        let pid_str = String::from_utf8_lossy(&output.stdout);
        pid_str.trim().parse().ok()
    } else {
        None
    }
}

/// Resolve a command name to its absolute path using `which`
pub fn resolve_command_path(cmd: &str) -> String {
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
