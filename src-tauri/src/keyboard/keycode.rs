/// macOS virtual keycodes
/// Reference: https://developer.apple.com/documentation/carbon/1430449-virtual_key_codes

/// Macro to define keycodes with all their properties in one place.
/// Format: (Variant, raw_code, name, display_name, optional_char, optional_digit)
macro_rules! define_keycodes {
    (
        $(
            $variant:ident = ($code:expr, $name:expr, $display:expr $(, char: $char:expr)? $(, digit: $digit:expr)?)
        ),* $(,)?
    ) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(u16)]
        #[allow(dead_code)]
        pub enum KeyCode {
            $($variant = $code),*
        }

        impl KeyCode {
            pub fn from_raw(code: u16) -> Option<Self> {
                match code {
                    $($code => Some(Self::$variant),)*
                    _ => None,
                }
            }

            pub fn as_raw(&self) -> u16 {
                *self as u16
            }

            /// Convert keycode to a snake_case string name (for settings storage)
            pub fn to_name(&self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)*
                }
            }

            /// Convert keycode to a human-readable display name
            pub fn to_display_name(&self) -> &'static str {
                match self {
                    $(Self::$variant => $display,)*
                }
            }

            /// Parse a key name string to KeyCode
            pub fn from_name(name: &str) -> Option<Self> {
                match name.to_lowercase().as_str() {
                    $($name => Some(Self::$variant),)*
                    _ => None,
                }
            }

            /// Convert keycode to character (for r{char} replacement)
            pub fn to_char(self) -> Option<char> {
                match self {
                    $($(Self::$variant => Some($char),)?)*
                    _ => None,
                }
            }

            /// Convert a numeric keycode to its digit value
            pub fn to_digit(self) -> Option<u32> {
                match self {
                    $($(Self::$variant => Some($digit),)?)*
                    _ => None,
                }
            }
        }
    };
}

define_keycodes! {
    // Letters
    A = (0x00, "a", "A", char: 'a'),
    S = (0x01, "s", "S", char: 's'),
    D = (0x02, "d", "D", char: 'd'),
    F = (0x03, "f", "F", char: 'f'),
    H = (0x04, "h", "H", char: 'h'),
    G = (0x05, "g", "G", char: 'g'),
    Z = (0x06, "z", "Z", char: 'z'),
    X = (0x07, "x", "X", char: 'x'),
    C = (0x08, "c", "C", char: 'c'),
    V = (0x09, "v", "V", char: 'v'),
    B = (0x0B, "b", "B", char: 'b'),
    Q = (0x0C, "q", "Q", char: 'q'),
    W = (0x0D, "w", "W", char: 'w'),
    E = (0x0E, "e", "E", char: 'e'),
    R = (0x0F, "r", "R", char: 'r'),
    Y = (0x10, "y", "Y", char: 'y'),
    T = (0x11, "t", "T", char: 't'),
    O = (0x1F, "o", "O", char: 'o'),
    U = (0x20, "u", "U", char: 'u'),
    I = (0x22, "i", "I", char: 'i'),
    P = (0x23, "p", "P", char: 'p'),
    L = (0x25, "l", "L", char: 'l'),
    J = (0x26, "j", "J", char: 'j'),
    K = (0x28, "k", "K", char: 'k'),
    N = (0x2D, "n", "N", char: 'n'),
    M = (0x2E, "m", "M", char: 'm'),

    // Numbers
    Num1 = (0x12, "1", "1", char: '1', digit: 1),
    Num2 = (0x13, "2", "2", char: '2', digit: 2),
    Num3 = (0x14, "3", "3", char: '3', digit: 3),
    Num4 = (0x15, "4", "4", char: '4', digit: 4),
    Num5 = (0x17, "5", "5", char: '5', digit: 5),
    Num6 = (0x16, "6", "6", char: '6', digit: 6),
    Num7 = (0x1A, "7", "7", char: '7', digit: 7),
    Num8 = (0x1C, "8", "8", char: '8', digit: 8),
    Num9 = (0x19, "9", "9", char: '9', digit: 9),
    Num0 = (0x1D, "0", "0", char: '0', digit: 0),

    // Special keys
    Return = (0x24, "return", "Return"),
    Tab = (0x30, "tab", "Tab"),
    Space = (0x31, "space", "Space", char: ' '),
    Delete = (0x33, "delete", "Delete"),
    Escape = (0x35, "escape", "Escape"),
    Command = (0x37, "command", "Command"),
    Shift = (0x38, "shift", "Shift"),
    CapsLock = (0x39, "caps_lock", "Caps Lock"),
    Option = (0x3A, "option", "Option"),
    Control = (0x3B, "control", "Control"),
    RightShift = (0x3C, "right_shift", "Right Shift"),
    RightOption = (0x3D, "right_option", "Right Option"),
    RightControl = (0x3E, "right_control", "Right Control"),
    Function = (0x3F, "function", "Function"),

    // Arrow keys
    Left = (0x7B, "left", "Left Arrow"),
    Right = (0x7C, "right", "Right Arrow"),
    Down = (0x7D, "down", "Down Arrow"),
    Up = (0x7E, "up", "Up Arrow"),

    // Function keys
    F1 = (0x7A, "f1", "F1"),
    F2 = (0x78, "f2", "F2"),
    F3 = (0x63, "f3", "F3"),
    F4 = (0x76, "f4", "F4"),
    F5 = (0x60, "f5", "F5"),
    F6 = (0x61, "f6", "F6"),
    F7 = (0x62, "f7", "F7"),
    F8 = (0x64, "f8", "F8"),
    F9 = (0x65, "f9", "F9"),
    F10 = (0x6D, "f10", "F10"),
    F11 = (0x67, "f11", "F11"),
    F12 = (0x6F, "f12", "F12"),

    // Navigation
    Home = (0x73, "home", "Home"),
    End = (0x77, "end", "End"),
    PageUp = (0x74, "page_up", "Page Up"),
    PageDown = (0x79, "page_down", "Page Down"),
    ForwardDelete = (0x75, "forward_delete", "Forward Delete"),

    // Punctuation
    Equal = (0x18, "equal", "="),
    Minus = (0x1B, "minus", "-"),
    LeftBracket = (0x21, "left_bracket", "["),
    RightBracket = (0x1E, "right_bracket", "]"),
    Quote = (0x27, "quote", "'"),
    Semicolon = (0x29, "semicolon", ";"),
    Backslash = (0x2A, "backslash", "\\"),
    Comma = (0x2B, "comma", ","),
    Slash = (0x2C, "slash", "/"),
    Period = (0x2F, "period", "."),
    Grave = (0x32, "grave", "`"),
}

/// Modifier flags matching CGEventFlags
#[derive(Debug, Clone, Copy, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub option: bool,
    pub command: bool,
    pub caps_lock: bool,
}

impl Modifiers {
    const SHIFT_MASK: u64 = 0x00020000;
    const CONTROL_MASK: u64 = 0x00040000;
    const OPTION_MASK: u64 = 0x00080000;
    const COMMAND_MASK: u64 = 0x00100000;
    const CAPS_LOCK_MASK: u64 = 0x00010000;

    pub fn from_cg_flags(flags: u64) -> Self {
        Self {
            shift: flags & Self::SHIFT_MASK != 0,
            control: flags & Self::CONTROL_MASK != 0,
            option: flags & Self::OPTION_MASK != 0,
            command: flags & Self::COMMAND_MASK != 0,
            caps_lock: flags & Self::CAPS_LOCK_MASK != 0,
        }
    }

    pub fn to_cg_flags(self) -> u64 {
        let mut flags = 0u64;
        if self.shift {
            flags |= Self::SHIFT_MASK;
        }
        if self.control {
            flags |= Self::CONTROL_MASK;
        }
        if self.option {
            flags |= Self::OPTION_MASK;
        }
        if self.command {
            flags |= Self::COMMAND_MASK;
        }
        if self.caps_lock {
            flags |= Self::CAPS_LOCK_MASK;
        }
        flags
    }
}

/// A key event with code and modifiers
#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    pub code: u16,
    pub modifiers: Modifiers,
    pub is_key_down: bool,
}

impl KeyEvent {
    pub fn keycode(&self) -> Option<KeyCode> {
        KeyCode::from_raw(self.code)
    }
}
