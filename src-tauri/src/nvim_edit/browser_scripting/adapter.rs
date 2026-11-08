//! User-defined edit transport, following the terminal launcher's script model.

use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{json, Value};

use super::super::accessibility::ElementFrame;
use super::types::TextAndCursor;

#[derive(Clone)]
pub struct AdapterTarget {
    script: PathBuf,
    context: Value,
    pub capture: TextAndCursor,
    pub frame: Option<ElementFrame>,
    pub hostname: Option<String>,
}

impl std::fmt::Debug for AdapterTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdapterTarget")
            .field("script", &self.script)
            .finish()
    }
}

impl AdapterTarget {
    pub fn call(&self, operation: &str, mut request: Value) -> Result<Value, String> {
        request["context"] = self.context.clone();
        invoke(&self.script, operation, request)
    }
}

#[derive(Deserialize)]
struct Frame {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

pub fn capture(bundle_id: &str) -> Result<Option<AdapterTarget>, String> {
    let Some(script) = dirs::config_dir().map(|dir| dir.join("ovim/edit-adapter")) else {
        return Ok(None);
    };
    if !script.is_file() {
        return Ok(None);
    }
    let response = invoke(&script, "capture", json!({"bundle_id": bundle_id}))?;
    if response.get("handled").and_then(Value::as_bool) == Some(false) {
        return Ok(None);
    }
    if response.get("handled").and_then(Value::as_bool) != Some(true) {
        return Err("Edit adapter capture must return handled: true or false".into());
    }
    let capture = super::parsing::parse_text_and_cursor_json(&response.to_string())
        .ok_or("Invalid edit adapter capture")?;
    let context = response
        .get("context")
        .cloned()
        .ok_or("Edit adapter capture returned no context")?;
    let frame = response
        .get("frame")
        .filter(|frame| !frame.is_null())
        .map(|frame| {
            serde_json::from_value::<Frame>(frame.clone())
                .map(|frame| ElementFrame {
                    x: frame.x,
                    y: frame.y,
                    width: frame.width,
                    height: frame.height,
                })
                .map_err(|_| "Invalid edit adapter frame".to_string())
        })
        .transpose()?;
    Ok(Some(AdapterTarget {
        script,
        context,
        capture,
        frame,
        hostname: response
            .get("hostname")
            .and_then(Value::as_str)
            .map(str::to_string),
    }))
}

fn invoke(script: &Path, operation: &str, request: Value) -> Result<Value, String> {
    let mut child = Command::new(script)
        .arg(operation)
        .process_group(0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Could not launch edit adapter: {error}"))?;
    let mut input = child.stdin.take().ok_or("Missing adapter stdin")?;
    let payload = request.to_string();
    let writer = std::thread::spawn(move || input.write_all(payload.as_bytes()));
    let mut output = child.stdout.take().ok_or("Missing adapter stdout")?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        output.read_to_end(&mut bytes).map(|_| bytes)
    });
    let mut errors = child.stderr.take().ok_or("Missing adapter stderr")?;
    let error_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        errors.read_to_end(&mut bytes).map(|_| bytes)
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            break status;
        }
        if started.elapsed() > Duration::from_secs(10) {
            // Stop the script's input commands as well as the script itself.
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.wait();
            return Err(format!("Edit adapter {operation} timed out"));
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    writer
        .join()
        .map_err(|_| "Adapter input thread failed")?
        .map_err(|error| error.to_string())?;
    let output = reader
        .join()
        .map_err(|_| "Adapter output thread failed")?
        .map_err(|error| error.to_string())?;
    let _ = error_reader.join();
    if !status.success() {
        // Adapter stderr can contain editor text; report the operation and exit status.
        return Err(format!("Edit adapter {operation} failed: {status}"));
    }
    serde_json::from_slice(&output).map_err(|error| format!("Invalid edit adapter JSON: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn transports_unicode_text_and_opaque_context_over_stdin() {
        let directory = std::env::temp_dir().join(format!("ovim-adapter-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let script = directory.join("adapter");
        std::fs::write(&script, "#!/bin/sh\ncat\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        let request = json!({"text": "hello\nไทย \"quote\" \\ $()", "context": {"target": "%7"}});
        assert_eq!(invoke(&script, "update", request.clone()).unwrap(), request);
        std::fs::write(&script, "#!/bin/sh\ncat >/dev/null\nprintf invalid\n").unwrap();
        assert!(invoke(&script, "update", json!({}))
            .unwrap_err()
            .contains("Invalid edit adapter JSON"));
        std::fs::write(&script, "#!/bin/sh\ncat >/dev/null\nexit 2\n").unwrap();
        assert!(invoke(&script, "capture", json!({}))
            .unwrap_err()
            .contains("capture failed"));
        std::fs::remove_dir_all(directory).unwrap();
    }
}
