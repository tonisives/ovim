//! Scroll Mode - Vimium-style keyboard navigation for scrolling pages
//!
//! This module provides keyboard-driven scrolling similar to Vimium browser extension.
//! Unlike vim mode, scroll mode is always active when enabled (no toggle needed).

use std::sync::{Arc, Mutex};

use crate::keyboard::{self, KeyCode};

/// State for scroll mode processing
#[derive(Debug, Default)]
pub struct ScrollModeState {
    /// Pending g key for gg command (scroll to top)
    pending_g: bool,
}

/// Result of processing a scroll mode key
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScrollResult {
    /// Key was handled by scroll mode (suppress it)
    Handled,
    /// Key is not a scroll command (pass through)
    PassThrough,
}

impl ScrollModeState {
    /// Create a new scroll mode state
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset any pending state
    pub fn reset(&mut self) {
        self.pending_g = false;
    }

    /// Process a key press in scroll mode
    ///
    /// Returns whether the key was handled or should pass through.
    /// Keys with modifiers (except Shift for G and R) are passed through.
    pub fn process_key(
        &mut self,
        keycode: KeyCode,
        shift: bool,
        control: bool,
        option: bool,
        command: bool,
        scroll_step: u32,
    ) -> ScrollResult {
        // If any modifier besides shift is pressed, pass through
        // (We need shift for G and R)
        if control || option || command {
            self.reset();
            return ScrollResult::PassThrough;
        }

        // Handle pending g (for gg command)
        if self.pending_g {
            self.pending_g = false;
            if keycode == KeyCode::G && !shift {
                // gg - scroll to top
                if let Err(e) = keyboard::scroll_to_top() {
                    log::error!("Failed to scroll to top: {}", e);
                }
                return ScrollResult::Handled;
            }
            // g followed by something else - pass through both
            return ScrollResult::PassThrough;
        }

        match keycode {
            // h - scroll left
            KeyCode::H if !shift => {
                if let Err(e) = keyboard::scroll_left(scroll_step) {
                    log::error!("Failed to scroll left: {}", e);
                }
                ScrollResult::Handled
            }

            // j - scroll down
            KeyCode::J if !shift => {
                if let Err(e) = keyboard::scroll_down(scroll_step) {
                    log::error!("Failed to scroll down: {}", e);
                }
                ScrollResult::Handled
            }

            // k - scroll up
            KeyCode::K if !shift => {
                if let Err(e) = keyboard::scroll_up(scroll_step) {
                    log::error!("Failed to scroll up: {}", e);
                }
                ScrollResult::Handled
            }

            // l - scroll right
            KeyCode::L if !shift => {
                if let Err(e) = keyboard::scroll_right(scroll_step) {
                    log::error!("Failed to scroll right: {}", e);
                }
                ScrollResult::Handled
            }

            // G (shift+g) - scroll to bottom
            KeyCode::G if shift => {
                if let Err(e) = keyboard::scroll_to_bottom() {
                    log::error!("Failed to scroll to bottom: {}", e);
                }
                ScrollResult::Handled
            }

            // g - start gg sequence (scroll to top)
            KeyCode::G if !shift => {
                self.pending_g = true;
                ScrollResult::Handled
            }

            // d - half page down
            KeyCode::D if !shift => {
                if let Err(e) = keyboard::half_page_scroll_down() {
                    log::error!("Failed to half page down: {}", e);
                }
                ScrollResult::Handled
            }

            // u - half page up
            KeyCode::U if !shift => {
                if let Err(e) = keyboard::half_page_scroll_up() {
                    log::error!("Failed to half page up: {}", e);
                }
                ScrollResult::Handled
            }

            // / - open find (Cmd+F)
            KeyCode::Slash if !shift => {
                if let Err(e) = keyboard::open_find() {
                    log::error!("Failed to open find: {}", e);
                }
                ScrollResult::Handled
            }

            // H (shift+h) - history back
            KeyCode::H if shift => {
                if let Err(e) = keyboard::history_back() {
                    log::error!("Failed to go back in history: {}", e);
                }
                ScrollResult::Handled
            }

            // L (shift+l) - history forward
            KeyCode::L if shift => {
                if let Err(e) = keyboard::history_forward() {
                    log::error!("Failed to go forward in history: {}", e);
                }
                ScrollResult::Handled
            }

            // r - reload
            KeyCode::R if !shift => {
                if let Err(e) = keyboard::reload_page(false) {
                    log::error!("Failed to reload: {}", e);
                }
                ScrollResult::Handled
            }

            // R (shift+r) - hard reload
            KeyCode::R if shift => {
                if let Err(e) = keyboard::reload_page(true) {
                    log::error!("Failed to hard reload: {}", e);
                }
                ScrollResult::Handled
            }

            // Any other key - pass through
            _ => ScrollResult::PassThrough,
        }
    }
}

/// Shared scroll mode state
pub type SharedScrollModeState = Arc<Mutex<ScrollModeState>>;

/// Create a new shared scroll mode state
pub fn create_scroll_state() -> SharedScrollModeState {
    Arc::new(Mutex::new(ScrollModeState::new()))
}
