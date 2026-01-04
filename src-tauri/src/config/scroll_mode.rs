use serde::{Deserialize, Serialize};

/// Settings for Scroll Mode feature (Vimium-style navigation)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScrollModeSettings {
    /// Enable scroll mode globally
    pub enabled: bool,
    /// Scroll amount in pixels for j/k keys
    pub scroll_step: u32,
    /// Bundle identifiers of apps where scroll mode is enabled
    pub enabled_apps: Vec<String>,
}

impl Default for ScrollModeSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            scroll_step: 100,
            enabled_apps: vec![
                // Browsers
                "com.apple.Safari".to_string(),
                "com.google.Chrome".to_string(),
                "org.mozilla.firefox".to_string(),
                "com.brave.Browser".to_string(),
                "company.thebrowser.Browser".to_string(), // Arc
                "com.microsoft.edgemac".to_string(),
                // System apps
                "com.apple.finder".to_string(),
                "com.apple.Preview".to_string(),
                "com.apple.Notes".to_string(),
                "com.apple.mail".to_string(),
            ],
        }
    }
}
