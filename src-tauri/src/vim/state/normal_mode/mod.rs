//! Normal mode command processing
//!
//! This module handles all key processing in normal mode.

mod motions;
mod operators;
mod text_objects;

use crate::keyboard::{KeyCode, Modifiers};

use super::super::commands::VimCommand;
use super::super::modes::VimMode;
use super::action::VimAction;
use super::{IndentDirection, ProcessResult, VimState};

impl VimState {
    pub(super) fn process_normal_mode(
        &mut self,
        keycode: KeyCode,
        modifiers: &Modifiers,
    ) -> ProcessResult {
        // Escape always goes to insert mode
        if keycode == KeyCode::Escape {
            self.set_mode(VimMode::Insert);
            return ProcessResult::ModeChanged(VimMode::Insert, None);
        }

        // Handle pending r (replace char)
        if self.pending_r {
            self.pending_r = false;
            return self.handle_replace_char(keycode, modifiers);
        }

        // Handle pending g
        if self.pending_g {
            self.pending_g = false;
            return self.handle_g_combo(keycode, modifiers);
        }

        // Handle pending text object modifier (i or a after operator)
        if self.pending_text_object.is_some() {
            return self.handle_text_object(keycode);
        }

        // Handle pending indent
        if let Some(dir) = self.pending_indent {
            self.pending_indent = None;
            return self.handle_indent_combo(keycode, dir);
        }

        // Handle count accumulation (1-9, then 0-9)
        if !modifiers.shift {
            if let Some(digit) = keycode.to_digit() {
                if digit != 0 || self.pending_count.is_some() {
                    let current = self.pending_count.unwrap_or(0);
                    self.pending_count = Some(current * 10 + digit);
                    return ProcessResult::Suppress;
                }
            }
        }

        // Handle operators with pending motion
        if self.pending_operator.is_some() {
            return self.handle_operator_motion(keycode, modifiers);
        }

        // Check for control key combinations
        if modifiers.control {
            return self.handle_control_combo(keycode);
        }

        // Normal mode commands
        self.handle_normal_command(keycode, modifiers)
    }

    fn handle_normal_command(
        &mut self,
        keycode: KeyCode,
        modifiers: &Modifiers,
    ) -> ProcessResult {
        let count = self.get_count();
        self.pending_count = None;

        match keycode {
            // Basic motions
            KeyCode::H => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::MoveLeft,
                count,
                select: false,
            }),
            KeyCode::J => self.handle_j_key(modifiers, count),
            KeyCode::K => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::MoveUp,
                count,
                select: false,
            }),
            KeyCode::L => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::MoveRight,
                count,
                select: false,
            }),

            // Word motions
            KeyCode::W => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::WordForward,
                count,
                select: false,
            }),
            KeyCode::E => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::WordEnd,
                count,
                select: false,
            }),
            KeyCode::B => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::WordBackward,
                count,
                select: false,
            }),

            // Line motions
            KeyCode::Num0 => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::LineStart,
                count: 1,
                select: false,
            }),
            KeyCode::Num4 if modifiers.shift => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::LineEnd,
                count: 1,
                select: false,
            }),
            KeyCode::Num6 if modifiers.shift => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::LineStart,
                count: 1,
                select: false,
            }),
            KeyCode::Minus if modifiers.shift => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::LineStart,
                count: 1,
                select: false,
            }),

            // Paragraph motions
            KeyCode::LeftBracket if modifiers.shift => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::ParagraphUp,
                count,
                select: false,
            }),
            KeyCode::RightBracket if modifiers.shift => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::ParagraphDown,
                count,
                select: false,
            }),

            // g commands
            KeyCode::G => self.handle_g_key(modifiers),

            // Operators
            KeyCode::D => self.handle_delete_operator(count, modifiers),
            KeyCode::Y => self.handle_yank_operator(count, modifiers),
            KeyCode::C => self.handle_change_operator(count, modifiers),

            // Single-key operations
            KeyCode::X => self.handle_x_key(modifiers, count),
            KeyCode::S => self.handle_s_key(modifiers, count),
            KeyCode::R => self.handle_r_key(modifiers),

            // Insert mode entries
            KeyCode::I => self.handle_insert_key(modifiers),
            KeyCode::A => self.handle_append_key(modifiers),
            KeyCode::O => self.handle_open_line_key(modifiers),

            // Visual mode
            KeyCode::V => {
                self.set_mode(VimMode::Visual);
                ProcessResult::ModeChanged(VimMode::Visual, None)
            }

            // Clipboard
            KeyCode::P => {
                let command = if modifiers.shift {
                    VimCommand::PasteBefore
                } else {
                    VimCommand::Paste
                };
                ProcessResult::SuppressWithAction(VimAction::Command {
                    command,
                    count,
                    select: false,
                })
            }

            // Undo
            KeyCode::U => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::Undo,
                count,
                select: false,
            }),

            // Indent: > (Shift+.)
            KeyCode::Period if modifiers.shift => {
                self.pending_indent = Some(IndentDirection::Indent);
                ProcessResult::Suppress
            }

            // Outdent: < (Shift+,)
            KeyCode::Comma if modifiers.shift => {
                self.pending_indent = Some(IndentDirection::Outdent);
                ProcessResult::Suppress
            }

            _ => ProcessResult::PassThrough,
        }
    }

    fn handle_j_key(&self, modifiers: &Modifiers, count: u32) -> ProcessResult {
        if modifiers.shift {
            ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::JoinLines,
                count,
                select: false,
            })
        } else {
            ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::MoveDown,
                count,
                select: false,
            })
        }
    }

    fn handle_g_key(&mut self, modifiers: &Modifiers) -> ProcessResult {
        if modifiers.shift {
            ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::DocumentEnd,
                count: 1,
                select: false,
            })
        } else {
            self.pending_g = true;
            ProcessResult::Suppress
        }
    }

    fn handle_x_key(&self, modifiers: &Modifiers, count: u32) -> ProcessResult {
        let command = if modifiers.shift {
            VimCommand::DeleteCharBefore
        } else {
            VimCommand::DeleteChar
        };
        ProcessResult::SuppressWithAction(VimAction::Command {
            command,
            count,
            select: false,
        })
    }

    fn handle_s_key(&mut self, modifiers: &Modifiers, count: u32) -> ProcessResult {
        self.set_mode(VimMode::Insert);
        let command = if modifiers.shift {
            VimCommand::SubstituteLine
        } else {
            VimCommand::SubstituteChar
        };
        ProcessResult::ModeChanged(
            VimMode::Insert,
            Some(VimAction::Command {
                command,
                count,
                select: false,
            }),
        )
    }

    fn handle_r_key(&mut self, modifiers: &Modifiers) -> ProcessResult {
        if !modifiers.shift {
            self.pending_r = true;
            ProcessResult::Suppress
        } else {
            ProcessResult::PassThrough
        }
    }

    fn handle_control_combo(&mut self, keycode: KeyCode) -> ProcessResult {
        let count = self.get_count();
        self.pending_count = None;

        let command = match keycode {
            KeyCode::F => VimCommand::PageDown,
            KeyCode::B => VimCommand::PageUp,
            KeyCode::D => VimCommand::HalfPageDown,
            KeyCode::U => VimCommand::HalfPageUp,
            KeyCode::R => VimCommand::Redo,
            _ => return ProcessResult::PassThrough,
        };

        ProcessResult::SuppressWithAction(VimAction::Command {
            command,
            count,
            select: false,
        })
    }

    fn handle_indent_combo(
        &mut self,
        keycode: KeyCode,
        direction: IndentDirection,
    ) -> ProcessResult {
        let count = self.get_count();
        self.pending_count = None;

        let expected_key = match direction {
            IndentDirection::Indent => KeyCode::Period,
            IndentDirection::Outdent => KeyCode::Comma,
        };

        if keycode == expected_key {
            let command = match direction {
                IndentDirection::Indent => VimCommand::IndentLine,
                IndentDirection::Outdent => VimCommand::OutdentLine,
            };
            ProcessResult::SuppressWithAction(VimAction::Command {
                command,
                count,
                select: false,
            })
        } else {
            ProcessResult::PassThrough
        }
    }
}
