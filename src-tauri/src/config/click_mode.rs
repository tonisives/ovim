//! Click Mode settings
//!
//! Configuration for the keyboard-driven element clicking feature.

use serde::{Deserialize, Serialize};

use super::VimKeyModifiers;

/// Settings for Click Mode feature
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ClickModeSettings {
    /// Enable the feature
    pub enabled: bool,
    /// Keyboard shortcut key (e.g., "f")
    pub shortcut_key: String,
    /// Shortcut modifiers (default: Cmd+Shift)
    pub shortcut_modifiers: VimKeyModifiers,
    /// Characters to use for hint labels (home row first for speed)
    pub hint_chars: String,
    /// Show search bar when click mode is activated
    pub show_search_bar: bool,
    /// Opacity of hint labels (0.0-1.0)
    pub hint_opacity: f32,
    /// Hint label font size
    pub hint_font_size: u32,
    /// Hint label background color (hex)
    pub hint_bg_color: String,
    /// Hint label text color (hex)
    pub hint_text_color: String,

    // Advanced timing settings
    /// Delay before querying accessibility elements (ms).
    /// Increase if hints are missing on slower computers.
    #[serde(default = "default_ax_delay")]
    pub ax_stabilization_delay_ms: u32,
    /// How long to cache elements (ms). Increase for faster repeat activations.
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl_ms: u32,

    // Advanced traversal settings
    /// Maximum depth to traverse in the accessibility tree.
    /// Increase for apps with deeply nested elements (e.g., Electron apps like Slack).
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,
    /// Maximum number of elements to collect.
    /// Increase if hints are missing in apps with many elements.
    #[serde(default = "default_max_elements")]
    pub max_elements: u32,
}

fn default_ax_delay() -> u32 {
    10
}

fn default_cache_ttl() -> u32 {
    500
}

fn default_max_depth() -> u32 {
    10
}

fn default_max_elements() -> u32 {
    500
}

impl Default for ClickModeSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            shortcut_key: "f".to_string(),
            shortcut_modifiers: VimKeyModifiers {
                shift: true,
                control: false,
                option: false,
                command: true, // Cmd+Shift+F
            },
            hint_chars: "asfghjklqwetyuiopzxvbm".to_string(), // excludes r, c, d, n (action keys)
            show_search_bar: true,
            hint_opacity: 0.95,
            hint_font_size: 12,
            hint_bg_color: "#FFCC00".to_string(), // Yellow background like Vimium
            hint_text_color: "#000000".to_string(), // Black text
            ax_stabilization_delay_ms: default_ax_delay(),
            cache_ttl_ms: default_cache_ttl(),
            max_depth: default_max_depth(),
            max_elements: default_max_elements(),
        }
    }
}

impl ClickModeSettings {
    /// Check if the shortcut matches the given key and modifiers
    pub fn matches_shortcut(
        &self,
        key: &str,
        shift: bool,
        control: bool,
        option: bool,
        command: bool,
    ) -> bool {
        if !self.enabled {
            return false;
        }

        let key_matches = self.shortcut_key.eq_ignore_ascii_case(key);
        let mods_match = self.shortcut_modifiers.shift == shift
            && self.shortcut_modifiers.control == control
            && self.shortcut_modifiers.option == option
            && self.shortcut_modifiers.command == command;

        key_matches && mods_match
    }
}
