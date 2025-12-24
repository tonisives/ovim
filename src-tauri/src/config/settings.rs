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
            terminal_path: "".to_string(), // Empty means auto-detect
            editor: EditorType::default(),
            nvim_path: "".to_string(), // Empty means use editor type's default
            popup_mode: true,
            popup_width: 0, // 0 = match text field width
            popup_height: 300,
            live_sync_enabled: true, // BETA feature, enabled by default
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
    pub fn editor_args(&self) -> Vec<&str> {
        self.editor.cursor_end_args()
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
