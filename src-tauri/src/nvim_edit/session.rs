//! Edit session management for "Edit with Neovim" feature

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use uuid::Uuid;

use super::accessibility::FocusContext;
use super::terminals::{spawn_terminal, SpawnInfo, TerminalType, WindowGeometry};
use crate::config::NvimEditSettings;

/// An active edit session
pub struct EditSession {
    pub id: Uuid,
    pub focus_context: FocusContext,
    pub original_text: String,
    pub temp_file: PathBuf,
    pub file_mtime: SystemTime,
    pub terminal_type: TerminalType,
    pub process_id: Option<u32>,
    pub window_title: Option<String>,
}

/// Manager for edit sessions
pub struct EditSessionManager {
    sessions: Arc<Mutex<HashMap<Uuid, EditSession>>>,
}

impl EditSessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Start a new edit session
    pub fn start_session(
        &self,
        focus_context: FocusContext,
        text: String,
        settings: NvimEditSettings,
        geometry: Option<WindowGeometry>,
    ) -> Result<Uuid, String> {
        // Create temp directory if needed
        let cache_dir = dirs::cache_dir()
            .ok_or("Could not determine cache directory")?
            .join("ovim");
        std::fs::create_dir_all(&cache_dir)
            .map_err(|e| format!("Failed to create cache directory: {}", e))?;

        // Generate session ID and temp file
        let session_id = Uuid::new_v4();
        let temp_file = cache_dir.join(format!("edit_{}.txt", session_id));

        // Write text to temp file
        std::fs::write(&temp_file, &text)
            .map_err(|e| format!("Failed to write temp file: {}", e))?;

        // Get file modification time after writing
        let file_mtime = std::fs::metadata(&temp_file)
            .and_then(|m| m.modified())
            .map_err(|e| format!("Failed to get file mtime: {}", e))?;

        // Spawn terminal
        let SpawnInfo {
            terminal_type,
            process_id,
            child: _,
            window_title,
        } = spawn_terminal(&settings.terminal, &settings.nvim_path, &temp_file, geometry)?;

        // Create session
        let session = EditSession {
            id: session_id,
            focus_context,
            original_text: text,
            temp_file,
            file_mtime,
            terminal_type,
            process_id,
            window_title,
        };

        // Store session
        let mut sessions = self.sessions.lock().unwrap();
        sessions.insert(session_id, session);

        Ok(session_id)
    }

    /// Get a session by ID
    pub fn get_session(&self, id: &Uuid) -> Option<EditSession> {
        let sessions = self.sessions.lock().unwrap();
        sessions.get(id).map(|s| EditSession {
            id: s.id,
            focus_context: s.focus_context.clone(),
            original_text: s.original_text.clone(),
            temp_file: s.temp_file.clone(),
            file_mtime: s.file_mtime,
            terminal_type: s.terminal_type.clone(),
            process_id: s.process_id,
            window_title: s.window_title.clone(),
        })
    }

    /// Cancel a session (clean up without applying changes)
    pub fn cancel_session(&self, id: &Uuid) {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.remove(id) {
            // Clean up temp file
            let _ = std::fs::remove_file(&session.temp_file);
        }
    }

    /// Remove a session after completion
    pub fn remove_session(&self, id: &Uuid) {
        let mut sessions = self.sessions.lock().unwrap();
        sessions.remove(id);
    }

    /// Check if there are any active sessions
    #[allow(dead_code)]
    pub fn has_active_sessions(&self) -> bool {
        let sessions = self.sessions.lock().unwrap();
        !sessions.is_empty()
    }
}

impl Default for EditSessionManager {
    fn default() -> Self {
        Self::new()
    }
}
