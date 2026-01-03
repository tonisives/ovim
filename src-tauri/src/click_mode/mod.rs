//! Click Mode - System-wide keyboard-driven element clicking
//!
//! This module provides Vimium-style hint labels for clicking UI elements
//! using the keyboard instead of the mouse.

pub mod accessibility;
pub mod browser_clickables;
pub mod element;
pub mod hints;
pub mod mouse;
pub mod native_hints;

use std::sync::{Arc, Mutex};

pub use element::{ClickableElement, ClickableElementInternal};

use serde::{Deserialize, Serialize};

/// The type of click action to perform
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq)]
pub enum ClickAction {
    /// Normal left-click
    #[default]
    Click,
    /// Right-click (context menu)
    RightClick,
    /// Cmd+click
    CmdClick,
    /// Double-click
    DoubleClick,
}

impl ClickAction {
    /// Get display name for the action
    pub fn display_name(&self) -> &'static str {
        match self {
            ClickAction::Click => "click",
            ClickAction::RightClick => "right",
            ClickAction::CmdClick => "cmd",
            ClickAction::DoubleClick => "double",
        }
    }
}

/// Click mode state machine
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClickModeState {
    /// Click mode is inactive
    Inactive,
    /// Hints are displayed, waiting for user input
    ShowingHints {
        /// Current input buffer (typed hint characters)
        input_buffer: String,
        /// Number of elements being shown
        element_count: usize,
        /// The type of click action to perform
        click_action: ClickAction,
        /// Whether the user made a wrong second keystroke (allows one retry)
        wrong_second_key: bool,
    },
    /// Search mode is active (fuzzy search by element text)
    Searching {
        /// Current search query
        query: String,
        /// Number of matching elements
        match_count: usize,
        /// The type of click action to perform
        click_action: ClickAction,
    },
}

impl Default for ClickModeState {
    fn default() -> Self {
        Self::Inactive
    }
}

impl ClickModeState {
    /// Check if click mode is currently active
    pub fn is_active(&self) -> bool {
        !matches!(self, ClickModeState::Inactive)
    }

    /// Check if we're in hint selection mode
    pub fn is_showing_hints(&self) -> bool {
        matches!(self, ClickModeState::ShowingHints { .. })
    }

    /// Check if we're in search mode
    pub fn is_searching(&self) -> bool {
        matches!(self, ClickModeState::Searching { .. })
    }

    /// Get the current input buffer (hint chars or search query)
    pub fn input(&self) -> &str {
        match self {
            ClickModeState::Inactive => "",
            ClickModeState::ShowingHints { input_buffer, .. } => input_buffer,
            ClickModeState::Searching { query, .. } => query,
        }
    }

    /// Get the current click action
    pub fn click_action(&self) -> ClickAction {
        match self {
            ClickModeState::Inactive => ClickAction::Click,
            ClickModeState::ShowingHints { click_action, .. } => *click_action,
            ClickModeState::Searching { click_action, .. } => *click_action,
        }
    }
}

/// Result of hint input handling
#[derive(Debug, Clone)]
pub enum HintInputResult {
    /// Exact match found - perform click
    Match(ClickableElement),
    /// Partial match - continue waiting for more input
    Partial,
    /// Wrong second key - show shake animation, allow one retry
    WrongSecondKey,
    /// No match at all - deactivate
    NoMatch,
}

/// Manager for click mode state and elements
pub struct ClickModeManager {
    /// Current state
    state: ClickModeState,
    /// Currently discovered elements (with AX handles)
    elements: Vec<ClickableElementInternal>,
    /// Current click action type
    click_action: ClickAction,
}

impl ClickModeManager {
    pub fn new() -> Self {
        Self {
            state: ClickModeState::Inactive,
            elements: Vec::new(),
            click_action: ClickAction::Click,
        }
    }

    /// Get current state
    pub fn state(&self) -> &ClickModeState {
        &self.state
    }

    /// Check if active
    pub fn is_active(&self) -> bool {
        self.state.is_active()
    }

    /// Set click mode to "activating" state immediately
    /// This ensures keys are captured while elements are being queried
    pub fn set_activating(&mut self) {
        log::info!("Click mode: set to activating state");
        self.click_action = ClickAction::Click; // Reset to default
        self.state = ClickModeState::ShowingHints {
            input_buffer: String::new(),
            element_count: 0,
            click_action: self.click_action,
            wrong_second_key: false,
        };
    }

    /// Activate click mode and query elements
    ///
    /// Returns the elements for display in the overlay
    pub fn activate(&mut self) -> Result<Vec<ClickableElement>, String> {
        log::info!("Activating click mode");

        // Query clickable elements from the frontmost app
        let internal_elements = accessibility::get_clickable_elements()?;

        if internal_elements.is_empty() {
            log::warn!("No clickable elements found");
            self.state = ClickModeState::Inactive;
            return Err("No clickable elements found".to_string());
        }

        log::info!("Found {} clickable elements", internal_elements.len());

        // Convert to serializable elements for frontend
        let elements: Vec<ClickableElement> = internal_elements
            .iter()
            .map(|e| e.to_serializable())
            .collect();

        let element_count = elements.len();

        // Store internal elements and update state
        self.elements = internal_elements;
        self.state = ClickModeState::ShowingHints {
            input_buffer: String::new(),
            element_count,
            click_action: self.click_action,
            wrong_second_key: false,
        };

        Ok(elements)
    }

    /// Deactivate click mode
    pub fn deactivate(&mut self) {
        log::info!("Deactivating click mode");
        self.state = ClickModeState::Inactive;
        self.elements.clear();
        self.click_action = ClickAction::Click;
    }

    /// Handle a character input in hint mode
    ///
    /// Returns:
    /// - `HintInputResult::Match(element)` if a hint was matched (perform click)
    /// - `HintInputResult::Partial` if input was accepted but no match yet
    /// - `HintInputResult::WrongSecondKey` if user typed wrong second character (allow retry)
    /// - `HintInputResult::NoMatch` if input doesn't match any hints
    pub fn handle_hint_input(&mut self, c: char) -> HintInputResult {
        let (current_input, was_wrong_second_key) = match &self.state {
            ClickModeState::ShowingHints { input_buffer, wrong_second_key, .. } => {
                (input_buffer.clone(), *wrong_second_key)
            }
            _ => return HintInputResult::NoMatch,
        };

        let new_input = format!("{}{}", current_input, c.to_uppercase());

        // Check for matches
        let matching: Vec<usize> = self
            .elements
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                hints::match_hint(&e.element.hint, &new_input).map(|exact| (i, exact))
            })
            .filter_map(|(i, exact)| if exact { Some(i) } else { None })
            .collect();

        // Exact match found
        if matching.len() == 1 {
            let element = self.elements[matching[0]].to_serializable();
            return HintInputResult::Match(element);
        }

        // Check for partial matches
        let partial_matches: Vec<usize> = self
            .elements
            .iter()
            .enumerate()
            .filter_map(|(i, e)| hints::match_hint(&e.element.hint, &new_input).map(|_| i))
            .collect();

        if !partial_matches.is_empty() {
            // Update input buffer
            self.state = ClickModeState::ShowingHints {
                input_buffer: new_input,
                element_count: self.elements.len(),
                click_action: self.click_action,
                wrong_second_key: false,
            };
            return HintInputResult::Partial;
        }

        // No partial matches - check if this is second character and allow retry
        // Only allow retry if: first char was correct AND this is the second char AND haven't retried yet
        if current_input.len() == 1 && !was_wrong_second_key {
            // Check if the first character still has partial matches
            let first_char_matches: Vec<usize> = self
                .elements
                .iter()
                .enumerate()
                .filter_map(|(i, e)| hints::match_hint(&e.element.hint, &current_input).map(|_| i))
                .collect();

            if !first_char_matches.is_empty() {
                // First char was correct, second was wrong - allow one retry
                self.state = ClickModeState::ShowingHints {
                    input_buffer: current_input, // Keep first char
                    element_count: self.elements.len(),
                    click_action: self.click_action,
                    wrong_second_key: true, // Mark that we had a wrong second key
                };
                return HintInputResult::WrongSecondKey;
            }
        }

        HintInputResult::NoMatch
    }

    /// Get the center position of an element by ID
    pub fn get_element_position(&self, element_id: usize) -> Option<(f64, f64)> {
        self.elements
            .iter()
            .find(|e| e.element.id == element_id)
            .map(|e| e.center())
    }

    /// Get a clone of the AXElementHandle for an element by ID (if available)
    pub fn get_ax_element(&self, element_id: usize) -> Option<crate::nvim_edit::accessibility::AXElementHandle> {
        self.elements
            .iter()
            .find(|e| e.element.id == element_id)
            .and_then(|e| e.ax_element.clone())
    }

    /// Perform click on element by ID
    pub fn click_element(&self, element_id: usize) -> Result<(), String> {
        let element = self
            .elements
            .iter()
            .find(|e| e.element.id == element_id)
            .ok_or_else(|| format!("Element {} not found", element_id))?;

        // Use position-based click (works for both subprocess and direct modes)
        let (x, y) = element.center();
        accessibility::perform_click_at_position(x, y)
    }

    /// Perform right-click on element by ID
    pub fn right_click_element(&self, element_id: usize) -> Result<(), String> {
        let element = self
            .elements
            .iter()
            .find(|e| e.element.id == element_id)
            .ok_or_else(|| format!("Element {} not found", element_id))?;

        // Use position-based right-click
        let (x, y) = element.center();
        accessibility::perform_right_click_at_position(x, y)
    }

    /// Enter search mode
    pub fn enter_search_mode(&mut self) {
        if !self.is_active() {
            return;
        }
        self.state = ClickModeState::Searching {
            query: String::new(),
            match_count: self.elements.len(),
            click_action: self.click_action,
        };
    }

    /// Handle search input
    pub fn handle_search_input(&mut self, query: &str) -> Vec<ClickableElement> {
        let query_lower = query.to_lowercase();

        let matching: Vec<ClickableElement> = self
            .elements
            .iter()
            .filter(|e| {
                e.element.title.to_lowercase().contains(&query_lower)
                    || e.element.role.to_lowercase().contains(&query_lower)
            })
            .map(|e| e.to_serializable())
            .collect();

        self.state = ClickModeState::Searching {
            query: query.to_string(),
            match_count: matching.len(),
            click_action: self.click_action,
        };

        matching
    }

    /// Clear input buffer (backspace)
    pub fn clear_last_input(&mut self) {
        match &mut self.state {
            ClickModeState::ShowingHints { input_buffer, .. } => {
                input_buffer.pop();
            }
            ClickModeState::Searching { query, .. } => {
                query.pop();
            }
            _ => {}
        }
    }

    /// Get elements matching current input
    pub fn get_filtered_elements(&self) -> Vec<ClickableElement> {
        match &self.state {
            ClickModeState::Inactive => Vec::new(),
            ClickModeState::ShowingHints { input_buffer, .. } => {
                if input_buffer.is_empty() {
                    self.elements.iter().map(|e| e.to_serializable()).collect()
                } else {
                    self.elements
                        .iter()
                        .filter(|e| hints::match_hint(&e.element.hint, input_buffer).is_some())
                        .map(|e| e.to_serializable())
                        .collect()
                }
            }
            ClickModeState::Searching { query, .. } => {
                let query_lower = query.to_lowercase();
                self.elements
                    .iter()
                    .filter(|e| {
                        e.element.title.to_lowercase().contains(&query_lower)
                            || e.element.role.to_lowercase().contains(&query_lower)
                    })
                    .map(|e| e.to_serializable())
                    .collect()
            }
        }
    }

    /// Get all elements (for filtering native hints)
    pub fn get_all_elements(&self) -> Vec<ClickableElement> {
        self.elements.iter().map(|e| e.to_serializable()).collect()
    }

    /// Get current input buffer
    pub fn get_current_input(&self) -> String {
        match &self.state {
            ClickModeState::ShowingHints { input_buffer, .. } => input_buffer.clone(),
            ClickModeState::Searching { query, .. } => query.clone(),
            ClickModeState::Inactive => String::new(),
        }
    }

    /// Get the current click action
    pub fn get_click_action(&self) -> ClickAction {
        self.click_action
    }

    /// Set the click action type
    pub fn set_click_action(&mut self, action: ClickAction) {
        log::info!("Click mode: setting action to {:?}", action);
        self.click_action = action;

        // Update the state to reflect the new action
        match &self.state {
            ClickModeState::ShowingHints { input_buffer, element_count, wrong_second_key, .. } => {
                self.state = ClickModeState::ShowingHints {
                    input_buffer: input_buffer.clone(),
                    element_count: *element_count,
                    click_action: action,
                    wrong_second_key: *wrong_second_key,
                };
            }
            ClickModeState::Searching { query, match_count, .. } => {
                self.state = ClickModeState::Searching {
                    query: query.clone(),
                    match_count: *match_count,
                    click_action: action,
                };
            }
            ClickModeState::Inactive => {}
        }
    }
}

impl Default for ClickModeManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe wrapper for ClickModeManager
pub type SharedClickModeManager = Arc<Mutex<ClickModeManager>>;

/// Create a new shared click mode manager
pub fn create_manager() -> SharedClickModeManager {
    Arc::new(Mutex::new(ClickModeManager::new()))
}

/// Start observing app focus changes
/// When the frontmost app changes, the callback will be called
pub fn start_focus_observer<F>(callback: F)
where
    F: Fn() + Send + Sync + 'static,
{
    use dispatch::Queue;
    use objc::{class, msg_send, sel, sel_impl};
    use std::sync::Arc;

    let callback = Arc::new(callback);

    // Dispatch to main thread to set up the observer
    Queue::main().exec_async(move || {
        unsafe {
            // Get NSWorkspace shared instance
            let workspace: *mut objc::runtime::Object =
                msg_send![class!(NSWorkspace), sharedWorkspace];
            if workspace.is_null() {
                log::error!("Failed to get NSWorkspace");
                return;
            }

            // Get the notification center
            let notification_center: *mut objc::runtime::Object =
                msg_send![workspace, notificationCenter];
            if notification_center.is_null() {
                log::error!("Failed to get notification center");
                return;
            }

            // Create block for the observer
            // We use a simple approach: observe didActivateApplicationNotification
            let callback_clone = Arc::clone(&callback);
            let block = block::ConcreteBlock::new(move |_notification: *mut objc::runtime::Object| {
                log::debug!("App focus changed - hiding click mode hints");
                callback_clone();
            });
            let block = block.copy();

            // Get the notification name
            let notification_name: *mut objc::runtime::Object = msg_send![
                class!(NSString),
                stringWithUTF8String: b"NSWorkspaceDidActivateApplicationNotification\0".as_ptr()
            ];

            // Add observer using the block-based API
            let _: *mut objc::runtime::Object = msg_send![
                notification_center,
                addObserverForName: notification_name
                object: std::ptr::null::<objc::runtime::Object>()
                queue: std::ptr::null::<objc::runtime::Object>()
                usingBlock: &*block
            ];

            log::info!("Focus change observer started");
        }
    });
}
