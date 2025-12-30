//! Data structures for accessibility helper

use serde::{Deserialize, Serialize};

/// Window bounds for filtering elements
#[derive(Debug, Clone, Copy)]
pub struct WindowBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl WindowBounds {
    pub fn contains(&self, elem_x: f64, elem_y: f64, elem_w: f64, elem_h: f64) -> bool {
        // Check if element is at least partially within window bounds
        // Element must have some overlap with the window
        let elem_right = elem_x + elem_w;
        let elem_bottom = elem_y + elem_h;
        let win_right = self.x + self.width;
        let win_bottom = self.y + self.height;

        // Element is visible if it overlaps with window
        elem_x < win_right && elem_right > self.x && elem_y < win_bottom && elem_bottom > self.y
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawElement {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub role: String,
    pub title: String,
}

/// Output from the helper, including metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelperOutput {
    pub elements: Vec<RawElement>,
    /// True if elements were collected from a sheet/dialog (modal UI)
    pub is_modal: bool,
}
