//! Double-tap detection for modifier keys
//!
//! Tracks FlagsChanged events to detect when a modifier key is pressed
//! and released twice in quick succession (double-tap).

use std::time::{Duration, Instant};

/// Which key to track for double-tap
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoubleTapKey {
    Command,
    Option,
    Control,
    Shift,
    Escape,
}

/// Tracks the state for double-tap detection
pub struct DoubleTapTracker {
    /// Maximum time between taps to count as a double-tap
    max_interval: Duration,
    /// Maximum time a key can be held to count as a tap (not a hold)
    max_hold_duration: Duration,
    /// Timestamp when the key was last pressed
    last_press_time: Option<Instant>,
    /// Timestamp when the key was last released (completing a tap)
    last_release_time: Option<Instant>,
    /// Whether we're currently in a "pressed" state
    is_pressed: bool,
    /// Count of recent taps within the interval
    tap_count: u8,
}

impl DoubleTapTracker {
    pub fn new() -> Self {
        Self {
            max_interval: Duration::from_millis(300),
            max_hold_duration: Duration::from_millis(200),
            last_press_time: None,
            last_release_time: None,
            is_pressed: false,
            tap_count: 0,
        }
    }

    /// Update tracker when the modifier key is pressed.
    /// Returns true if this is a valid tap start.
    pub fn on_press(&mut self) -> bool {
        let now = Instant::now();

        // If we had a previous release, check if we're still within the double-tap window
        if let Some(last_release) = self.last_release_time {
            if now.duration_since(last_release) > self.max_interval {
                // Too long since last tap, reset
                self.tap_count = 0;
            }
        }

        self.last_press_time = Some(now);
        self.is_pressed = true;
        true
    }

    /// Update tracker when the modifier key is released.
    /// Returns true if a double-tap was detected.
    pub fn on_release(&mut self) -> bool {
        let now = Instant::now();

        // Check if this was a quick tap (not a hold)
        if let Some(press_time) = self.last_press_time {
            let hold_duration = now.duration_since(press_time);
            if hold_duration <= self.max_hold_duration {
                // This was a tap
                self.tap_count += 1;
                self.last_release_time = Some(now);

                if self.tap_count >= 2 {
                    // Double tap detected!
                    self.reset();
                    return true;
                }
            } else {
                // Key was held too long, reset
                self.reset();
            }
        }

        self.is_pressed = false;
        false
    }

    /// Reset the tracker state
    pub fn reset(&mut self) {
        self.tap_count = 0;
        self.last_press_time = None;
        self.last_release_time = None;
        self.is_pressed = false;
    }
}

impl Default for DoubleTapTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Manages double-tap detection for multiple keys
pub struct DoubleTapManager {
    pub command_tracker: DoubleTapTracker,
    pub option_tracker: DoubleTapTracker,
    pub control_tracker: DoubleTapTracker,
    pub shift_tracker: DoubleTapTracker,
    pub escape_tracker: DoubleTapTracker,
    /// Previous modifier state to detect transitions
    prev_command: bool,
    prev_option: bool,
    prev_control: bool,
    prev_shift: bool,
}

impl DoubleTapManager {
    pub fn new() -> Self {
        Self {
            command_tracker: DoubleTapTracker::new(),
            option_tracker: DoubleTapTracker::new(),
            control_tracker: DoubleTapTracker::new(),
            shift_tracker: DoubleTapTracker::new(),
            escape_tracker: DoubleTapTracker::new(),
            prev_command: false,
            prev_option: false,
            prev_control: false,
            prev_shift: false,
        }
    }

    /// Reset all trackers except the one for the given key
    fn reset_other_trackers(&mut self, except: DoubleTapKey) {
        if except != DoubleTapKey::Command {
            self.command_tracker.reset();
        }
        if except != DoubleTapKey::Option {
            self.option_tracker.reset();
        }
        if except != DoubleTapKey::Control {
            self.control_tracker.reset();
        }
        if except != DoubleTapKey::Shift {
            self.shift_tracker.reset();
        }
        if except != DoubleTapKey::Escape {
            self.escape_tracker.reset();
        }
    }

    /// Process a FlagsChanged event (for modifier keys).
    /// Returns Some(key) if a double-tap was detected for that key.
    pub fn process_flags_changed(
        &mut self,
        command: bool,
        option: bool,
        control: bool,
        shift: bool,
    ) -> Option<DoubleTapKey> {
        let mut result = None;

        // Count how many modifiers are currently pressed
        let modifier_count = [command, option, control, shift].iter().filter(|&&x| x).count();

        // If multiple modifiers are pressed, reset all trackers
        if modifier_count > 1 {
            self.command_tracker.reset();
            self.option_tracker.reset();
            self.control_tracker.reset();
            self.shift_tracker.reset();
        } else {
            // Check Command key transitions
            if command != self.prev_command {
                if command {
                    self.reset_other_trackers(DoubleTapKey::Command);
                    self.command_tracker.on_press();
                } else if self.command_tracker.on_release() {
                    result = Some(DoubleTapKey::Command);
                }
            }

            // Check Option key transitions
            if option != self.prev_option {
                if option {
                    self.reset_other_trackers(DoubleTapKey::Option);
                    self.option_tracker.on_press();
                } else if self.option_tracker.on_release() {
                    result = Some(DoubleTapKey::Option);
                }
            }

            // Check Control key transitions
            if control != self.prev_control {
                if control {
                    self.reset_other_trackers(DoubleTapKey::Control);
                    self.control_tracker.on_press();
                } else if self.control_tracker.on_release() {
                    result = Some(DoubleTapKey::Control);
                }
            }

            // Check Shift key transitions
            if shift != self.prev_shift {
                if shift {
                    self.reset_other_trackers(DoubleTapKey::Shift);
                    self.shift_tracker.on_press();
                } else if self.shift_tracker.on_release() {
                    result = Some(DoubleTapKey::Shift);
                }
            }
        }

        self.prev_command = command;
        self.prev_option = option;
        self.prev_control = control;
        self.prev_shift = shift;

        result
    }

    /// Process a regular key event (for non-modifier keys like Escape).
    /// Returns Some(key) if a double-tap was detected.
    pub fn process_key_event(&mut self, key: DoubleTapKey, is_key_down: bool) -> Option<DoubleTapKey> {
        // Only handle Escape for now
        if key != DoubleTapKey::Escape {
            return None;
        }

        if is_key_down {
            self.reset_other_trackers(DoubleTapKey::Escape);
            self.escape_tracker.on_press();
            None
        } else if self.escape_tracker.on_release() {
            Some(DoubleTapKey::Escape)
        } else {
            None
        }
    }

    /// Reset all trackers
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.command_tracker.reset();
        self.option_tracker.reset();
        self.control_tracker.reset();
        self.shift_tracker.reset();
        self.escape_tracker.reset();
    }
}

impl Default for DoubleTapManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_double_tap_detection() {
        let mut tracker = DoubleTapTracker::new();

        // First tap
        tracker.on_press();
        assert!(!tracker.on_release()); // First tap, no double-tap yet

        // Second tap (quick)
        tracker.on_press();
        assert!(tracker.on_release()); // Double-tap detected!
    }

    #[test]
    fn test_tap_timeout() {
        let mut tracker = DoubleTapTracker::new();

        // First tap
        tracker.on_press();
        tracker.on_release();

        // Wait too long
        sleep(Duration::from_millis(350));

        // Second tap - should not count as double-tap
        tracker.on_press();
        assert!(!tracker.on_release());
    }

    #[test]
    fn test_hold_resets() {
        let mut tracker = DoubleTapTracker::new();

        // First tap
        tracker.on_press();
        tracker.on_release();

        // Hold too long
        tracker.on_press();
        sleep(Duration::from_millis(250));
        assert!(!tracker.on_release()); // Should reset due to hold
    }
}
