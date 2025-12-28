//! Color types for mode colors

use serde::{Deserialize, Serialize};

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
