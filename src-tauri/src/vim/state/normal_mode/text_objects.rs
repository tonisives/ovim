//! Text object handling for normal mode (iw, aw, etc.)

use crate::keyboard::KeyCode;

use super::super::super::commands::{Operator, VimCommand};
use super::super::super::modes::VimMode;
use super::super::action::VimAction;
use super::super::{ProcessResult, TextObjectModifier, VimState};

impl VimState {
    pub(super) fn handle_text_object(&mut self, keycode: KeyCode) -> ProcessResult {
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

            if operator == Operator::Change {
                self.set_mode(VimMode::Insert);
                ProcessResult::ModeChanged(
                    VimMode::Insert,
                    Some(VimAction::TextObject {
                        operator,
                        text_object,
                        count,
                    }),
                )
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
}
