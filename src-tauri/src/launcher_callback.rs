//! Launcher script callback handling
//!
//! This module manages channels for launcher scripts to signal their intent
//! back to the main app via IPC.

use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::oneshot;

/// Callback from launcher script
#[derive(Debug)]
pub enum LauncherCallback {
    /// Script handled spawning, optionally with editor PID
    Handled { editor_pid: Option<u32> },
    /// Script wants normal terminal flow
    Fallthrough,
}

/// Global registry of pending launcher callbacks
static LAUNCHER_CALLBACKS: Mutex<Option<HashMap<String, oneshot::Sender<LauncherCallback>>>> =
    Mutex::new(None);

/// Initialize the callback registry (call once at startup)
pub fn init() {
    let mut callbacks = LAUNCHER_CALLBACKS.lock().unwrap();
    *callbacks = Some(HashMap::new());
}

/// Register a callback channel for a session
/// Returns a receiver that will get the callback when the script signals
pub fn register(session_id: String) -> oneshot::Receiver<LauncherCallback> {
    let (tx, rx) = oneshot::channel();

    let mut callbacks = LAUNCHER_CALLBACKS.lock().unwrap();
    if let Some(map) = callbacks.as_mut() {
        map.insert(session_id, tx);
    }

    rx
}

/// Unregister a callback (cleanup on timeout or completion)
pub fn unregister(session_id: &str) {
    let mut callbacks = LAUNCHER_CALLBACKS.lock().unwrap();
    if let Some(map) = callbacks.as_mut() {
        map.remove(session_id);
    }
}

/// Signal that the launcher handled spawning
/// Returns true if the session was found and signaled
pub fn signal_handled(session_id: &str, editor_pid: Option<u32>) -> bool {
    let mut callbacks = LAUNCHER_CALLBACKS.lock().unwrap();
    if let Some(map) = callbacks.as_mut() {
        if let Some(tx) = map.remove(session_id) {
            let _ = tx.send(LauncherCallback::Handled { editor_pid });
            return true;
        }
    }
    false
}

/// Signal that the launcher wants fallthrough to normal terminal
/// Returns true if the session was found and signaled
pub fn signal_fallthrough(session_id: &str) -> bool {
    let mut callbacks = LAUNCHER_CALLBACKS.lock().unwrap();
    if let Some(map) = callbacks.as_mut() {
        if let Some(tx) = map.remove(session_id) {
            let _ = tx.send(LauncherCallback::Fallthrough);
            return true;
        }
    }
    false
}
