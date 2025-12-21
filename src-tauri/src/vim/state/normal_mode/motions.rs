//! Motion handling for normal mode (g combos, replace char)

use crate::keyboard::{KeyCode, Modifiers};

use super::super::super::commands::VimCommand;
use super::super::action::VimAction;
use super::super::{ProcessResult, VimState};

impl VimState {
    pub(super) fn handle_g_combo(
        &mut self,
        keycode: KeyCode,
        modifiers: &Modifiers,
    ) -> ProcessResult {
        let count = self.get_count();
        self.pending_count = None;

        match keycode {
            KeyCode::G => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::DocumentStart,
                count: 1,
                select: false,
            }),
            KeyCode::E => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::WordEndBackward,
                count,
                select: false,
            }),
            KeyCode::J => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::MoveDown,
                count,
                select: false,
            }),
            KeyCode::K => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::MoveUp,
                count,
                select: false,
            }),
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
            _ => ProcessResult::PassThrough,
        }
    }

    pub(super) fn handle_replace_char(
        &mut self,
        keycode: KeyCode,
        modifiers: &Modifiers,
    ) -> ProcessResult {
        let count = self.get_count();
        self.pending_count = None;

        if keycode.to_char().is_some() {
            ProcessResult::SuppressWithAction(VimAction::ReplaceChar {
                keycode,
                shift: modifiers.shift,
                count,
            })
        } else {
            ProcessResult::Suppress
        }
    }
}
