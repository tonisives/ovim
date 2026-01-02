//! Mouse click simulation for Click Mode
//!
//! Provides functions to simulate various mouse clicks at screen positions.

use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, CGMouseButton};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;

/// Perform a left-click at a specific position
pub fn click_at(x: f64, y: f64) -> Result<(), String> {
    log::info!("Performing mouse click at position ({}, {})", x, y);

    let point = CGPoint::new(x, y);
    let source = create_event_source()?;

    post_mouse_event(&source, CGEventType::LeftMouseDown, point, CGMouseButton::Left)?;
    std::thread::sleep(std::time::Duration::from_millis(10));
    post_mouse_event(&source, CGEventType::LeftMouseUp, point, CGMouseButton::Left)?;

    log::info!("Mouse click completed");
    Ok(())
}

/// Perform a right-click at a specific position
pub fn right_click_at(x: f64, y: f64) -> Result<(), String> {
    log::info!("Performing right-click at position ({}, {})", x, y);

    let point = CGPoint::new(x, y);
    let source = create_event_source()?;

    post_mouse_event(&source, CGEventType::RightMouseDown, point, CGMouseButton::Right)?;
    std::thread::sleep(std::time::Duration::from_millis(10));
    post_mouse_event(&source, CGEventType::RightMouseUp, point, CGMouseButton::Right)?;

    log::info!("Right-click completed");
    Ok(())
}

/// Perform a double-click at a specific position
pub fn double_click_at(x: f64, y: f64) -> Result<(), String> {
    log::info!("Performing double-click at position ({}, {})", x, y);

    let point = CGPoint::new(x, y);
    let source = create_event_source()?;

    // First click
    let mouse_down1 = create_mouse_event(&source, CGEventType::LeftMouseDown, point)?;
    mouse_down1.set_integer_value_field(
        core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE,
        1,
    );
    mouse_down1.post(CGEventTapLocation::HID);

    std::thread::sleep(std::time::Duration::from_millis(10));

    let mouse_up1 = create_mouse_event(&source, CGEventType::LeftMouseUp, point)?;
    mouse_up1.set_integer_value_field(
        core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE,
        1,
    );
    mouse_up1.post(CGEventTapLocation::HID);

    std::thread::sleep(std::time::Duration::from_millis(50));

    // Second click (click count = 2)
    let mouse_down2 = create_mouse_event(&source, CGEventType::LeftMouseDown, point)?;
    mouse_down2.set_integer_value_field(
        core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE,
        2,
    );
    mouse_down2.post(CGEventTapLocation::HID);

    std::thread::sleep(std::time::Duration::from_millis(10));

    let mouse_up2 = create_mouse_event(&source, CGEventType::LeftMouseUp, point)?;
    mouse_up2.set_integer_value_field(
        core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE,
        2,
    );
    mouse_up2.post(CGEventTapLocation::HID);

    log::info!("Double-click completed");
    Ok(())
}

/// Perform a Cmd+click at a specific position
pub fn cmd_click_at(x: f64, y: f64) -> Result<(), String> {
    log::info!("Performing Cmd+click at position ({}, {})", x, y);

    let point = CGPoint::new(x, y);
    let source = create_event_source()?;

    // Mouse down with Cmd modifier
    let mouse_down = create_mouse_event(&source, CGEventType::LeftMouseDown, point)?;
    mouse_down.set_flags(CGEventFlags::CGEventFlagCommand);
    mouse_down.post(CGEventTapLocation::HID);

    std::thread::sleep(std::time::Duration::from_millis(10));

    // Mouse up with Cmd modifier
    let mouse_up = create_mouse_event(&source, CGEventType::LeftMouseUp, point)?;
    mouse_up.set_flags(CGEventFlags::CGEventFlagCommand);
    mouse_up.post(CGEventTapLocation::HID);

    log::info!("Cmd+click completed");
    Ok(())
}

// Helper functions

fn create_event_source() -> Result<CGEventSource, String> {
    CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "Could not create event source".to_string())
}

fn create_mouse_event(
    source: &CGEventSource,
    event_type: CGEventType,
    point: CGPoint,
) -> Result<CGEvent, String> {
    let button = match event_type {
        CGEventType::LeftMouseDown | CGEventType::LeftMouseUp => CGMouseButton::Left,
        CGEventType::RightMouseDown | CGEventType::RightMouseUp => CGMouseButton::Right,
        _ => CGMouseButton::Left,
    };

    CGEvent::new_mouse_event(source.clone(), event_type, point, button)
        .map_err(|_| format!("Could not create {:?} event", event_type))
}

fn post_mouse_event(
    source: &CGEventSource,
    event_type: CGEventType,
    point: CGPoint,
    button: CGMouseButton,
) -> Result<(), String> {
    let event = CGEvent::new_mouse_event(source.clone(), event_type, point, button)
        .map_err(|_| format!("Could not create {:?} event", event_type))?;
    event.post(CGEventTapLocation::HID);
    Ok(())
}
