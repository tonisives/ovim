//! Operator handling for normal mode (d, y, c)

use crate::keyboard::{KeyCode, Modifiers};

use super::super::super::commands::{Operator, VimCommand};
use super::super::super::modes::VimMode;
use super::super::action::VimAction;
use super::super::{ProcessResult, TextObjectModifier, VimState};

impl VimState {
    pub(super) fn handle_delete_operator(
        &mut self,
        count: u32,
        modifiers: &Modifiers,
    ) -> ProcessResult {
        if modifiers.shift {
            ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::DeleteToLineEnd,
                count,
                select: false,
            })
        } else if self.pending_operator == Some(Operator::Delete) {
            self.pending_operator = None;
            ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::DeleteLine,
                count,
                select: false,
            })
        } else {
            self.pending_operator = Some(Operator::Delete);
            ProcessResult::Suppress
        }
    }

    pub(super) fn handle_yank_operator(
        &mut self,
        count: u32,
        modifiers: &Modifiers,
    ) -> ProcessResult {
        if modifiers.shift {
            ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::YankLine,
                count,
                select: false,
            })
        } else if self.pending_operator == Some(Operator::Yank) {
            self.pending_operator = None;
            ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::YankLine,
                count,
                select: false,
            })
        } else {
            self.pending_operator = Some(Operator::Yank);
            ProcessResult::Suppress
        }
    }

    pub(super) fn handle_change_operator(
        &mut self,
        count: u32,
        modifiers: &Modifiers,
    ) -> ProcessResult {
        if modifiers.shift {
            self.set_mode(VimMode::Insert);
            ProcessResult::ModeChanged(
                VimMode::Insert,
                Some(VimAction::Command {
                    command: VimCommand::ChangeToLineEnd,
                    count,
                    select: false,
                }),
            )
        } else if self.pending_operator == Some(Operator::Change) {
            self.pending_operator = None;
            self.set_mode(VimMode::Insert);
            ProcessResult::ModeChanged(
                VimMode::Insert,
                Some(VimAction::Command {
                    command: VimCommand::ChangeLine,
                    count,
                    select: false,
                }),
            )
        } else {
            self.pending_operator = Some(Operator::Change);
            ProcessResult::Suppress
        }
    }

    pub(super) fn handle_insert_key(&mut self, modifiers: &Modifiers) -> ProcessResult {
        self.set_mode(VimMode::Insert);
        if modifiers.shift {
            ProcessResult::ModeChanged(
                VimMode::Insert,
                Some(VimAction::Command {
                    command: VimCommand::InsertAtLineStart,
                    count: 1,
                    select: false,
                }),
            )
        } else {
            ProcessResult::ModeChanged(VimMode::Insert, None)
        }
    }

    pub(super) fn handle_append_key(&mut self, modifiers: &Modifiers) -> ProcessResult {
        self.set_mode(VimMode::Insert);
        if modifiers.shift {
            ProcessResult::ModeChanged(
                VimMode::Insert,
                Some(VimAction::Command {
                    command: VimCommand::AppendAtLineEnd,
                    count: 1,
                    select: false,
                }),
            )
        } else {
            ProcessResult::ModeChanged(
                VimMode::Insert,
                Some(VimAction::Command {
                    command: VimCommand::AppendAfterCursor,
                    count: 1,
                    select: false,
                }),
            )
        }
    }

    pub(super) fn handle_open_line_key(&mut self, modifiers: &Modifiers) -> ProcessResult {
        self.set_mode(VimMode::Insert);
        let command = if modifiers.shift {
            VimCommand::OpenLineAbove
        } else {
            VimCommand::OpenLineBelow
        };
        ProcessResult::ModeChanged(
            VimMode::Insert,
            Some(VimAction::Command {
                command,
                count: 1,
                select: false,
            }),
        )
    }

    pub(super) fn handle_operator_motion(
        &mut self,
        keycode: KeyCode,
        modifiers: &Modifiers,
    ) -> ProcessResult {
        // Check for doubled operator (dd, yy, cc)
        let doubled = match (&self.pending_operator, keycode) {
            (Some(Operator::Delete), KeyCode::D) if !modifiers.shift => true,
            (Some(Operator::Yank), KeyCode::Y) if !modifiers.shift => true,
            (Some(Operator::Change), KeyCode::C) if !modifiers.shift => true,
            _ => false,
        };

        if doubled {
            return self.handle_doubled_operator();
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
                return self.handle_operator_gg();
            }
        }

        self.handle_operator_with_motion(keycode, modifiers)
    }

    fn handle_doubled_operator(&mut self) -> ProcessResult {
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
            ProcessResult::ModeChanged(
                VimMode::Insert,
                Some(VimAction::Command {
                    command,
                    count,
                    select: false,
                }),
            )
        } else {
            ProcessResult::SuppressWithAction(VimAction::Command {
                command,
                count,
                select: false,
            })
        }
    }

    fn handle_operator_gg(&mut self) -> ProcessResult {
        let operator = match self.pending_operator.take() {
            Some(op) => op,
            None => return ProcessResult::PassThrough,
        };
        let count = self.get_count();
        self.pending_count = None;

        if operator == Operator::Change {
            self.set_mode(VimMode::Insert);
            ProcessResult::ModeChanged(
                VimMode::Insert,
                Some(VimAction::OperatorMotion {
                    operator,
                    motion: VimCommand::DocumentStart,
                    count,
                }),
            )
        } else {
            ProcessResult::SuppressWithAction(VimAction::OperatorMotion {
                operator,
                motion: VimCommand::DocumentStart,
                count,
            })
        }
    }

    fn handle_operator_with_motion(
        &mut self,
        keycode: KeyCode,
        modifiers: &Modifiers,
    ) -> ProcessResult {
        let operator = match self.pending_operator.take() {
            Some(op) => op,
            None => return ProcessResult::PassThrough,
        };

        let count = self.get_count();
        self.pending_count = None;

        let motion = match keycode {
            KeyCode::H => Some(VimCommand::MoveLeft),
            KeyCode::J => Some(VimCommand::MoveDown),
            KeyCode::K => Some(VimCommand::MoveUp),
            KeyCode::L => Some(VimCommand::MoveRight),
            KeyCode::W => Some(VimCommand::WordForward),
            KeyCode::E => Some(VimCommand::WordEnd),
            KeyCode::B => Some(VimCommand::WordBackward),
            KeyCode::Num0 => Some(VimCommand::LineStart),
            KeyCode::Num4 if modifiers.shift => Some(VimCommand::LineEnd),
            KeyCode::Num6 if modifiers.shift => Some(VimCommand::LineStart),
            KeyCode::Minus if modifiers.shift => Some(VimCommand::LineStart),
            KeyCode::LeftBracket if modifiers.shift => Some(VimCommand::ParagraphUp),
            KeyCode::RightBracket if modifiers.shift => Some(VimCommand::ParagraphDown),
            KeyCode::G if modifiers.shift => Some(VimCommand::DocumentEnd),
            _ => None,
        };

        if let Some(motion) = motion {
            if operator == Operator::Change {
                self.set_mode(VimMode::Insert);
                ProcessResult::ModeChanged(
                    VimMode::Insert,
                    Some(VimAction::OperatorMotion {
                        operator,
                        motion,
                        count,
                    }),
                )
            } else {
                ProcessResult::SuppressWithAction(VimAction::OperatorMotion {
                    operator,
                    motion,
                    count,
                })
            }
        } else {
            // Invalid motion, restore operator
            self.pending_operator = Some(operator);
            ProcessResult::Suppress
        }
    }
}
