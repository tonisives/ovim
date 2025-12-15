use crate::keyboard;
use super::super::commands::{Operator, VimCommand};

/// Action to execute after suppressing the key event
#[derive(Debug, Clone)]
pub enum VimAction {
    /// No action needed
    None,
    /// Execute a vim command
    Command { command: VimCommand, count: u32, select: bool },
    /// Execute an operator with a motion
    OperatorMotion { operator: Operator, motion: VimCommand, count: u32 },
    /// Cut (Cmd+X)
    Cut,
    /// Copy (Cmd+C)
    Copy,
}

impl VimAction {
    /// Execute the action
    pub fn execute(&self) -> Result<bool, String> {
        match self {
            VimAction::None => Ok(false),
            VimAction::Command { command, count, select } => {
                command.execute(*count, *select)?;
                Ok(false)
            }
            VimAction::OperatorMotion { operator, motion, count } => {
                operator.execute_with_motion(*motion, *count)
            }
            VimAction::Cut => {
                keyboard::cut()?;
                Ok(false)
            }
            VimAction::Copy => {
                keyboard::copy()?;
                Ok(false)
            }
        }
    }
}
