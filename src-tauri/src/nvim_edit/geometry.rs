//! Window geometry calculations for popup positioning

use super::accessibility::{self, ElementFrame};
use super::terminals::WindowGeometry;
use crate::config::NvimEditSettings;

/// Calculate window geometry for popup mode based on element and window frames
pub fn calculate_popup_geometry(
    settings: &NvimEditSettings,
    element_frame: Option<ElementFrame>,
    window_frame: Option<ElementFrame>,
) -> Option<WindowGeometry> {
    if !settings.popup_mode {
        log::info!("popup_mode is disabled");
        return None;
    }

    // Try to get element frame from accessibility API
    let frame_geometry = element_frame.map(|frame| {
        calculate_geometry_from_element(&frame, settings)
    });

    // If element frame not available (e.g., web views), center in the focused window
    let result = frame_geometry.or_else(|| {
        window_frame.map(|wf| {
            calculate_geometry_centered_in_window(&wf, settings)
        })
    });

    if result.is_none() {
        log::warn!("No geometry available - window will open at default size/position");
    }

    result
}

/// Calculate geometry positioning the popup relative to an element
fn calculate_geometry_from_element(frame: &ElementFrame, settings: &NvimEditSettings) -> WindowGeometry {
    let gap = 5;
    let x = frame.x as i32;

    // Use configured width, or match text field width (min 400)
    let width = if settings.popup_width > 0 {
        settings.popup_width
    } else {
        (frame.width as u32).max(400)
    };

    let height = settings.popup_height;
    let popup_height = height as i32;

    // Default: position below the text field
    let mut y = (frame.y + frame.height) as i32 + gap;

    // Get screen bounds to check if popup fits
    if let Some(screen) = accessibility::get_screen_bounds_for_point(frame.x, frame.y) {
        let screen_bottom = (screen.y + screen.height) as i32;
        let popup_bottom = y + popup_height;

        // If popup would go off screen bottom, try positioning above
        if popup_bottom > screen_bottom {
            let above_y = frame.y as i32 - popup_height - gap;
            let screen_top = screen.y as i32;

            if above_y >= screen_top {
                // Fits above - use that position
                y = above_y;
                log::info!("Popup doesn't fit below, positioning above at y={}", y);
            } else {
                // Neither above nor below fits - center on screen
                y = (screen.y + (screen.height - popup_height as f64) / 2.0) as i32;
                log::info!("Popup doesn't fit above or below, centering at y={}", y);
            }
        }
    }

    log::info!("Using element frame geometry: x={}, y={}, w={}, h={}", x, y, width, height);
    WindowGeometry { x, y, width, height }
}

/// Calculate geometry centering the popup in a window
fn calculate_geometry_centered_in_window(wf: &ElementFrame, settings: &NvimEditSettings) -> WindowGeometry {
    let width = if settings.popup_width > 0 {
        settings.popup_width
    } else {
        500 // Default width for web views
    };

    let height = settings.popup_height;

    // Center popup in the focused window
    let x = (wf.x + (wf.width - width as f64) / 2.0) as i32;
    let y = (wf.y + (wf.height - height as f64) / 2.0) as i32;

    log::info!("Using window frame geometry (centered): x={}, y={}, w={}, h={}", x, y, width, height);
    WindowGeometry { x, y, width, height }
}
