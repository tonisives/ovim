use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use core_foundation::runloop::{kCFRunLoopDefaultMode, CFRunLoop};
use core_graphics::event::{
    CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventTapProxy, CGEventType, EventField, CallbackResult,
};

/// Mouse event callback type - called for mouse click events
/// Return true to pass through, false to suppress (though suppressing mouse events is rare)
pub type MouseEventCallback = Box<dyn Fn(MouseClickEvent) -> bool + Send + 'static>;

/// Scroll event callback type - called for scroll wheel events
pub type ScrollEventCallback = Box<dyn Fn() + Send + 'static>;

/// Represents a mouse click event
#[derive(Debug, Clone, Copy)]
pub struct MouseClickEvent {
    pub is_left_click: bool,
    pub is_right_click: bool,
}

use super::inject::INJECTED_EVENT_MARKER;
use super::keycode::{KeyEvent, Modifiers};

pub type KeyEventCallback = Box<dyn Fn(KeyEvent) -> Option<KeyEvent> + Send + 'static>;

/// Helper to compare CGEventType (which doesn't implement PartialEq)
fn is_event_type(event_type: CGEventType, expected: CGEventType) -> bool {
    (event_type as u32) == (expected as u32)
}

/// Keyboard capture using CGEventTap
pub struct KeyboardCapture {
    callback: Arc<Mutex<Option<KeyEventCallback>>>,
    mouse_callback: Arc<Mutex<Option<MouseEventCallback>>>,
    scroll_callback: Arc<Mutex<Option<ScrollEventCallback>>>,
    running: Arc<Mutex<bool>>,
}

impl KeyboardCapture {
    pub fn new() -> Self {
        Self {
            callback: Arc::new(Mutex::new(None)),
            mouse_callback: Arc::new(Mutex::new(None)),
            scroll_callback: Arc::new(Mutex::new(None)),
            running: Arc::new(Mutex::new(false)),
        }
    }

    /// Set the callback for key events
    /// Return Some(event) to pass through (possibly modified)
    /// Return None to suppress the event
    pub fn set_callback<F>(&self, callback: F)
    where
        F: Fn(KeyEvent) -> Option<KeyEvent> + Send + 'static,
    {
        let mut cb = self.callback.lock().unwrap();
        *cb = Some(Box::new(callback));
    }

    /// Set the callback for mouse click events
    /// Return true to pass through, false to suppress
    pub fn set_mouse_callback<F>(&self, callback: F)
    where
        F: Fn(MouseClickEvent) -> bool + Send + 'static,
    {
        let mut cb = self.mouse_callback.lock().unwrap();
        *cb = Some(Box::new(callback));
    }

    /// Set the callback for scroll events
    pub fn set_scroll_callback<F>(&self, callback: F)
    where
        F: Fn() + Send + 'static,
    {
        let mut cb = self.scroll_callback.lock().unwrap();
        *cb = Some(Box::new(callback));
    }

    /// Start capturing keyboard events
    /// This spawns a new thread with its own run loop
    pub fn start(&self) -> Result<(), String> {
        let mut running = self.running.lock().unwrap();
        if *running {
            return Ok(());
        }
        *running = true;
        drop(running);

        let callback = Arc::clone(&self.callback);
        let mouse_callback = Arc::clone(&self.mouse_callback);
        let scroll_callback = Arc::clone(&self.scroll_callback);
        let running_flag = Arc::clone(&self.running);

        // Flag to signal that tap needs re-enabling
        let needs_reenable = Arc::new(AtomicBool::new(false));
        let needs_reenable_for_callback = Arc::clone(&needs_reenable);

        thread::spawn(move || {
            // Create the event tap - use HID tap location for reliable key suppression
            // Also listen for mouse down events to detect clicks
            let tap = CGEventTap::new(
                CGEventTapLocation::HID,
                CGEventTapPlacement::HeadInsertEventTap,
                CGEventTapOptions::Default,
                vec![
                    CGEventType::KeyDown,
                    CGEventType::KeyUp,
                    CGEventType::FlagsChanged,
                    CGEventType::LeftMouseDown,
                    CGEventType::RightMouseDown,
                    CGEventType::ScrollWheel,
                ],
                move |_proxy: CGEventTapProxy, event_type: CGEventType, event| -> CallbackResult {
                    // Handle tap disabled by timeout - signal re-enable
                    if is_event_type(event_type, CGEventType::TapDisabledByTimeout) {
                        log::warn!("CGEventTap was disabled by timeout, signaling re-enable...");
                        needs_reenable_for_callback.store(true, Ordering::SeqCst);
                        return CallbackResult::Keep;
                    }

                    // Handle tap disabled by user - also re-enable
                    if is_event_type(event_type, CGEventType::TapDisabledByUserInput) {
                        log::warn!("CGEventTap was disabled by user input, signaling re-enable...");
                        needs_reenable_for_callback.store(true, Ordering::SeqCst);
                        return CallbackResult::Keep;
                    }

                    // Handle mouse click events
                    if is_event_type(event_type, CGEventType::LeftMouseDown)
                        || is_event_type(event_type, CGEventType::RightMouseDown)
                    {
                        let mouse_event = MouseClickEvent {
                            is_left_click: is_event_type(event_type, CGEventType::LeftMouseDown),
                            is_right_click: is_event_type(event_type, CGEventType::RightMouseDown),
                        };
                        let cb_lock = mouse_callback.lock().unwrap();
                        if let Some(ref cb) = *cb_lock {
                            cb(mouse_event);
                        }
                        // Always pass through mouse events
                        return CallbackResult::Keep;
                    }

                    // Handle scroll wheel events
                    if is_event_type(event_type, CGEventType::ScrollWheel) {
                        let cb_lock = scroll_callback.lock().unwrap();
                        if let Some(ref cb) = *cb_lock {
                            cb();
                        }
                        // Always pass through scroll events
                        return CallbackResult::Keep;
                    }

                    // Skip events we injected ourselves
                    let user_data = event.get_integer_value_field(EventField::EVENT_SOURCE_USER_DATA);
                    if user_data == INJECTED_EVENT_MARKER {
                        log::trace!("Skipping injected event");
                        return CallbackResult::Keep;
                    }

                    // Skip FlagsChanged events (modifier key changes) - pass through
                    if is_event_type(event_type, CGEventType::FlagsChanged) {
                        return CallbackResult::Keep;
                    }

                    // Get key code and flags
                    let keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
                    log::trace!("Key event: keycode={}, type={:?}", keycode, event_type);
                    let flags = event.get_flags();
                    let is_key_down = is_event_type(event_type, CGEventType::KeyDown);

                    let key_event = KeyEvent {
                        code: keycode,
                        modifiers: Modifiers::from_cg_flags(flags.bits()),
                        is_key_down,
                    };

                    // Call user callback
                    let cb_lock = callback.lock().unwrap();
                    if let Some(ref cb) = *cb_lock {
                        match cb(key_event) {
                            Some(_modified_event) => {
                                // Pass through
                                log::trace!("capture: passing through keycode={}", keycode);
                                CallbackResult::Keep
                            }
                            None => {
                                // Suppress the event - use Drop to return null_ptr
                                log::trace!("capture: SUPPRESSING keycode={}", keycode);
                                CallbackResult::Drop
                            }
                        }
                    } else {
                        // No callback set, pass through
                        log::trace!("capture: no callback, passing through keycode={}", keycode);
                        CallbackResult::Keep
                    }
                },
            );

            match tap {
                Ok(tap) => {
                    // Create run loop source and add to run loop
                    let loop_source = tap
                        .mach_port()
                        .create_runloop_source(0)
                        .expect("Failed to create run loop source");

                    let run_loop = CFRunLoop::get_current();
                    unsafe {
                        run_loop.add_source(&loop_source, kCFRunLoopDefaultMode);
                    }

                    // Enable the tap
                    tap.enable();

                    log::info!("CGEventTap started successfully");

                    // Run the loop
                    while *running_flag.lock().unwrap() {
                        // Check if tap needs re-enabling
                        if needs_reenable.swap(false, Ordering::SeqCst) {
                            log::info!("Re-enabling CGEventTap...");
                            tap.enable();
                        }

                        CFRunLoop::run_in_mode(
                            unsafe { kCFRunLoopDefaultMode },
                            Duration::from_millis(100),
                            false,
                        );
                    }

                    log::info!("CGEventTap stopped");
                }
                Err(()) => {
                    log::error!(
                        "Failed to create CGEventTap. Make sure Input Monitoring permission is granted."
                    );
                    *running_flag.lock().unwrap() = false;
                }
            }
        });

        Ok(())
    }

    /// Stop capturing keyboard events
    pub fn stop(&self) {
        let mut running = self.running.lock().unwrap();
        *running = false;
    }

    /// Check if currently capturing
    pub fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }
}

impl Default for KeyboardCapture {
    fn default() -> Self {
        Self::new()
    }
}
