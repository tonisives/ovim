use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Modifier keys for vim key activation
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct VimKeyModifiers {
    pub shift: bool,
    pub control: bool,
    pub option: bool,
    pub command: bool,
}

/// Settings for "Edit with Neovim" feature
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NvimEditSettings {
    /// Enable the feature
    pub enabled: bool,
    /// Keyboard shortcut key (e.g., "e")
    pub shortcut_key: String,
    /// Shortcut modifiers
    pub shortcut_modifiers: VimKeyModifiers,
    /// Terminal to use: "alacritty", "iterm", "kitty", "default"
    pub terminal: String,
    /// Path to nvim (default: "nvim" - uses PATH)
    pub nvim_path: String,
}

impl Default for NvimEditSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            shortcut_key: "e".to_string(),
            shortcut_modifiers: VimKeyModifiers {
                shift: true,
                control: false,
                option: false,
                command: true, // Cmd+Shift+E
            },
            terminal: "alacritty".to_string(),
            nvim_path: "nvim".to_string(),
        }
    }
}

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The key that toggles vim mode (keycode string)
    pub vim_key: String,
    /// Modifier keys required for vim key activation
    #[serde(default)]
    pub vim_key_modifiers: VimKeyModifiers,
    /// Indicator window position (0-5 for 2x3 grid)
    pub indicator_position: u8,
    /// Indicator opacity (0.0 - 1.0)
    pub indicator_opacity: f32,
    /// Indicator size scale (0.5 - 2.0)
    pub indicator_size: f32,
    /// Bundle identifiers of apps where vim mode is disabled
    pub ignored_apps: Vec<String>,
    /// Launch at login
    pub launch_at_login: bool,
    /// Show in menu bar
    pub show_in_menu_bar: bool,
    /// Top widget type
    pub top_widget: String,
    /// Bottom widget type
    pub bottom_widget: String,
    /// Bundle identifiers of Electron apps for selection observing
    pub electron_apps: Vec<String>,
    /// Settings for "Edit with Neovim" feature
    pub nvim_edit: NvimEditSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            vim_key: "caps_lock".to_string(),
            vim_key_modifiers: VimKeyModifiers::default(),
            indicator_position: 1, // Top center
            indicator_opacity: 0.9,
            indicator_size: 1.0,
            ignored_apps: vec![],
            launch_at_login: false,
            show_in_menu_bar: true,
            top_widget: "None".to_string(),
            bottom_widget: "None".to_string(),
            electron_apps: vec![],
            nvim_edit: NvimEditSettings::default(),
        }
    }
}

impl Settings {
    /// Get the path to the settings file
    pub fn file_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("ovim").join("settings.json"))
    }

    /// Load settings from disk
    pub fn load() -> Self {
        if let Some(path) = Self::file_path() {
            if let Ok(contents) = std::fs::read_to_string(&path) {
                if let Ok(settings) = serde_json::from_str(&contents) {
                    return settings;
                }
            }
        }
        Self::default()
    }

    /// Save settings to disk
    pub fn save(&self) -> Result<(), String> {
        let path = Self::file_path().ok_or("Could not determine config directory")?;

        // Create directory if it doesn't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        let contents =
            serde_json::to_string_pretty(self).map_err(|e| format!("Failed to serialize: {}", e))?;

        std::fs::write(&path, contents).map_err(|e| format!("Failed to write settings: {}", e))
    }
}
