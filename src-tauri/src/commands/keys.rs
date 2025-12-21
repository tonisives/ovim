//! Key recording and display Tauri commands

use tauri::State;

use crate::keyboard::KeyCode;
use crate::AppState;

/// Recorded key info returned to frontend
#[derive(Debug, Clone, serde::Serialize)]
pub struct RecordedKey {
    pub name: String,
    pub display_name: String,
    pub modifiers: RecordedModifiers,
}

/// Modifier state for recorded key
#[derive(Debug, Clone, serde::Serialize)]
pub struct RecordedModifiers {
    pub shift: bool,
    pub control: bool,
    pub option: bool,
    pub command: bool,
}

#[tauri::command]
pub fn get_key_display_name(key_name: String) -> Option<String> {
    KeyCode::from_name(&key_name).map(|k| k.to_display_name().to_string())
}

#[tauri::command]
pub async fn record_key(state: State<'_, AppState>) -> Result<RecordedKey, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    {
        let mut record_tx = state.record_key_tx.lock().unwrap();
        *record_tx = Some(tx);
    }

    rx.await.map_err(|_| "Key recording cancelled".to_string())
}

#[tauri::command]
pub fn cancel_record_key(state: State<AppState>) {
    let mut record_tx = state.record_key_tx.lock().unwrap();
    *record_tx = None;
}
