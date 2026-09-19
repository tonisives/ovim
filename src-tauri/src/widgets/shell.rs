use std::process::Command;

/// Run a shell script (inline or from file) and return its stdout output.
/// Returns the trimmed output string, or an error message on failure.
pub fn run_shell_script(script: Option<&str>, script_path: Option<&str>) -> String {
    let result = if let Some(inline) = script {
        Command::new("/bin/sh").arg("-c").arg(inline).output()
    } else if let Some(path) = script_path {
        // Expand ~ to home directory
        let expanded = if let Some(relative_path) = path.strip_prefix("~/") {
            if let Some(home) = dirs::home_dir() {
                home.join(relative_path).to_string_lossy().to_string()
            } else {
                path.to_string()
            }
        } else {
            path.to_string()
        };
        Command::new("/bin/sh").arg(&expanded).output()
    } else {
        return "no script".to_string();
    };

    match result {
        Ok(output) => {
            if output.status.success() {
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                if stderr.is_empty() {
                    format!("exit {}", output.status.code().unwrap_or(-1))
                } else {
                    stderr
                }
            }
        }
        Err(e) => format!("err: {}", e),
    }
}
