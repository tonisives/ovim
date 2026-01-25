//! Edit Popup (NvimEdit) settings

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::click_mode::DoubleTapModifier;
use super::VimKeyModifiers;

/// Supported editor types for Edit Popup
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum EditorType {
    #[default]
    Neovim,
    Vim,
    Helix,
    Custom,
}

impl EditorType {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "neovim" | "nvim" => EditorType::Neovim,
            "vim" => EditorType::Vim,
            "helix" | "hx" => EditorType::Helix,
            _ => EditorType::Custom,
        }
    }

    /// Get the default executable name for this editor
    pub fn default_executable(&self) -> &'static str {
        match self {
            EditorType::Neovim => "nvim",
            EditorType::Vim => "vim",
            EditorType::Helix => "hx",
            EditorType::Custom => "",
        }
    }

    /// Get the process name to search for (may differ from executable)
    pub fn process_name(&self) -> &'static str {
        match self {
            EditorType::Neovim => "nvim",
            EditorType::Vim => "vim",
            EditorType::Helix => "hx",
            EditorType::Custom => "",
        }
    }

    /// Get the arguments to position cursor at end of file
    pub fn cursor_end_args(&self) -> Vec<&'static str> {
        match self {
            EditorType::Neovim | EditorType::Vim => vec!["+normal G$"],
            EditorType::Helix => vec![], // Helix doesn't have equivalent startup command
            EditorType::Custom => vec![],
        }
    }

    /// Get the arguments to position cursor at end of file and start in insert mode
    /// Used when the text field is empty
    pub fn cursor_end_args_insert(&self) -> Vec<&'static str> {
        match self {
            EditorType::Neovim | EditorType::Vim => vec!["+startinsert"],
            EditorType::Helix => vec![], // Helix doesn't have equivalent startup command
            EditorType::Custom => vec![],
        }
    }
}

/// Settings for Edit Popup feature
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NvimEditSettings {
    /// Enable the feature
    pub enabled: bool,
    /// Keyboard shortcut key (e.g., "e")
    pub shortcut_key: String,
    /// Shortcut modifiers
    pub shortcut_modifiers: VimKeyModifiers,
    /// Terminal to use: "alacritty", "iterm", "kitty", "wezterm", "ghostty", "default"
    pub terminal: String,
    /// Path to terminal executable (empty = auto-detect)
    /// Use this if the terminal is not found automatically
    #[serde(default)]
    pub terminal_path: String,
    /// Editor type: "neovim", "vim", "helix", or "custom"
    #[serde(default)]
    pub editor: EditorType,
    /// Path to editor executable (default: uses editor type's default)
    /// For backwards compatibility, this is still called nvim_path
    pub nvim_path: String,
    /// Position window below text field instead of fullscreen
    pub popup_mode: bool,
    /// Popup window width in pixels (0 = match text field width)
    pub popup_width: u32,
    /// Popup window height in pixels
    pub popup_height: u32,
    /// Enable live sync (BETA) - sync text field as you type in editor
    #[serde(default)]
    pub live_sync_enabled: bool,
    /// Use custom launcher script instead of built-in terminal spawning
    #[serde(default)]
    pub use_custom_script: bool,
    /// Use clipboard mode (Cmd+A, Cmd+C/V) instead of smart text field detection
    /// When true, always uses clipboard for text capture/restore
    /// When false (default), uses JavaScript for browsers and accessibility API for native apps
    #[serde(default)]
    pub clipboard_mode: bool,
    /// Double-tap modifier to activate edit mode (alternative to keyboard shortcut)
    #[serde(default)]
    pub double_tap_modifier: DoubleTapModifier,
    /// Saved filetypes per domain (browser hostname) or app bundle ID
    /// Stored in separate domain-filetypes.yaml file, not in main settings
    #[serde(skip)]
    pub domain_filetypes: HashMap<String, String>,
}

impl Default for NvimEditSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            shortcut_key: "".to_string(), // Disabled by default
            shortcut_modifiers: VimKeyModifiers {
                shift: false,
                control: false,
                option: false,
                command: false,
            },
            terminal: "alacritty".to_string(),
            terminal_path: "".to_string(), // Empty means auto-detect
            editor: EditorType::default(),
            nvim_path: "".to_string(), // Empty means use editor type's default
            popup_mode: true,
            popup_width: 0, // 0 = match text field width
            popup_height: 300,
            live_sync_enabled: true, // BETA feature, enabled by default
            use_custom_script: false,
            clipboard_mode: false, // Use smart detection by default
            double_tap_modifier: DoubleTapModifier::Command, // Cmd+Cmd by default
            domain_filetypes: HashMap::new(),
        }
    }
}

impl NvimEditSettings {
    /// Get the effective editor executable path
    pub fn editor_path(&self) -> String {
        if self.nvim_path.is_empty() {
            self.editor.default_executable().to_string()
        } else {
            self.nvim_path.clone()
        }
    }

    /// Get the effective terminal executable path
    /// Returns the user-specified path if set and matches terminal type,
    /// otherwise the terminal name for auto-detection
    pub fn get_terminal_path(&self) -> String {
        if self.terminal_path.is_empty() {
            return self.terminal.clone();
        }

        // Validate that the path matches the terminal type
        if self.terminal_path_matches_type() {
            self.terminal_path.clone()
        } else {
            // Path doesn't match terminal type, use auto-detection
            log::warn!(
                "Terminal path '{}' doesn't match terminal type '{}', using auto-detection",
                self.terminal_path,
                self.terminal
            );
            self.terminal.clone()
        }
    }

    /// Check if terminal_path matches the terminal type
    fn terminal_path_matches_type(&self) -> bool {
        let path_lower = self.terminal_path.to_lowercase();
        match self.terminal.as_str() {
            "alacritty" => path_lower.contains("alacritty"),
            "kitty" => path_lower.contains("kitty"),
            "wezterm" => path_lower.contains("wezterm"),
            "ghostty" => path_lower.contains("ghostty"),
            "iterm" => path_lower.contains("iterm"),
            "default" => path_lower.contains("terminal"),
            _ => true,
        }
    }

    /// Clean up any invalid state (e.g., mismatched paths)
    pub fn sanitize(&mut self) {
        // Check if terminal_path matches terminal type
        if !self.terminal_path.is_empty() && !self.terminal_path_matches_type() {
            log::warn!(
                "Clearing mismatched terminal_path '{}' for terminal type '{}'",
                self.terminal_path,
                self.terminal
            );
            self.terminal_path = String::new();
        }
    }

    /// Get the editor arguments for cursor positioning
    /// If text is empty, also start in insert mode
    pub fn editor_args(&self, text_is_empty: bool) -> Vec<&'static str> {
        if text_is_empty {
            self.editor.cursor_end_args_insert()
        } else {
            self.editor.cursor_end_args()
        }
    }

    /// Get the process name to search for when waiting for editor to exit
    pub fn editor_process_name(&self) -> &str {
        if self.nvim_path.is_empty() {
            self.editor.process_name()
        } else {
            // For custom paths, extract the binary name from the path
            std::path::Path::new(&self.nvim_path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
        }
    }

    /// Get the saved filetype for a domain/app
    pub fn get_filetype_for_domain(&self, domain: &str) -> Option<&str> {
        self.domain_filetypes.get(domain).map(|s| s.as_str())
    }

    /// Set the filetype for a domain/app and save to separate file for visibility
    pub fn set_filetype_for_domain(&mut self, domain: String, filetype: String) {
        self.domain_filetypes.insert(domain, filetype);
        // Also write to separate file for visibility
        self.save_domain_filetypes_file();
    }

    /// Remove the filetype for a domain/app
    pub fn remove_filetype_for_domain(&mut self, domain: &str) {
        self.domain_filetypes.remove(domain);
        self.save_domain_filetypes_file();
    }

    /// Get path to the domain filetypes file
    fn domain_filetypes_path() -> Option<std::path::PathBuf> {
        dirs::config_dir().map(|p| p.join("ovim").join("domain-filetypes.yaml"))
    }

    /// Load domain filetypes from the separate file
    pub fn load_domain_filetypes(&mut self) {
        if let Some(path) = Self::domain_filetypes_path() {
            if let Ok(contents) = std::fs::read_to_string(&path) {
                if let Ok(filetypes) = serde_yml::from_str(&contents) {
                    self.domain_filetypes = filetypes;
                    log::info!("Loaded {} domain filetypes", self.domain_filetypes.len());
                }
            }
        }
    }

    /// Save domain filetypes to the separate file
    fn save_domain_filetypes_file(&self) {
        if let Some(path) = Self::domain_filetypes_path() {
            if let Ok(contents) = serde_yml::to_string(&self.domain_filetypes) {
                let _ = std::fs::write(&path, contents);
            }
        }
    }

    /// Get all domain filetypes
    pub fn get_all_domain_filetypes(&self) -> &HashMap<String, String> {
        &self.domain_filetypes
    }
}
