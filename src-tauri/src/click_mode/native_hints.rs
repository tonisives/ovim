//! Native hint rendering using macOS NSWindow
//!
//! Uses a pre-created pool of hint windows for fast activation.
//! On show, windows are repositioned and text is updated (no alloc).
//! All AppKit operations are dispatched to the main thread.

#![allow(deprecated)] // objc/cocoa crates are deprecated, but objc2 migration is future work

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

/// A pre-created hint window with its text field reference
struct PooledWindow {
    window: SendableId,
    text_field: SendableId,
}

/// Pool of pre-created hint windows ready to be shown
struct WindowPool {
    windows: Vec<PooledWindow>,
    /// How many windows from the pool are currently in use (visible)
    active_count: usize,
}

/// Global window pool
static WINDOW_POOL: Mutex<Option<WindowPool>> = Mutex::new(None);

/// Max pool size - pre-create this many windows
const POOL_SIZE: usize = 200;

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
// Pool Initialization
// ============================================================================

/// Pre-create hint windows at app startup. Call from main thread or dispatch to it.
pub fn init_pool() {
    Queue::main().exec_async(|| {
        let start = std::time::Instant::now();
        let mut pool_windows = Vec::with_capacity(POOL_SIZE);

        let style = HintStyle::default();

        for _ in 0..POOL_SIZE {
            if let Some(pw) = unsafe { create_pooled_window(&style) } {
                pool_windows.push(pw);
            }
        }

        let count = pool_windows.len();
        if let Ok(mut pool) = WINDOW_POOL.lock() {
            *pool = Some(WindowPool {
                windows: pool_windows,
                active_count: 0,
            });
        }

        log::info!(
            "[TIMING] Pre-created {} hint windows in {}ms",
            count,
            start.elapsed().as_millis()
        );
    });
}

/// Create a single pooled window (hidden, offscreen)
unsafe fn create_pooled_window(style: &HintStyle) -> Option<PooledWindow> {
    // Create offscreen, hidden
    let frame = core_graphics::geometry::CGRect::new(
        &core_graphics::geometry::CGPoint::new(-1000.0, -1000.0),
        &core_graphics::geometry::CGSize::new(30.0, style.font_size + 4.0),
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
        return None;
    }

    // Configure window properties (these don't change)
    let _: () = msg_send![window, setOpaque: false];
    let clear_color: *mut objc::runtime::Object = msg_send![class!(NSColor), clearColor];
    let _: () = msg_send![window, setBackgroundColor: clear_color];
    let _: () = msg_send![window, setLevel: 102i64];
    let _: () = msg_send![window, setIgnoresMouseEvents: true];

    let content_view: *mut objc::runtime::Object = msg_send![window, contentView];
    if content_view.is_null() {
        let _: () = msg_send![window, close];
        return None;
    }

    // Configure layer
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
    let tf_frame = core_graphics::geometry::CGRect::new(
        &core_graphics::geometry::CGPoint::new(0.0, 0.0),
        &core_graphics::geometry::CGSize::new(30.0, style.font_size + 4.0),
    );

    let text_field: *mut objc::runtime::Object = msg_send![class!(NSTextField), alloc];
    let text_field: *mut objc::runtime::Object = msg_send![text_field, initWithFrame: tf_frame];
    if text_field.is_null() {
        let _: () = msg_send![window, close];
        return None;
    }

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

    Some(PooledWindow {
        window: SendableId(window),
        text_field: SendableId(text_field),
    })
}

// ============================================================================
// Public API
// ============================================================================

/// Show native hint windows for the given elements using the pre-created pool
pub fn show_hints(elements: &[ClickableElement], _style: &HintStyle) {
    let start = std::time::Instant::now();

    let elements = elements.to_vec();
    let element_count = elements.len();

    log::info!(
        "[TIMING] show_hints prep took {}ms for {} elements",
        start.elapsed().as_millis(),
        element_count
    );

    let queued_at = std::time::Instant::now();
    Queue::main().exec_async(move || {
        let dispatch_delay = queued_at.elapsed().as_millis();
        let main_start = std::time::Instant::now();

        let screen_height = match get_primary_screen_height() {
            Some(h) => h,
            None => return,
        };

        if let Ok(mut pool) = WINDOW_POOL.lock() {
            if let Some(ref mut pool) = *pool {
                // Hide any previously active windows
                let hide_start = std::time::Instant::now();
                for i in 0..pool.active_count {
                    if i < pool.windows.len() {
                        let w = pool.windows[i].window.0;
                        if !w.is_null() {
                            unsafe {
                                let _: () = msg_send![w, orderOut: std::ptr::null::<objc::runtime::Object>()];
                            }
                        }
                    }
                }
                let hide_ms = hide_start.elapsed().as_millis();

                // Show new hints by repositioning pool windows
                let show_start = std::time::Instant::now();
                let count = elements.len().min(pool.windows.len());
                let font_size = 11.0f64;
                let hint_height = font_size + 4.0;
                let char_width = font_size * 0.75;

                for (i, element) in elements.iter().take(count).enumerate() {
                    let pw = &pool.windows[i];
                    let w = pw.window.0;
                    let tf = pw.text_field.0;
                    if w.is_null() || tf.is_null() {
                        continue;
                    }

                    let width = (element.hint.len() as f64 * char_width).max(20.0) + 8.0;
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

                    unsafe {
                        // Update text
                        let nsstring = create_nsstring(&element.hint);
                        let _: () = msg_send![tf, setStringValue: nsstring];

                        // Resize text field
                        let tf_frame = core_graphics::geometry::CGRect::new(
                            &core_graphics::geometry::CGPoint::new(0.0, 0.0),
                            &core_graphics::geometry::CGSize::new(width, hint_height),
                        );
                        let _: () = msg_send![tf, setFrame: tf_frame];

                        // Reposition and resize window
                        let frame = core_graphics::geometry::CGRect::new(
                            &core_graphics::geometry::CGPoint::new(element.x, cocoa_y),
                            &core_graphics::geometry::CGSize::new(width, hint_height),
                        );
                        let _: () = msg_send![w, setFrame: frame display: false];

                        // Show
                        let _: () = msg_send![w, orderFrontRegardless];
                    }
                }

                pool.active_count = count;

                log::info!(
                    "Showed {} hint windows from pool (screen_height={})",
                    count,
                    screen_height
                );
                log::info!(
                    "[TIMING] show_hints: dispatch_delay={}ms, hide_old={}ms, show_new={}ms, total_main={}ms",
                    dispatch_delay, hide_ms, show_start.elapsed().as_millis(), main_start.elapsed().as_millis()
                );
            } else {
                log::error!("Window pool not initialized - call init_pool() at startup");
            }
        }
    });
}

/// Hide all active hint windows (return them to pool)
pub fn hide_hints() {
    Queue::main().exec_async(|| {
        if let Ok(mut pool) = WINDOW_POOL.lock() {
            if let Some(ref mut pool) = *pool {
                let count = pool.active_count;
                for i in 0..count {
                    if i < pool.windows.len() {
                        let w = pool.windows[i].window.0;
                        if !w.is_null() {
                            unsafe {
                                let _: () = msg_send![w, orderOut: std::ptr::null::<objc::runtime::Object>()];
                            }
                        }
                    }
                }
                pool.active_count = 0;
                log::info!("Hid {} native hint windows", count);
            }
        }
    });
}

/// Update hint visibility based on input filter
pub fn filter_hints(input: &str, elements: &[ClickableElement]) {
    let input_upper = input.to_uppercase();
    let hints: Vec<String> = elements.iter().map(|e| e.hint.clone()).collect();

    Queue::main().exec_async(move || {
        if let Ok(pool) = WINDOW_POOL.lock() {
            if let Some(ref pool) = *pool {
                for (i, hint) in hints.iter().enumerate() {
                    if i < pool.windows.len() && i < pool.active_count {
                        let w = pool.windows[i].window.0;
                        if !w.is_null() {
                            let visible = input_upper.is_empty() || hint.starts_with(&input_upper);
                            set_window_visibility(w, visible);
                        }
                    }
                }
            }
        }
    });
}

/// Update hint visibility and text based on input filter
pub fn filter_hints_with_input(input: &str, elements: &[ClickableElement]) {
    let input_upper = input.to_uppercase();
    let input_len = input_upper.len();
    let hints: Vec<String> = elements.iter().map(|e| e.hint.clone()).collect();

    Queue::main().exec_async(move || {
        if let Ok(pool) = WINDOW_POOL.lock() {
            if let Some(ref pool) = *pool {
                for (i, hint) in hints.iter().enumerate() {
                    if i < pool.windows.len() && i < pool.active_count {
                        let w = pool.windows[i].window.0;
                        let tf = pool.windows[i].text_field.0;
                        if w.is_null() {
                            continue;
                        }

                        let visible = input_upper.is_empty() || hint.starts_with(&input_upper);
                        if visible {
                            set_window_visibility(w, true);
                            if input_len > 0 && hint.len() > input_len && !tf.is_null() {
                                unsafe {
                                    let nsstring = create_nsstring(&hint[input_len..]);
                                    let _: () = msg_send![tf, setStringValue: nsstring];
                                }
                            }
                        } else {
                            set_window_visibility(w, false);
                        }
                    }
                }
            }
        }
    });
}

/// Trigger shake animation on all visible hint windows
pub fn shake_hints() {
    Queue::main().exec_async(|| {
        if let Ok(pool) = WINDOW_POOL.lock() {
            if let Some(ref pool) = *pool {
                for i in 0..pool.active_count {
                    if i < pool.windows.len() {
                        let w = pool.windows[i].window.0;
                        if !w.is_null() {
                            unsafe {
                                let is_visible: bool = msg_send![w, isVisible];
                                if is_visible {
                                    animate_shake(w);
                                }
                            }
                        }
                    }
                }
            }
        }
    });
}

// ============================================================================
// Helpers
// ============================================================================

fn get_primary_screen_height() -> Option<f64> {
    unsafe {
        let screens: *mut objc::runtime::Object = msg_send![class!(NSScreen), screens];
        if screens.is_null() {
            return None;
        }

        let count: usize = msg_send![screens, count];
        if count == 0 {
            return None;
        }

        let primary_screen: *mut objc::runtime::Object = msg_send![screens, objectAtIndex: 0usize];
        if primary_screen.is_null() {
            return None;
        }

        let screen_frame: core_graphics::geometry::CGRect = msg_send![primary_screen, frame];
        Some(screen_frame.size.height)
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

unsafe fn create_nsstring(s: &str) -> *mut objc::runtime::Object {
    let nsstring: *mut objc::runtime::Object = msg_send![class!(NSString), alloc];
    let bytes = s.as_ptr();
    let len = s.len();
    msg_send![nsstring, initWithBytes: bytes length: len encoding: 4u64]
}
