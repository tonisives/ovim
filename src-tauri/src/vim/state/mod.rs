mod action;
mod normal_mode;
mod visual_mode;

pub use action::VimAction;

use tokio::sync::broadcast;

use crate::keyboard::{KeyCode, KeyEvent};
use super::commands::Operator;
use super::modes::VimMode;

/// Result of processing a key event
#[derive(Debug, Clone)]
pub enum ProcessResult {
    /// Suppress the key event entirely (no action needed)
    Suppress,
    /// Suppress and execute an action (deferred execution)
    SuppressWithAction(VimAction),
    /// Pass the key event through unchanged
    PassThrough,
    /// Mode changed (emit event), with optional action to execute
    ModeChanged(VimMode, Option<VimAction>),
}

/// Text object modifier (i for inner, a for around)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextObjectModifier {
    Inner,  // i
    Around, // a
}

/// Indent direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentDirection {
    Indent,  // >
    Outdent, // <
}

/// Vim state machine
pub struct VimState {
    mode: VimMode,
    /// The key that toggles vim mode (default: CapsLock)
    vim_key: KeyCode,
    /// Pending count for repeat (e.g., "5" in "5j")
    pending_count: Option<u32>,
    /// Pending operator (d, y, c)
    pending_operator: Option<Operator>,
    /// Pending g key for gg, gt, gT, ge, etc
    pending_g: bool,
    /// Pending r key for r{char} replace
    pending_r: bool,
    /// Pending text object modifier (i or a after d/y/c)
    pending_text_object: Option<TextObjectModifier>,
    /// Pending indent direction (> or <)
    pending_indent: Option<IndentDirection>,
    /// Channel to emit mode changes
    mode_tx: broadcast::Sender<VimMode>,
}

impl VimState {
    pub fn new() -> (Self, broadcast::Receiver<VimMode>) {
        let (mode_tx, mode_rx) = broadcast::channel(16);
        (
            Self {
                mode: VimMode::Insert,
                vim_key: KeyCode::CapsLock,
                pending_count: None,
                pending_operator: None,
                pending_g: false,
                pending_r: false,
                pending_text_object: None,
                pending_indent: None,
                mode_tx,
            },
            mode_rx,
        )
    }

    pub fn mode(&self) -> VimMode {
        self.mode
    }

    pub(super) fn set_mode(&mut self, mode: VimMode) {
        if self.mode != mode {
            self.mode = mode;
            self.reset_pending();
            let _ = self.mode_tx.send(mode);
        }
    }

    /// Set mode externally (from CLI/IPC)
    pub fn set_mode_external(&mut self, mode: VimMode) {
        self.set_mode(mode);
    }

    /// Toggle between insert and normal mode (for CLI/IPC)
    pub fn toggle_mode(&mut self) -> VimMode {
        let new_mode = match self.mode {
            VimMode::Insert => VimMode::Normal,
            VimMode::Normal | VimMode::Visual => VimMode::Insert,
        };
        self.set_mode(new_mode);
        new_mode
    }

    pub(super) fn reset_pending(&mut self) {
        self.pending_count = None;
        self.pending_operator = None;
        self.pending_g = false;
        self.pending_r = false;
        self.pending_text_object = None;
        self.pending_indent = None;
    }

    pub(super) fn get_count(&self) -> u32 {
        self.pending_count.unwrap_or(1)
    }

    /// Get a string representation of pending keys for display
    pub fn get_pending_keys(&self) -> String {
        let mut buf = String::new();
        if let Some(count) = self.pending_count {
            buf.push_str(&count.to_string());
        }
        if let Some(ref op) = self.pending_operator {
            buf.push(match op {
                Operator::Delete => 'd',
                Operator::Yank => 'y',
                Operator::Change => 'c',
            });
        }
        if self.pending_g {
            buf.push('g');
        }
        if self.pending_r {
            buf.push('r');
        }
        if let Some(ref modifier) = self.pending_text_object {
            buf.push(match modifier {
                TextObjectModifier::Inner => 'i',
                TextObjectModifier::Around => 'a',
            });
        }
        if let Some(ref dir) = self.pending_indent {
            buf.push(match dir {
                IndentDirection::Indent => '>',
                IndentDirection::Outdent => '<',
            });
        }
        buf
    }

    /// Process a key event and return what to do with it
    pub fn process_key(&mut self, event: KeyEvent) -> ProcessResult {
        // For key up events in Normal/Visual mode, suppress keys that we would suppress on key down
        if !event.is_key_down {
            return self.process_key_up(&event);
        }

        let keycode = match event.keycode() {
            Some(k) => k,
            None => return ProcessResult::PassThrough,
        };

        // Check for vim key toggle
        if keycode == self.vim_key {
            return self.handle_vim_key();
        }

        match self.mode {
            VimMode::Insert => ProcessResult::PassThrough,
            VimMode::Normal => self.process_normal_mode(keycode, &event.modifiers),
            VimMode::Visual => self.process_visual_mode_with_modifiers(keycode, &event.modifiers),
        }
    }

    fn process_key_up(&self, event: &KeyEvent) -> ProcessResult {
        // In Insert mode, pass through all key up events
        if self.mode == VimMode::Insert {
            return ProcessResult::PassThrough;
        }

        // In Normal/Visual mode, suppress key up for keys we handle
        let keycode = match event.keycode() {
            Some(k) => k,
            None => return ProcessResult::PassThrough,
        };

        // Check if this is a key we would handle (suppress its key up too)
        let should_suppress = matches!(
            keycode,
            KeyCode::H | KeyCode::J | KeyCode::K | KeyCode::L |
            KeyCode::W | KeyCode::E | KeyCode::B |
            KeyCode::Num0 | KeyCode::Num1 | KeyCode::Num2 | KeyCode::Num3 |
            KeyCode::Num4 | KeyCode::Num5 | KeyCode::Num6 | KeyCode::Num7 |
            KeyCode::Num8 | KeyCode::Num9 | KeyCode::G | KeyCode::R |
            KeyCode::D | KeyCode::Y | KeyCode::C | KeyCode::X |
            KeyCode::I | KeyCode::A | KeyCode::O | KeyCode::S |
            KeyCode::V | KeyCode::P | KeyCode::U |
            KeyCode::LeftBracket | KeyCode::RightBracket |
            KeyCode::Period | KeyCode::Comma
        );

        if should_suppress {
            ProcessResult::Suppress
        } else {
            ProcessResult::PassThrough
        }
    }

    fn handle_vim_key(&mut self) -> ProcessResult {
        match self.mode {
            VimMode::Insert => {
                self.set_mode(VimMode::Normal);
                ProcessResult::ModeChanged(VimMode::Normal, None)
            }
            VimMode::Normal | VimMode::Visual => {
                self.set_mode(VimMode::Insert);
                ProcessResult::ModeChanged(VimMode::Insert, None)
            }
        }
    }
}

impl Default for VimState {
    fn default() -> Self {
        Self::new().0
    }
}
