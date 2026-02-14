//! List Mode - Vim-style hjkl navigation for list views
//!
//! This module provides keyboard-driven list navigation for apps like Finder,
//! System Settings, and other list-based interfaces. Unlike scroll mode which
//! scrolls the page, list mode sends arrow keys for item selection.

use std::sync::{Arc, Mutex};

use crate::keyboard::{self, KeyCode};

/// State for list mode processing
#[derive(Debug, Default)]
pub struct ListModeState {
    /// Pending g key for gg command (go to top)
    pending_g: bool,
}

/// Result of processing a list mode key
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ListResult {
    /// Key was handled by list mode (suppress it)
    Handled,
    /// Key is not a list command (pass through)
    PassThrough,
}

impl ListModeState {
    /// Create a new list mode state
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset any pending state
    pub fn reset(&mut self) {
        self.pending_g = false;
    }

    /// Process a key press in list mode
    ///
    /// Returns whether the key was handled or should pass through.
    /// Keys with modifiers (except Shift for selection) are passed through.
    pub fn process_key(
        &mut self,
        keycode: KeyCode,
        shift: bool,
        control: bool,
        option: bool,
        command: bool,
    ) -> ListResult {
        // If any modifier besides shift is pressed, pass through
        // (We need shift for selection and G)
        if control || option || command {
            self.reset();
            return ListResult::PassThrough;
        }

        // Handle pending g (for gg command - go to top)
        if self.pending_g {
            self.pending_g = false;
            if keycode == KeyCode::G && !shift {
                // gg - go to top of list (Home key)
                if let Err(e) = keyboard::list_go_top() {
                    log::error!("Failed to go to top: {}", e);
                }
                return ListResult::Handled;
            }
            // g followed by something else - pass through both
            return ListResult::PassThrough;
        }

        match keycode {
            // h - left arrow (or collapse in tree views)
            KeyCode::H if !shift => {
                if let Err(e) = keyboard::list_left() {
                    log::error!("Failed to move left: {}", e);
                }
                ListResult::Handled
            }

            // j - down arrow (select next item)
            KeyCode::J if !shift => {
                if let Err(e) = keyboard::list_down() {
                    log::error!("Failed to move down: {}", e);
                }
                ListResult::Handled
            }

            // k - up arrow (select previous item)
            KeyCode::K if !shift => {
                if let Err(e) = keyboard::list_up() {
                    log::error!("Failed to move up: {}", e);
                }
                ListResult::Handled
            }

            // l - right arrow (or expand in tree views)
            KeyCode::L if !shift => {
                if let Err(e) = keyboard::list_right() {
                    log::error!("Failed to move right: {}", e);
                }
                ListResult::Handled
            }

            // J (shift+j) - extend selection down
            KeyCode::J if shift => {
                if let Err(e) = keyboard::list_select_down() {
                    log::error!("Failed to extend selection down: {}", e);
                }
                ListResult::Handled
            }

            // K (shift+k) - extend selection up
            KeyCode::K if shift => {
                if let Err(e) = keyboard::list_select_up() {
                    log::error!("Failed to extend selection up: {}", e);
                }
                ListResult::Handled
            }

            // G (shift+g) - go to bottom of list (End key)
            KeyCode::G if shift => {
                if let Err(e) = keyboard::list_go_bottom() {
                    log::error!("Failed to go to bottom: {}", e);
                }
                ListResult::Handled
            }

            // g - start gg sequence (go to top)
            KeyCode::G if !shift => {
                self.pending_g = true;
                ListResult::Handled
            }

            // H (shift+h) - go back (Cmd+[)
            KeyCode::H if shift => {
                if let Err(e) = keyboard::history_back() {
                    log::error!("Failed to go back: {}", e);
                }
                ListResult::Handled
            }

            // L (shift+l) - go forward (Cmd+])
            KeyCode::L if shift => {
                if let Err(e) = keyboard::history_forward() {
                    log::error!("Failed to go forward: {}", e);
                }
                ListResult::Handled
            }

            // o - open item (Return key)
            KeyCode::O if !shift => {
                if let Err(e) = keyboard::inject_return() {
                    log::error!("Failed to open item: {}", e);
                }
                ListResult::Handled
            }

            // / - open search (Cmd+F)
            KeyCode::Slash if !shift => {
                if let Err(e) = keyboard::open_find() {
                    log::error!("Failed to open search: {}", e);
                }
                ListResult::Handled
            }

            // Any other key - pass through
            _ => ListResult::PassThrough,
        }
    }
}

/// Shared list mode state
pub type SharedListModeState = Arc<Mutex<ListModeState>>;

/// Create a new shared list mode state
pub fn create_list_state() -> SharedListModeState {
    Arc::new(Mutex::new(ListModeState::new()))
}
