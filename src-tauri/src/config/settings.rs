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
    /// Position window below text field instead of fullscreen
    pub popup_mode: bool,
    /// Popup window width in pixels (0 = match text field width)
    pub popup_width: u32,
    /// Popup window height in pixels
    pub popup_height: u32,
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
            popup_mode: true,
            popup_width: 0, // 0 = match text field width
            popup_height: 300,
        }
    }
}

/// RGB color representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Default for RgbColor {
    fn default() -> Self {
        Self { r: 128, g: 128, b: 128 }
    }
}

/// Mode-specific color settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ModeColors {
    /// Insert mode background color
    pub insert: RgbColor,
    /// Normal mode background color
    pub normal: RgbColor,
    /// Visual mode background color
    pub visual: RgbColor,
}

impl Default for ModeColors {
    fn default() -> Self {
        Self {
            insert: RgbColor { r: 74, g: 144, b: 217 },   // Blue
            normal: RgbColor { r: 232, g: 148, b: 74 },   // Orange
            visual: RgbColor { r: 155, g: 109, b: 215 },  // Purple
        }
    }
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
    /// Settings for "Edit with Neovim" feature
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
