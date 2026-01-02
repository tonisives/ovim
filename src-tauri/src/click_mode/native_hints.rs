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

// ============================================================================
// Types
// ============================================================================

/// Wrapper to make id Send + Sync
struct SendableId(*mut objc::runtime::Object);
unsafe impl Send for SendableId {}
unsafe impl Sync for SendableId {}

/// Store for active hint windows
static HINT_WINDOWS: Mutex<Vec<SendableId>> = Mutex::new(Vec::new());

/// Style settings for hint windows
#[derive(Clone)]
pub struct HintStyle {
    pub font_size: f64,
    pub bg_color: (f64, f64, f64),
    pub text_color: (f64, f64, f64),
    pub opacity: f64,
}

impl Default for HintStyle {
    fn default() -> Self {
        Self {
            font_size: 11.0,
            bg_color: (1.0, 0.8, 0.0),
            text_color: (0.0, 0.0, 0.0),
            opacity: 0.95,
        }
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Show native hint windows for the given elements
pub fn show_hints(elements: &[ClickableElement], style: &HintStyle) {
    let start = std::time::Instant::now();

    let windows_to_close = drain_windows();
    if windows_to_close.is_none() {
        return;
    }
    let windows_to_close = windows_to_close.unwrap();

    let elements = elements.to_vec();
    let style = style.clone();
    let element_count = elements.len();

    log::info!(
        "[TIMING] show_hints prep took {}ms for {} elements",
        start.elapsed().as_millis(),
        element_count
    );

    Queue::main().exec_async(move || {
        let main_start = std::time::Instant::now();
        close_windows(windows_to_close);
        create_hint_windows(&elements, &style);
        log::info!(
            "[TIMING] show_hints main thread took {}ms",
            main_start.elapsed().as_millis()
        );
    });
}

/// Hide and release all hint windows
pub fn hide_hints() {
    let windows_to_close = match drain_windows() {
        Some(w) if !w.is_empty() => w,
        _ => return,
    };

    let count = windows_to_close.len();
    Queue::main().exec_async(move || {
        close_windows(windows_to_close);
        log::info!("Hid {} native hint windows", count);
    });
}

/// Update hint visibility based on input filter
pub fn filter_hints(input: &str, elements: &[ClickableElement]) {
    let input_upper = input.to_uppercase();
    let hints: Vec<String> = elements.iter().map(|e| e.hint.clone()).collect();

    Queue::main().exec_async(move || {
        with_windows(|windows| {
            for (i, SendableId(window)) in windows.iter().enumerate() {
                if i < hints.len() && !window.is_null() {
                    let visible = input_upper.is_empty() || hints[i].starts_with(&input_upper);
                    set_window_visibility(*window, visible);
                }
            }
        });
    });
}

/// Update hint visibility and text based on input filter
pub fn filter_hints_with_input(input: &str, elements: &[ClickableElement]) {
    let input_upper = input.to_uppercase();
    let input_len = input_upper.len();
    let hints: Vec<String> = elements.iter().map(|e| e.hint.clone()).collect();

    Queue::main().exec_async(move || {
        with_windows(|windows| {
            for (i, SendableId(window)) in windows.iter().enumerate() {
                if i < hints.len() && !window.is_null() {
                    let hint = &hints[i];
                    let visible = input_upper.is_empty() || hint.starts_with(&input_upper);

                    if visible {
                        set_window_visibility(*window, true);
                        if input_len > 0 && hint.len() > input_len {
                            unsafe { update_window_text(*window, &hint[input_len..]) };
                        }
                    } else {
                        set_window_visibility(*window, false);
                    }
                }
            }
        });
    });
}

/// Trigger shake animation on all visible hint windows
pub fn shake_hints() {
    Queue::main().exec_async(|| {
        with_windows(|windows| {
            for SendableId(window) in windows.iter() {
                if !window.is_null() {
                    unsafe {
                        let is_visible: bool = msg_send![*window, isVisible];
                        if is_visible {
                            animate_shake(*window);
                        }
                    }
                }
            }
        });
    });
}

// ============================================================================
// Window Management Helpers
// ============================================================================

fn drain_windows() -> Option<Vec<SendableId>> {
    match HINT_WINDOWS.try_lock() {
        Ok(mut w) => Some(w.drain(..).collect()),
        Err(_) => {
            log::warn!("Could not lock HINT_WINDOWS");
            None
        }
    }
}

fn with_windows<F>(f: F)
where
    F: FnOnce(&Vec<SendableId>),
{
    if let Ok(windows) = HINT_WINDOWS.try_lock() {
        f(&windows);
    }
}

fn close_windows(windows: Vec<SendableId>) {
    unsafe {
        for SendableId(window) in windows {
            if !window.is_null() {
                let _: () = msg_send![window, orderOut: std::ptr::null::<objc::runtime::Object>()];
                let _: () = msg_send![window, close];
            }
        }
    }
}

fn set_window_visibility(window: *mut objc::runtime::Object, visible: bool) {
    unsafe {
        if visible {
            let _: () = msg_send![window, orderFrontRegardless];
        } else {
            let _: () = msg_send![window, orderOut: std::ptr::null::<objc::runtime::Object>()];
        }
    }
}

// ============================================================================
// Window Creation
// ============================================================================

/// SAFETY: This function is only called from the main thread via `Queue::main().exec_async`
fn create_hint_windows(elements: &[ClickableElement], style: &HintStyle) {
    let mut windows = match HINT_WINDOWS.try_lock() {
        Ok(w) => w,
        Err(_) => {
            log::error!("Failed to lock HINT_WINDOWS for creating");
            return;
        }
    };

    let screen_height = match get_primary_screen_height() {
        Some(h) => h,
        None => return,
    };

    for (i, element) in elements.iter().enumerate() {
        let hint_height = style.font_size + 4.0;
        let cocoa_y = screen_height - element.y - hint_height;

        if i < 3 {
            log::info!(
                "Hint '{}' at AX({}, {}) -> Cocoa({}, {})",
                element.hint,
                element.x,
                element.y,
                element.x,
                cocoa_y
            );
        }

        // SAFETY: We're creating windows on the main thread
        if let Some(window) = unsafe { create_hint_window(element.x, cocoa_y, &element.hint, style) } {
            windows.push(SendableId(window));
        }
    }

    log::info!(
        "Created {} native hint windows (screen_height={})",
        windows.len(),
        screen_height
    );
}

fn get_primary_screen_height() -> Option<f64> {
    unsafe {
        let screens: *mut objc::runtime::Object = msg_send![class!(NSScreen), screens];
        if screens.is_null() {
            log::error!("Failed to get screens");
            return None;
        }

        let count: usize = msg_send![screens, count];
        if count == 0 {
            log::error!("No screens found");
            return None;
        }

        let primary_screen: *mut objc::runtime::Object = msg_send![screens, objectAtIndex: 0usize];
        if primary_screen.is_null() {
            log::error!("Failed to get primary screen");
            return None;
        }

        let screen_frame: core_graphics::geometry::CGRect = msg_send![primary_screen, frame];
        Some(screen_frame.size.height)
    }
}

unsafe fn create_hint_window(
    x: f64,
    y: f64,
    hint: &str,
    style: &HintStyle,
) -> Option<*mut objc::runtime::Object> {
    let char_width = style.font_size * 0.75;
    let width = (hint.len() as f64 * char_width).max(20.0) + 8.0;
    let height = style.font_size + 4.0;

    let window = create_borderless_window(x, y, width, height)?;
    configure_window(window);

    let content_view = get_content_view(window)?;
    configure_layer(content_view, style);
    add_text_field(content_view, hint, width, height, style);

    let _: () = msg_send![window, orderFrontRegardless];
    Some(window)
}

unsafe fn create_borderless_window(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Option<*mut objc::runtime::Object> {
    let frame = core_graphics::geometry::CGRect::new(
        &core_graphics::geometry::CGPoint::new(x, y),
        &core_graphics::geometry::CGSize::new(width, height),
    );

    let window: *mut objc::runtime::Object = msg_send![class!(NSWindow), alloc];
    if window.is_null() {
        return None;
    }

    let window: *mut objc::runtime::Object = msg_send![
        window,
        initWithContentRect: frame
        styleMask: 0u64
        backing: 2u64
        defer: true
    ];

    if window.is_null() {
        None
    } else {
        Some(window)
    }
}

unsafe fn configure_window(window: *mut objc::runtime::Object) {
    let _: () = msg_send![window, setOpaque: false];
    let clear_color: *mut objc::runtime::Object = msg_send![class!(NSColor), clearColor];
    let _: () = msg_send![window, setBackgroundColor: clear_color];
    let _: () = msg_send![window, setLevel: 102i64];
    let _: () = msg_send![window, setIgnoresMouseEvents: true];
}

unsafe fn get_content_view(window: *mut objc::runtime::Object) -> Option<*mut objc::runtime::Object> {
    let content_view: *mut objc::runtime::Object = msg_send![window, contentView];
    if content_view.is_null() {
        let _: () = msg_send![window, close];
        None
    } else {
        Some(content_view)
    }
}

unsafe fn configure_layer(content_view: *mut objc::runtime::Object, style: &HintStyle) {
    let _: () = msg_send![content_view, setWantsLayer: true];

    let layer: *mut objc::runtime::Object = msg_send![content_view, layer];
    if layer.is_null() {
        return;
    }

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

unsafe fn add_text_field(
    content_view: *mut objc::runtime::Object,
    text: &str,
    width: f64,
    height: f64,
    style: &HintStyle,
) {
    let frame = core_graphics::geometry::CGRect::new(
        &core_graphics::geometry::CGPoint::new(0.0, 0.0),
        &core_graphics::geometry::CGSize::new(width, height),
    );

    let text_field: *mut objc::runtime::Object = msg_send![class!(NSTextField), alloc];
    let text_field: *mut objc::runtime::Object = msg_send![text_field, initWithFrame: frame];

    if text_field.is_null() {
        return;
    }

    let nsstring = create_nsstring(text);
    let _: () = msg_send![text_field, setStringValue: nsstring];
    let _: () = msg_send![text_field, setBezeled: false];
    let _: () = msg_send![text_field, setDrawsBackground: false];
    let _: () = msg_send![text_field, setEditable: false];
    let _: () = msg_send![text_field, setSelectable: false];
    let _: () = msg_send![text_field, setAlignment: 2u64];

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

// ============================================================================
// Text Updates & Animation
// ============================================================================

unsafe fn update_window_text(window: *mut objc::runtime::Object, text: &str) {
    let content_view: *mut objc::runtime::Object = msg_send![window, contentView];
    if content_view.is_null() {
        return;
    }

    let subviews: *mut objc::runtime::Object = msg_send![content_view, subviews];
    if subviews.is_null() {
        return;
    }

    let count: usize = msg_send![subviews, count];
    for i in 0..count {
        let subview: *mut objc::runtime::Object = msg_send![subviews, objectAtIndex: i];
        if subview.is_null() {
            continue;
        }

        let class_name: *mut objc::runtime::Object = msg_send![subview, className];
        if class_name.is_null() {
            continue;
        }

        let utf8: *const std::os::raw::c_char = msg_send![class_name, UTF8String];
        if utf8.is_null() {
            continue;
        }

        let name = std::ffi::CStr::from_ptr(utf8).to_string_lossy();
        if name == "NSTextField" {
            let nsstring = create_nsstring(text);
            let _: () = msg_send![subview, setStringValue: nsstring];
            break;
        }
    }
}

unsafe fn animate_shake(window: *mut objc::runtime::Object) {
    let content_view: *mut objc::runtime::Object = msg_send![window, contentView];
    if content_view.is_null() {
        return;
    }

    let layer: *mut objc::runtime::Object = msg_send![content_view, layer];
    if layer.is_null() {
        return;
    }

    let animation: *mut objc::runtime::Object = msg_send![
        class!(CAKeyframeAnimation),
        animationWithKeyPath: create_nsstring("transform.translation.x")
    ];

    if animation.is_null() {
        return;
    }

    let values: *mut objc::runtime::Object = msg_send![class!(NSMutableArray), array];
    for &v in &[0.0, 4.0, -4.0, 3.0, -3.0, 0.0] {
        let num: *mut objc::runtime::Object = msg_send![class!(NSNumber), numberWithDouble: v];
        let _: () = msg_send![values, addObject: num];
    }
    let _: () = msg_send![animation, setValues: values];
    let _: () = msg_send![animation, setDuration: 0.25f64];
    let _: () = msg_send![layer, addAnimation: animation forKey: create_nsstring("shake")];
}

// ============================================================================
// Utilities
// ============================================================================

unsafe fn create_nsstring(s: &str) -> *mut objc::runtime::Object {
    let nsstring: *mut objc::runtime::Object = msg_send![class!(NSString), alloc];
    let bytes = s.as_ptr();
    let len = s.len();
    msg_send![nsstring, initWithBytes: bytes length: len encoding: 4u64]
}
