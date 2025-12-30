mod capture;
mod inject;
pub mod keycode;
mod permission;

#[allow(unused_imports)]
pub use capture::{KeyboardCapture, MouseClickEvent};
pub use inject::*;
pub use keycode::{KeyCode, KeyEvent, Modifiers};
pub use permission::{check_accessibility_permission, request_accessibility_permission};
