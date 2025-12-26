//! Environment capture from launcher script
//!
//! This module handles capturing environment variables from the user's
//! launcher script, allowing users to customize PATH and other env vars
//! that affect the spawned terminal and editor.

use std::collections::HashMap;
use std::process::Command;

use super::ensure_launcher_script;

/// Capture environment variables from the launcher script.
///
/// Sources the script and captures any environment variable changes.
/// Returns only the variables that differ from the current process environment.
pub fn capture_script_environment() -> Result<HashMap<String, String>, String> {
    let script_path = ensure_launcher_script()?;

    // Get current environment for comparison
    let current_env: HashMap<String, String> = std::env::vars().collect();

    // Source the script and capture resulting environment
    // We use a subshell to prevent 'exit' in the script from terminating our capture
    // The script's exports are captured, then we print env
    let output = Command::new("bash")
        .arg("-c")
        .arg(format!(
            r#"
            # Source script in a way that captures exports but ignores exit
            eval "$(grep -E '^export ' {:?} 2>/dev/null || true)"
            env
            "#,
            script_path
        ))
        .output()
        .map_err(|e| format!("Failed to run launcher script: {}", e))?;

    if !output.status.success() {
        // Script may have exited with non-zero, but we still want the env
        log::debug!(
            "Launcher script exited with non-zero status (this is normal for built-in terminals)"
        );
    }

    let output_str = String::from_utf8_lossy(&output.stdout);

    // Parse the environment output and find differences
    let custom_env = parse_env_output(&output_str, &current_env);

    log::info!(
        "Captured custom env vars: {:?}",
        custom_env.keys().collect::<Vec<_>>()
    );
    if let Some(path) = custom_env.get("PATH") {
        log::info!("Custom PATH: {}", path);
    }

    // Debug: write to file
    let debug_info = format!(
        "Captured {} env vars\nKeys: {:?}\nPATH: {:?}\n",
        custom_env.len(),
        custom_env.keys().collect::<Vec<_>>(),
        custom_env.get("PATH")
    );
    let _ = std::fs::write("/tmp/ovim-env-debug.log", debug_info);

    Ok(custom_env)
}

/// Parse env command output and extract variables that differ from current environment.
///
/// Handles multiline values correctly - a new env var starts with a valid name
/// (alphanumeric + underscore, not starting with digit) followed by '='.
fn parse_env_output(
    output_str: &str,
    current_env: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut custom_env = HashMap::new();
    let mut current_key: Option<String> = None;
    let mut current_value = String::new();

    for line in output_str.lines() {
        // Check if this line starts a new environment variable
        if let Some(eq_pos) = line.find('=') {
            let potential_key = &line[..eq_pos];
            let is_valid_key = is_valid_env_key(potential_key);

            if is_valid_key {
                // Save previous key-value pair if any
                if let Some(key) = current_key.take() {
                    maybe_insert_env_var(&mut custom_env, &key, &current_value, current_env);
                }
                // Start new key-value
                current_key = Some(potential_key.to_string());
                current_value = line[eq_pos + 1..].to_string();
                continue;
            }
        }

        // This line is a continuation of the previous value
        if current_key.is_some() {
            current_value.push('\n');
            current_value.push_str(line);
        }
    }

    // Don't forget the last key-value pair
    if let Some(key) = current_key {
        maybe_insert_env_var(&mut custom_env, &key, &current_value, current_env);
    }

    // Filter to only include vars that differ from current process env
    custom_env
        .into_iter()
        .filter(|(k, v)| match current_env.get(k) {
            Some(current_val) => current_val != v,
            None => true,
        })
        .collect()
}

/// Check if a string is a valid environment variable name.
fn is_valid_env_key(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .next()
            .map_or(false, |c| c.is_alphabetic() || c == '_')
        && s.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// Insert an env var if it's new or changed, and not a shell internal var.
fn maybe_insert_env_var(
    custom_env: &mut HashMap<String, String>,
    key: &str,
    value: &str,
    current_env: &HashMap<String, String>,
) {
    // Skip shell internal variables
    if key.starts_with("BASH_") || key == "_" || key == "SHLVL" || key == "PWD" {
        return;
    }

    let is_new_or_changed = match current_env.get(key) {
        Some(existing) => existing != value,
        None => true,
    };

    if is_new_or_changed {
        custom_env.insert(key.to_string(), value.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_env_key() {
        assert!(is_valid_env_key("PATH"));
        assert!(is_valid_env_key("HOME"));
        assert!(is_valid_env_key("MY_VAR"));
        assert!(is_valid_env_key("_private"));
        assert!(is_valid_env_key("VAR123"));

        assert!(!is_valid_env_key(""));
        assert!(!is_valid_env_key("123VAR"));
        assert!(!is_valid_env_key("MY-VAR"));
        assert!(!is_valid_env_key("MY.VAR"));
    }

    #[test]
    fn test_parse_env_output_simple() {
        let output = "PATH=/usr/bin\nHOME=/home/user\n";
        let current = HashMap::new();
        let result = parse_env_output(output, &current);

        assert_eq!(result.get("PATH"), Some(&"/usr/bin".to_string()));
        assert_eq!(result.get("HOME"), Some(&"/home/user".to_string()));
    }

    #[test]
    fn test_parse_env_output_multiline() {
        let output = "SIMPLE=value\nMULTILINE=line1\nline2\nline3\nNEXT=after\n";
        let current = HashMap::new();
        let result = parse_env_output(output, &current);

        assert_eq!(result.get("SIMPLE"), Some(&"value".to_string()));
        assert_eq!(
            result.get("MULTILINE"),
            Some(&"line1\nline2\nline3".to_string())
        );
        assert_eq!(result.get("NEXT"), Some(&"after".to_string()));
    }

    #[test]
    fn test_parse_env_output_filters_unchanged() {
        let output = "PATH=/usr/bin\nHOME=/home/user\n";
        let mut current = HashMap::new();
        current.insert("PATH".to_string(), "/usr/bin".to_string()); // Same value

        let result = parse_env_output(output, &current);

        assert!(result.get("PATH").is_none()); // Filtered out (unchanged)
        assert_eq!(result.get("HOME"), Some(&"/home/user".to_string()));
    }
}
