//! Native hint rendering using macOS NSWindow
//!
//! Creates small native windows for each hint label, positioned at element locations.
//! All AppKit operations are dispatched to the main thread.

#![allow(deprecated)]

use core_foundation::base::CFTypeRef;
use dispatch::Queue;
use objc::{class, msg_send, sel, sel_impl};
use std::sync::Mutex;

use super::element::ClickableElement;

/// Wrapper to make id Send + Sync
struct SendableId(*mut objc::runtime::Object);
unsafe impl Send for SendableId {}
unsafe impl Sync for SendableId {}

/// Store for active hint windows - use a separate mutex for window storage
static HINT_WINDOWS: Mutex<Vec<SendableId>> = Mutex::new(Vec::new());

/// Style settings for hint windows
#[derive(Clone)]
pub struct HintStyle {
    pub font_size: f64,
    pub bg_color: (f64, f64, f64), // RGB 0-1
    pub text_color: (f64, f64, f64), // RGB 0-1
    pub opacity: f64,
}

impl Default for HintStyle {
    fn default() -> Self {
        Self {
            font_size: 11.0,
            bg_color: (1.0, 0.8, 0.0), // Yellow
            text_color: (0.0, 0.0, 0.0), // Black
            opacity: 0.95,
        }
    }
}

/// Show native hint windows for the given elements
/// This dispatches to the main thread
pub fn show_hints(elements: &[ClickableElement], style: &HintStyle) {
    let start = std::time::Instant::now();

    // First, collect windows to close
    let windows_to_close: Vec<SendableId> = {
        match HINT_WINDOWS.try_lock() {
            Ok(mut w) => w.drain(..).collect(),
            Err(_) => {
                log::warn!("Could not lock HINT_WINDOWS for clearing");
                return;
            }
        }
    };

    let elements = elements.to_vec();
    let style = style.clone();
    let element_count = elements.len();

    log::info!("[TIMING] show_hints prep took {}ms for {} elements", start.elapsed().as_millis(), element_count);

    Queue::main().exec_async(move || {
        let main_start = std::time::Instant::now();

        // First close old windows
        unsafe {
            for SendableId(window) in windows_to_close {
                if !window.is_null() {
                    let _: () = msg_send![window, orderOut: std::ptr::null::<objc::runtime::Object>()];
                    let _: () = msg_send![window, close];
                }
            }
        }

        // Then create new ones
        show_hints_on_main_thread(&elements, &style);

        log::info!("[TIMING] show_hints main thread took {}ms", main_start.elapsed().as_millis());
    });
}

/// Actually create the hint windows (runs on main thread)
fn show_hints_on_main_thread(elements: &[ClickableElement], style: &HintStyle) {
    let mut windows = match HINT_WINDOWS.try_lock() {
        Ok(w) => w,
        Err(_) => {
            log::error!("Failed to lock HINT_WINDOWS for creating");
            return;
        }
    };

    unsafe {
        // Get the PRIMARY screen (the one with the menu bar, at Cocoa origin 0,0)
        // NOT mainScreen which returns the screen with the key window!
        // The primary screen is always screens[0] in the NSScreen.screens array.
        let screens: *mut objc::runtime::Object = msg_send![class!(NSScreen), screens];
        if screens.is_null() {
            log::error!("Failed to get screens");
            return;
        }
        let count: usize = msg_send![screens, count];
        if count == 0 {
            log::error!("No screens found");
            return;
        }
        let primary_screen: *mut objc::runtime::Object = msg_send![screens, objectAtIndex: 0usize];
        if primary_screen.is_null() {
            log::error!("Failed to get primary screen");
            return;
        }
        let screen_frame: core_graphics::geometry::CGRect = msg_send![primary_screen, frame];
        let screen_height = screen_frame.size.height;

        for element in elements {
            // AXPosition gives top-left in screen coordinates (origin at top-left of main screen)
            // Cocoa uses bottom-left origin for the main screen
            //
            // For main screen (0,0 at top-left in AX):
            //   cocoa_y = screen_height - ax_y - hint_height
            //
            // The main screen's Cocoa origin is (0, 0) at bottom-left.
            // AX origin is (0, 0) at top-left of main screen.
            // So: cocoa_y = main_screen_height - ax_y - hint_height
            //
            // This works for ALL screens because Cocoa coordinates are relative to
            // the main screen's bottom-left corner, and AX coordinates are relative to
            // the main screen's top-left corner.
            let hint_height = style.font_size + 4.0;
            let cocoa_y = screen_height - element.y - hint_height;

            // Log first 3 hints at info level for debugging
            if windows.len() < 3 {
                log::info!(
                    "Hint '{}' at AX({}, {}) -> Cocoa({}, {})",
                    element.hint, element.x, element.y, element.x, cocoa_y
                );
            }

            match create_hint_window(element.x, cocoa_y, &element.hint, style) {
                Some(window) => windows.push(SendableId(window)),
                None => {} // Silently skip failures
            }
        }

        log::info!(
            "Created {} native hint windows (main_screen_height={})",
            windows.len(),
            screen_height
        );
    }
}

/// Hide and release all hint windows
pub fn hide_hints() {
    // Try to lock, but don't block if we can't
    let windows_to_close: Vec<SendableId> = match HINT_WINDOWS.try_lock() {
        Ok(mut w) => w.drain(..).collect(),
        Err(_) => {
            log::warn!("Could not lock HINT_WINDOWS for hiding");
            return;
        }
    };

    if windows_to_close.is_empty() {
        return;
    }

    let count = windows_to_close.len();

    Queue::main().exec_async(move || {
        unsafe {
            for SendableId(window) in windows_to_close {
                if !window.is_null() {
                    let _: () = msg_send![window, orderOut: std::ptr::null::<objc::runtime::Object>()];
                    let _: () = msg_send![window, close];
                }
            }
        }
        log::info!("Hid {} native hint windows", count);
    });
}

/// Create a single hint window at the specified position
unsafe fn create_hint_window(
    x: f64,
    y: f64,
    hint: &str,
    style: &HintStyle,
) -> Option<*mut objc::runtime::Object> {
    // Calculate window size based on hint text length
    // Use a wider char_width to ensure text fits
    let char_width = style.font_size * 0.75;
    let width = (hint.len() as f64 * char_width).max(20.0) + 8.0;
    let height = style.font_size + 4.0;

    let frame = core_graphics::geometry::CGRect::new(
        &core_graphics::geometry::CGPoint::new(x, y),
        &core_graphics::geometry::CGSize::new(width, height),
    );

    // Create borderless window
    let window_class = class!(NSWindow);
    let window: *mut objc::runtime::Object = msg_send![window_class, alloc];
    if window.is_null() {
        return None;
    }

    let window: *mut objc::runtime::Object = msg_send![
        window,
        initWithContentRect: frame
        styleMask: 0u64  // Borderless
        backing: 2u64    // NSBackingStoreBuffered
        defer: true      // Defer creation for speed
    ];

    if window.is_null() {
        return None;
    }

    // Minimal window configuration for speed
    let _: () = msg_send![window, setOpaque: false];
    let clear_color: *mut objc::runtime::Object = msg_send![class!(NSColor), clearColor];
    let _: () = msg_send![window, setBackgroundColor: clear_color];
    // Use a very high window level to appear above context menus
    // NSPopUpMenuWindowLevel is 101, NSScreenSaverWindowLevel is 1000
    // We use 102 to be just above popup menus
    let _: () = msg_send![window, setLevel: 102i64];
    let _: () = msg_send![window, setIgnoresMouseEvents: true];

    // Get content view and set up layer
    let content_view: *mut objc::runtime::Object = msg_send![window, contentView];
    if content_view.is_null() {
        let _: () = msg_send![window, close];
        return None;
    }

    let _: () = msg_send![content_view, setWantsLayer: true];

    let layer: *mut objc::runtime::Object = msg_send![content_view, layer];
    if !layer.is_null() {
        let bg_color: *mut objc::runtime::Object = msg_send![
            class!(NSColor),
            colorWithRed: style.bg_color.0
            green: style.bg_color.1
            blue: style.bg_color.2
            alpha: style.opacity
        ];
        let cg_color: CFTypeRef = msg_send![bg_color, CGColor];
        let _: () = msg_send![layer, setBackgroundColor: cg_color];
        let _: () = msg_send![layer, setCornerRadius: 2.0f64];
        let _: () = msg_send![layer, setBorderWidth: 0.5f64];
        let border_color: *mut objc::runtime::Object = msg_send![
            class!(NSColor),
            colorWithRed: 0.0f64
            green: 0.0f64
            blue: 0.0f64
            alpha: 0.2f64
        ];
        let cg_border: CFTypeRef = msg_send![border_color, CGColor];
        let _: () = msg_send![layer, setBorderColor: cg_border];
    }

    // Create text field
    let text_frame = core_graphics::geometry::CGRect::new(
        &core_graphics::geometry::CGPoint::new(0.0, 0.0),
        &core_graphics::geometry::CGSize::new(width, height),
    );

    let text_field: *mut objc::runtime::Object = msg_send![class!(NSTextField), alloc];
    let text_field: *mut objc::runtime::Object = msg_send![text_field, initWithFrame: text_frame];

    if !text_field.is_null() {
        let hint_nsstring = create_nsstring(hint);
        let _: () = msg_send![text_field, setStringValue: hint_nsstring];
        let _: () = msg_send![text_field, setBezeled: false];
        let _: () = msg_send![text_field, setDrawsBackground: false];
        let _: () = msg_send![text_field, setEditable: false];
        let _: () = msg_send![text_field, setSelectable: false];
        let _: () = msg_send![text_field, setAlignment: 2u64]; // Center

        // Use system font for speed (no font lookup)
        let font: *mut objc::runtime::Object =
            msg_send![class!(NSFont), boldSystemFontOfSize: style.font_size];
        if !font.is_null() {
            let _: () = msg_send![text_field, setFont: font];
        }

        let text_color: *mut objc::runtime::Object = msg_send![
            class!(NSColor),
            colorWithRed: style.text_color.0
            green: style.text_color.1
            blue: style.text_color.2
            alpha: 1.0f64
        ];
        let _: () = msg_send![text_field, setTextColor: text_color];
        let _: () = msg_send![content_view, addSubview: text_field];
    }

    // Show the window
    let _: () = msg_send![window, orderFrontRegardless];

    Some(window)
}

/// Create an NSString from a Rust string
unsafe fn create_nsstring(s: &str) -> *mut objc::runtime::Object {
    let nsstring: *mut objc::runtime::Object = msg_send![class!(NSString), alloc];
    let bytes = s.as_ptr();
    let len = s.len();
    msg_send![nsstring, initWithBytes: bytes length: len encoding: 4u64]
}

/// Update hint visibility based on input filter
/// Dispatches to main thread for thread safety
pub fn filter_hints(input: &str, elements: &[ClickableElement]) {
    let input_upper = input.to_uppercase();
    let hints: Vec<String> = elements.iter().map(|e| e.hint.clone()).collect();

    Queue::main().exec_async(move || {
        let windows = match HINT_WINDOWS.try_lock() {
            Ok(w) => w,
            Err(_) => return,
        };

        unsafe {
            for (i, SendableId(window)) in windows.iter().enumerate() {
                if i < hints.len() && !window.is_null() {
                    let hint = &hints[i];
                    let visible = input_upper.is_empty() || hint.starts_with(&input_upper);

                    if visible {
                        let _: () = msg_send![*window, orderFrontRegardless];
                    } else {
                        let _: () =
                            msg_send![*window, orderOut: std::ptr::null::<objc::runtime::Object>()];
                    }
                }
            }
        }
    });
}
