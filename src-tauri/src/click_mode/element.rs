//! Clickable element representation for Click Mode

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::nvim_edit::accessibility::AXElementHandle;

/// Represents a clickable UI element discovered via Accessibility API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickableElement {
    /// Unique identifier for this element
    pub id: usize,
    /// The hint label (e.g., "A", "SD", "FG")
    pub hint: String,
    /// Element position x in screen coordinates
    pub x: f64,
    /// Element position y in screen coordinates
    pub y: f64,
    /// Element width
    pub width: f64,
    /// Element height
    pub height: f64,
    /// Element role (button, link, menuitem, etc.)
    pub role: String,
    /// Element title/label text
    pub title: String,
}

/// Internal element with AX handle (not serializable)
#[derive(Debug)]
pub struct ClickableElementInternal {
    /// The serializable element data
    pub element: ClickableElement,
    /// AX element reference for performing actions
    pub ax_element: AXElementHandle,
}

impl ClickableElementInternal {
    pub fn new(
        id: usize,
        hint: String,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        role: String,
        title: String,
        ax_element: AXElementHandle,
    ) -> Self {
        Self {
            element: ClickableElement {
                id,
                hint,
                x,
                y,
                width,
                height,
                role,
                title,
            },
            ax_element,
        }
    }

    /// Get the serializable element for sending to frontend
    pub fn to_serializable(&self) -> ClickableElement {
        self.element.clone()
    }
}
