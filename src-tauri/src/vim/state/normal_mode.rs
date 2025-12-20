use crate::keyboard::{KeyCode, Modifiers};
use super::super::commands::{Operator, VimCommand};
use super::super::modes::VimMode;
use super::action::VimAction;
use super::{IndentDirection, ProcessResult, TextObjectModifier, VimState};

impl VimState {
    pub(super) fn process_normal_mode(&mut self, keycode: KeyCode, modifiers: &Modifiers) -> ProcessResult {
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
        // Only accumulate if shift is NOT pressed (shift+number = special chars like $ ^)
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

    fn handle_normal_command(&mut self, keycode: KeyCode, modifiers: &Modifiers) -> ProcessResult {
        let count = self.get_count();
        self.pending_count = None;

        match keycode {
            // Basic motions
            KeyCode::H => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::MoveLeft, count, select: false
            }),
            KeyCode::J => {
                if modifiers.shift {
                    // J = join lines
                    ProcessResult::SuppressWithAction(VimAction::Command {
                        command: VimCommand::JoinLines, count, select: false
                    })
                } else {
                    ProcessResult::SuppressWithAction(VimAction::Command {
                        command: VimCommand::MoveDown, count, select: false
                    })
                }
            }
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
            // $ = line end (Shift+4)
            KeyCode::Num4 if modifiers.shift => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::LineEnd, count: 1, select: false
            }),
            // ^ = line start (first non-blank, same as 0 on macOS) (Shift+6)
            KeyCode::Num6 if modifiers.shift => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::LineStart, count: 1, select: false
            }),
            // _ = first non-blank character (Shift+Minus)
            KeyCode::Minus if modifiers.shift => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::LineStart, count: 1, select: false
            }),

            // Paragraph motions { and } (Shift+[ and Shift+])
            KeyCode::LeftBracket if modifiers.shift => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::ParagraphUp, count, select: false
            }),
            KeyCode::RightBracket if modifiers.shift => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::ParagraphDown, count, select: false
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
            KeyCode::D => self.handle_delete_operator(count, modifiers),
            KeyCode::Y => self.handle_yank_operator(count, modifiers),
            KeyCode::C => self.handle_change_operator(count, modifiers),

            // Single-key operations
            KeyCode::X => {
                if modifiers.shift {
                    // X = delete char before cursor
                    ProcessResult::SuppressWithAction(VimAction::Command {
                        command: VimCommand::DeleteCharBefore, count, select: false
                    })
                } else {
                    // x = delete char
                    ProcessResult::SuppressWithAction(VimAction::Command {
                        command: VimCommand::DeleteChar, count, select: false
                    })
                }
            }

            // s = substitute (delete char + insert mode)
            KeyCode::S => {
                self.set_mode(VimMode::Insert);
                if modifiers.shift {
                    // S = substitute line (same as cc)
                    ProcessResult::ModeChanged(VimMode::Insert, Some(VimAction::Command {
                        command: VimCommand::SubstituteLine, count, select: false
                    }))
                } else {
                    // s = substitute char
                    ProcessResult::ModeChanged(VimMode::Insert, Some(VimAction::Command {
                        command: VimCommand::SubstituteChar, count, select: false
                    }))
                }
            }

            // r = replace char (wait for next key)
            KeyCode::R => {
                if !modifiers.shift {
                    self.pending_r = true;
                    ProcessResult::Suppress
                } else {
                    // R = replace mode (not implemented, pass through)
                    ProcessResult::PassThrough
                }
            }

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

    fn handle_delete_operator(&mut self, count: u32, modifiers: &Modifiers) -> ProcessResult {
        if modifiers.shift {
            // D = delete to end of line
            ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::DeleteToLineEnd, count, select: false
            })
        } else if self.pending_operator == Some(Operator::Delete) {
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

    fn handle_yank_operator(&mut self, count: u32, modifiers: &Modifiers) -> ProcessResult {
        if modifiers.shift {
            // Y = yank line (like yy for consistency)
            ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::YankLine, count, select: false
            })
        } else if self.pending_operator == Some(Operator::Yank) {
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

    fn handle_change_operator(&mut self, count: u32, modifiers: &Modifiers) -> ProcessResult {
        if modifiers.shift {
            // C = change to end of line
            self.set_mode(VimMode::Insert);
            ProcessResult::ModeChanged(VimMode::Insert, Some(VimAction::Command {
                command: VimCommand::ChangeToLineEnd, count, select: false
            }))
        } else if self.pending_operator == Some(Operator::Change) {
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

    fn handle_g_combo(&mut self, keycode: KeyCode, modifiers: &Modifiers) -> ProcessResult {
        let count = self.get_count();
        self.pending_count = None;

        match keycode {
            KeyCode::G => {
                // gg = go to start
                ProcessResult::SuppressWithAction(VimAction::Command {
                    command: VimCommand::DocumentStart, count: 1, select: false
                })
            }
            KeyCode::E => {
                // ge = end of previous word
                ProcessResult::SuppressWithAction(VimAction::Command {
                    command: VimCommand::WordEndBackward, count, select: false
                })
            }
            KeyCode::J => {
                // gj = move down (same as j in text fields)
                ProcessResult::SuppressWithAction(VimAction::Command {
                    command: VimCommand::MoveDown, count, select: false
                })
            }
            KeyCode::K => {
                // gk = move up (same as k in text fields)
                ProcessResult::SuppressWithAction(VimAction::Command {
                    command: VimCommand::MoveUp, count, select: false
                })
            }
            KeyCode::Num0 => {
                // g0 = screen line start (same as 0 in text fields)
                ProcessResult::SuppressWithAction(VimAction::Command {
                    command: VimCommand::LineStart, count: 1, select: false
                })
            }
            KeyCode::Num4 if modifiers.shift => {
                // g$ = screen line end (same as $ in text fields)
                ProcessResult::SuppressWithAction(VimAction::Command {
                    command: VimCommand::LineEnd, count: 1, select: false
                })
            }
            _ => ProcessResult::PassThrough,
        }
    }

    fn handle_replace_char(&mut self, keycode: KeyCode, modifiers: &Modifiers) -> ProcessResult {
        // For r{char}, we delete the current char and type the replacement
        // This is a simplified version - full vim would handle this differently
        let count = self.get_count();
        self.pending_count = None;

        // Only handle letter and number keys for replacement
        if keycode.to_char().is_some() {
            ProcessResult::SuppressWithAction(VimAction::ReplaceChar {
                keycode,
                shift: modifiers.shift,
                count,
            })
        } else {
            // Cancel replace on non-character keys
            ProcessResult::Suppress
        }
    }

    fn handle_text_object(&mut self, keycode: KeyCode) -> ProcessResult {
        let modifier = match self.pending_text_object.take() {
            Some(m) => m,
            None => return ProcessResult::PassThrough,
        };

        let operator = match self.pending_operator.take() {
            Some(op) => op,
            None => return ProcessResult::PassThrough,
        };

        let count = self.get_count();
        self.pending_count = None;

        // Only 'w' text object is supported for now
        if keycode == KeyCode::W {
            let text_object = match modifier {
                TextObjectModifier::Inner => VimCommand::InnerWord,
                TextObjectModifier::Around => VimCommand::AroundWord,
            };

            // Execute text object selection then operator
            if operator == Operator::Change {
                self.set_mode(VimMode::Insert);
                ProcessResult::ModeChanged(VimMode::Insert, Some(VimAction::TextObject {
                    operator,
                    text_object,
                    count,
                }))
            } else {
                ProcessResult::SuppressWithAction(VimAction::TextObject {
                    operator,
                    text_object,
                    count,
                })
            }
        } else {
            // Unsupported text object
            self.reset_pending();
            ProcessResult::Suppress
        }
    }

    fn handle_indent_combo(&mut self, keycode: KeyCode, direction: IndentDirection) -> ProcessResult {
        let count = self.get_count();
        self.pending_count = None;

        // >> or << (same key pressed twice)
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
                command, count, select: false
            })
        } else {
            // Cancel on different key
            ProcessResult::PassThrough
        }
    }

    pub(super) fn handle_operator_motion(&mut self, keycode: KeyCode, modifiers: &Modifiers) -> ProcessResult {
        // Check for doubled operator (dd, yy, cc)
        let doubled = match (&self.pending_operator, keycode) {
            (Some(Operator::Delete), KeyCode::D) if !modifiers.shift => true,
            (Some(Operator::Yank), KeyCode::Y) if !modifiers.shift => true,
            (Some(Operator::Change), KeyCode::C) if !modifiers.shift => true,
            _ => false,
        };

        if doubled {
            let operator = self.pending_operator.take().unwrap();
            let count = self.get_count();
            self.pending_count = None;

            let command = match operator {
                Operator::Delete => VimCommand::DeleteLine,
                Operator::Yank => VimCommand::YankLine,
                Operator::Change => VimCommand::ChangeLine,
            };

            if operator == Operator::Change {
                self.set_mode(VimMode::Insert);
                return ProcessResult::ModeChanged(VimMode::Insert, Some(VimAction::Command {
                    command, count, select: false
                }));
            } else {
                return ProcessResult::SuppressWithAction(VimAction::Command {
                    command, count, select: false
                });
            }
        }

        // Check for text object modifier (i or a)
        if keycode == KeyCode::I && !modifiers.shift {
            self.pending_text_object = Some(TextObjectModifier::Inner);
            return ProcessResult::Suppress;
        }
        if keycode == KeyCode::A && !modifiers.shift {
            self.pending_text_object = Some(TextObjectModifier::Around);
            return ProcessResult::Suppress;
        }

        // Handle g prefix in operator mode
        if keycode == KeyCode::G && !modifiers.shift {
            self.pending_g = true;
            return ProcessResult::Suppress;
        }

        // Handle gg motion after g in operator mode
        if self.pending_g {
            self.pending_g = false;
            if keycode == KeyCode::G {
                let operator = match self.pending_operator.take() {
                    Some(op) => op,
                    None => return ProcessResult::PassThrough,
                };
                let count = self.get_count();
                self.pending_count = None;

                if operator == Operator::Change {
                    self.set_mode(VimMode::Insert);
                    return ProcessResult::ModeChanged(VimMode::Insert, Some(VimAction::OperatorMotion {
                        operator, motion: VimCommand::DocumentStart, count
                    }));
                } else {
                    return ProcessResult::SuppressWithAction(VimAction::OperatorMotion {
                        operator, motion: VimCommand::DocumentStart, count
                    });
                }
            }
        }

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
            // $ = line end
            KeyCode::Num4 if modifiers.shift => Some(VimCommand::LineEnd),
            // ^ = line start
            KeyCode::Num6 if modifiers.shift => Some(VimCommand::LineStart),
            // _ = first non-blank character
            KeyCode::Minus if modifiers.shift => Some(VimCommand::LineStart),
            // { = paragraph up
            KeyCode::LeftBracket if modifiers.shift => Some(VimCommand::ParagraphUp),
            // } = paragraph down
            KeyCode::RightBracket if modifiers.shift => Some(VimCommand::ParagraphDown),
            // G = document end
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
            // Invalid motion, reset and restore operator
            self.pending_operator = Some(operator);
            ProcessResult::Suppress
        }
    }
}
