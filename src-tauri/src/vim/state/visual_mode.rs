use crate::keyboard::KeyCode;
use super::super::commands::VimCommand;
use super::super::modes::VimMode;
use super::action::VimAction;
use super::ProcessResult;
use super::VimState;

impl VimState {
    pub(super) fn process_visual_mode(&mut self, keycode: KeyCode) -> ProcessResult {
        // Escape exits visual mode
        if keycode == KeyCode::Escape {
            self.set_mode(VimMode::Normal);
            return ProcessResult::ModeChanged(VimMode::Normal, None);
        }

        // v toggles back to normal
        if keycode == KeyCode::V {
            self.set_mode(VimMode::Normal);
            return ProcessResult::ModeChanged(VimMode::Normal, None);
        }

        // Handle motions (with selection)
        let count = self.get_count();
        self.pending_count = None;

        match keycode {
            KeyCode::H => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::MoveLeft, count, select: true
            }),
            KeyCode::J => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::MoveDown, count, select: true
            }),
            KeyCode::K => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::MoveUp, count, select: true
            }),
            KeyCode::L => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::MoveRight, count, select: true
            }),
            KeyCode::W | KeyCode::E => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::WordForward, count, select: true
            }),
            KeyCode::B => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::WordBackward, count, select: true
            }),
            KeyCode::Num0 => ProcessResult::SuppressWithAction(VimAction::Command {
                command: VimCommand::LineStart, count: 1, select: true
            }),

            // Operations
            KeyCode::D | KeyCode::X => {
                self.set_mode(VimMode::Normal);
                ProcessResult::ModeChanged(VimMode::Normal, Some(VimAction::Cut))
            }
            KeyCode::Y => {
                self.set_mode(VimMode::Normal);
                ProcessResult::ModeChanged(VimMode::Normal, Some(VimAction::Copy))
            }
            KeyCode::C => {
                self.set_mode(VimMode::Insert);
                ProcessResult::ModeChanged(VimMode::Insert, Some(VimAction::Cut))
            }

            _ => ProcessResult::PassThrough,
        }
    }
}
