use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::colors::ModeColors;
use super::nvim_edit::NvimEditSettings;

/// Modifier keys for vim key activation
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct VimKeyModifiers {
    pub shift: bool,
    pub control: bool,
    pub option: bool,
    pub command: bool,
}

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Enable vim mode and indicator
    #[serde(default = "default_enabled")]
    pub enabled: bool,
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
    /// Indicator X offset in pixels
    #[serde(default)]
    pub indicator_offset_x: i32,
    /// Indicator Y offset in pixels
    #[serde(default)]
    pub indicator_offset_y: i32,
    /// Whether the indicator window is visible
    #[serde(default = "default_true")]
    pub indicator_visible: bool,
    /// Show mode indicator in menu bar icon
    #[serde(default)]
    pub show_mode_in_menu_bar: bool,
    /// Mode-specific background colors
    #[serde(default)]
    pub mode_colors: ModeColors,
    /// Font family for indicator
    #[serde(default = "default_font_family")]
    pub indicator_font: String,
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
    /// Settings for Edit Popup feature
    pub nvim_edit: NvimEditSettings,
}

fn default_font_family() -> String {
    "system-ui, -apple-system, sans-serif".to_string()
}

fn default_enabled() -> bool {
    true
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            vim_key: "caps_lock".to_string(),
            vim_key_modifiers: VimKeyModifiers::default(),
            indicator_position: 1, // Top center
            indicator_opacity: 0.9,
            indicator_size: 1.0,
            indicator_offset_x: 0,
            indicator_offset_y: 0,
            indicator_visible: true,
            show_mode_in_menu_bar: false,
            mode_colors: ModeColors::default(),
            indicator_font: default_font_family(),
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
    /// Get the path to the YAML settings file
    pub fn file_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("ovim").join("settings.yaml"))
    }

    /// Get the path to the terminal launcher script
    /// Uses the same directory as other settings (~/Library/Application Support/ovim/)
    pub fn launcher_script_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("ovim").join("terminal-launcher.sh"))
    }

    /// Get the path to the legacy JSON settings file
    fn legacy_json_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("ovim").join("settings.json"))
    }

    /// Load settings from disk (YAML format, with JSON migration)
    pub fn load() -> Self {
        let mut settings = Self::load_raw();
        // Sanitize settings to fix any invalid state
        settings.nvim_edit.sanitize();
        settings
    }

    /// Load raw settings without sanitization
    fn load_raw() -> Self {
        // First, try to load from YAML
        if let Some(yaml_path) = Self::file_path() {
            if let Ok(contents) = std::fs::read_to_string(&yaml_path) {
                if let Ok(settings) = serde_yml::from_str(&contents) {
                    return settings;
                }
            }
        }

        // If no YAML exists, try to migrate from JSON
        if let Some(json_path) = Self::legacy_json_path() {
            if let Ok(contents) = std::fs::read_to_string(&json_path) {
                if let Ok(settings) = serde_json::from_str::<Settings>(&contents) {
                    // Save as YAML and delete the old JSON file
                    if settings.save().is_ok() {
                        let _ = std::fs::remove_file(&json_path);
                        log::info!("Migrated settings from JSON to YAML format");
                    }
                    return settings;
                }
            }
        }

        Self::default()
    }

    /// Save settings to disk (YAML format)
    pub fn save(&self) -> Result<(), String> {
        let path = Self::file_path().ok_or("Could not determine config directory")?;

        // Create directory if it doesn't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        let contents =
            serde_yml::to_string(self).map_err(|e| format!("Failed to serialize: {}", e))?;

        std::fs::write(&path, contents).map_err(|e| format!("Failed to write settings: {}", e))
    }
}
