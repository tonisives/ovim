use crate::keyboard::{KeyCode, Modifiers};
use super::super::commands::{Operator, VimCommand};
use super::super::modes::VimMode;
use super::action::VimAction;
use super::ProcessResult;
use super::VimState;

impl VimState {
    pub(super) fn process_normal_mode(&mut self, keycode: KeyCode, modifiers: &Modifiers) -> ProcessResult {
        // Escape always goes to insert mode
        if keycode == KeyCode::Escape {
            self.set_mode(VimMode::Insert);
            return ProcessResult::ModeChanged(VimMode::Insert, None);
        }

        // Handle pending g
        if self.pending_g {
            self.pending_g = false;
            return self.handle_g_combo(keycode);
        }

        // Handle count accumulation (1-9, then 0-9)
        if let Some(digit) = keycode.to_digit() {
            if digit != 0 || self.pending_count.is_some() {
                let current = self.pending_count.unwrap_or(0);
                self.pending_count = Some(current * 10 + digit);
                return ProcessResult::Suppress;
            }
        }

        // Handle operators
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

    fn handle_normal_command(&mut self, keycode: KeyCode, modifiers: &Modifiers) -> ProcessResult {
        let count = self.get_count();
        self.pending_count = None;

        match keycode {
            // Basic motions
            KeyCode::H => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::MoveLeft, count, select: false
            }),
            KeyCode::J => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::MoveDown, count, select: false
            }),
            KeyCode::K => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::MoveUp, count, select: false
            }),
            KeyCode::L => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::MoveRight, count, select: false
            }),

            // Word motions
            KeyCode::W => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::WordForward, count, select: false
            }),
            KeyCode::E => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::WordEnd, count, select: false
            }),
            KeyCode::B => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::WordBackward, count, select: false
            }),

            // Line motions
            KeyCode::Num0 => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::LineStart, count: 1, select: false
            }),

            // g commands
            KeyCode::G => {
                if modifiers.shift {
                    // G = go to end
                    ProcessResult::SuppressWithAction(VimAction::Command {
                        command: VimCommand::DocumentEnd, count: 1, select: false
                    })
                } else {
                    // g = start g combo
                    self.pending_g = true;
                    ProcessResult::Suppress
                }
            }

            // Operators
            KeyCode::D => self.handle_delete_operator(count),
            KeyCode::Y => self.handle_yank_operator(count),
            KeyCode::C => self.handle_change_operator(count),

            // Single-key operations
            KeyCode::X => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::DeleteChar, count, select: false
            }),

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
                    command, count, select: false
                })
            }

            // Undo/Redo
            KeyCode::U => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::Undo, count, select: false
            }),

            _ => ProcessResult::PassThrough,
        }
    }

    fn handle_delete_operator(&mut self, count: u32) -> ProcessResult {
        if self.pending_operator == Some(Operator::Delete) {
            // dd = delete line
            self.pending_operator = None;
            ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::DeleteLine, count, select: false
            })
        } else {
            self.pending_operator = Some(Operator::Delete);
            ProcessResult::Suppress
        }
    }

    fn handle_yank_operator(&mut self, count: u32) -> ProcessResult {
        if self.pending_operator == Some(Operator::Yank) {
            // yy = yank line
            self.pending_operator = None;
            ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::YankLine, count, select: false
            })
        } else {
            self.pending_operator = Some(Operator::Yank);
            ProcessResult::Suppress
        }
    }

    fn handle_change_operator(&mut self, count: u32) -> ProcessResult {
        if self.pending_operator == Some(Operator::Change) {
            // cc = change line
            self.pending_operator = None;
            self.set_mode(VimMode::Insert);
            ProcessResult::ModeChanged(VimMode::Insert, Some(VimAction::Command {
                command: VimCommand::ChangeLine, count, select: false
            }))
        } else {
            self.pending_operator = Some(Operator::Change);
            ProcessResult::Suppress
        }
    }

    fn handle_insert_key(&mut self, modifiers: &Modifiers) -> ProcessResult {
        self.set_mode(VimMode::Insert);
        if modifiers.shift {
            // I = insert at line start
            ProcessResult::ModeChanged(VimMode::Insert, Some(VimAction::Command {
                command: VimCommand::InsertAtLineStart, count: 1, select: false
            }))
        } else {
            ProcessResult::ModeChanged(VimMode::Insert, None)
        }
    }

    fn handle_append_key(&mut self, modifiers: &Modifiers) -> ProcessResult {
        self.set_mode(VimMode::Insert);
        if modifiers.shift {
            // A = append at line end
            ProcessResult::ModeChanged(VimMode::Insert, Some(VimAction::Command {
                command: VimCommand::AppendAtLineEnd, count: 1, select: false
            }))
        } else {
            // a = append after cursor
            ProcessResult::ModeChanged(VimMode::Insert, Some(VimAction::Command {
                command: VimCommand::AppendAfterCursor, count: 1, select: false
            }))
        }
    }

    fn handle_open_line_key(&mut self, modifiers: &Modifiers) -> ProcessResult {
        self.set_mode(VimMode::Insert);
        if modifiers.shift {
            ProcessResult::ModeChanged(VimMode::Insert, Some(VimAction::Command {
                command: VimCommand::OpenLineAbove, count: 1, select: false
            }))
        } else {
            ProcessResult::ModeChanged(VimMode::Insert, Some(VimAction::Command {
                command: VimCommand::OpenLineBelow, count: 1, select: false
            }))
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
            command, count, select: false
        })
    }

    fn handle_g_combo(&mut self, keycode: KeyCode) -> ProcessResult {
        match keycode {
            KeyCode::G => {
                // gg = go to start
                ProcessResult::SuppressWithAction(VimAction::Command {
                    command: VimCommand::DocumentStart, count: 1, select: false
                })
            }
            _ => ProcessResult::PassThrough,
        }
    }

    pub(super) fn handle_operator_motion(&mut self, keycode: KeyCode, modifiers: &Modifiers) -> ProcessResult {
        let operator = match self.pending_operator.take() {
            Some(op) => op,
            None => return ProcessResult::PassThrough,
        };

        let count = self.get_count();
        self.pending_count = None;

        // Map keycode to motion
        let motion = match keycode {
            KeyCode::H => Some(VimCommand::MoveLeft),
            KeyCode::J => Some(VimCommand::MoveDown),
            KeyCode::K => Some(VimCommand::MoveUp),
            KeyCode::L => Some(VimCommand::MoveRight),
            KeyCode::W => Some(VimCommand::WordForward),
            KeyCode::E => Some(VimCommand::WordEnd),
            KeyCode::B => Some(VimCommand::WordBackward),
            KeyCode::Num0 => Some(VimCommand::LineStart),
            KeyCode::G if modifiers.shift => Some(VimCommand::DocumentEnd),
            _ => None,
        };

        if let Some(motion) = motion {
            // For change operator, we need to enter insert mode after the action
            if operator == Operator::Change {
                self.set_mode(VimMode::Insert);
                ProcessResult::ModeChanged(VimMode::Insert, Some(VimAction::OperatorMotion {
                    operator, motion, count
                }))
            } else {
                ProcessResult::SuppressWithAction(VimAction::OperatorMotion {
                    operator, motion, count
                })
            }
        } else {
            // Invalid motion, reset
            self.reset_pending();
            ProcessResult::Suppress
        }
    }
}
