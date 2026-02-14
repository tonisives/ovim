use serde::{Deserialize, Serialize};

/// Settings for Scroll Mode feature (Vimium-style navigation)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScrollModeSettings {
    /// Enable scroll mode globally
    pub enabled: bool,
    /// Scroll amount in pixels for j/k keys
    pub scroll_step: u32,
    /// Enable list navigation mode (hjkl sends arrow keys instead of scroll)
    /// Useful for Finder, System Settings, and other list-based apps
    pub list_navigation: bool,
    /// Bundle identifiers of apps where scroll mode is enabled
    pub enabled_apps: Vec<String>,
    /// Bundle identifiers of apps where list navigation is enabled (hjkl = arrow keys)
    /// When empty, uses enabled_apps as fallback
    pub list_navigation_apps: Vec<String>,
    /// Bundle identifiers of apps that disable scroll mode when they have visible windows
    /// (e.g., overlay apps like Keyboard Maestro palettes)
    pub overlay_blocklist: Vec<String>,
    /// Shortcut groups that are disabled (e.g., "hjkl", "gg", "G", "du", "slash", "HL", "rR")
    pub disabled_shortcuts: Vec<String>,
}

impl Default for ScrollModeSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            scroll_step: 100,
            list_navigation: false,
            enabled_apps: vec![
                "com.apple.Safari".to_string(),
                "com.google.Chrome".to_string(),
                "org.mozilla.firefox".to_string(),
                "com.brave.Browser".to_string(),
                "company.thebrowser.Browser".to_string(), // Arc
                "com.microsoft.edgemac".to_string(),
            ],
            list_navigation_apps: vec![
                "com.apple.finder".to_string(),
                "com.apple.systempreferences".to_string(),
                "com.apple.SystemPreferences".to_string(),
            ],
            overlay_blocklist: vec![
                "com.stairways.keyboardmaestro.engine".to_string(), // KM palettes
                "com.raycast.macos".to_string(),                    // Raycast
                "com.alfredapp.Alfred".to_string(),                 // Alfred
                "com.runningwithcrayons.Alfred".to_string(),        // Alfred (alt bundle)
                "com.1password.1password".to_string(),              // 1Password
                "com.bitwarden.desktop".to_string(),                // Bitwarden
            ],
            disabled_shortcuts: vec![],
        }
    }
}
