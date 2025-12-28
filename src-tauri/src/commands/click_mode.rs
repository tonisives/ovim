//! Tauri commands for Click Mode

use tauri::{AppHandle, Emitter, State};

use crate::click_mode::{ClickModeState, ClickableElement};
use crate::AppState;

/// Activate click mode and return the list of clickable elements
#[tauri::command]
pub async fn activate_click_mode(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<ClickableElement>, String> {
    let elements = {
        let mut manager = state
            .click_mode_manager
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        manager.activate()?
    };

    // Emit event to show overlay
    let _ = app.emit("click-mode-activated", &elements);

    Ok(elements)
}

/// Deactivate click mode
#[tauri::command]
pub async fn deactivate_click_mode(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    {
        let mut manager = state
            .click_mode_manager
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        manager.deactivate();
    }

    // Emit event to hide overlay
    let _ = app.emit("click-mode-deactivated", ());

    Ok(())
}

/// Get current click mode state
#[tauri::command]
pub async fn get_click_mode_state(state: State<'_, AppState>) -> Result<ClickModeState, String> {
    let manager = state
        .click_mode_manager
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    Ok(manager.state().clone())
}

/// Click an element by its ID
#[tauri::command]
pub async fn click_mode_click_element(
    app: AppHandle,
    state: State<'_, AppState>,
    element_id: usize,
) -> Result<(), String> {
    {
        let manager = state
            .click_mode_manager
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        manager.click_element(element_id)?;
    }

    // Deactivate after click
    deactivate_click_mode(app, state).await
}

/// Right-click an element by its ID
#[tauri::command]
pub async fn click_mode_right_click_element(
    app: AppHandle,
    state: State<'_, AppState>,
    element_id: usize,
) -> Result<(), String> {
    {
        let manager = state
            .click_mode_manager
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        manager.right_click_element(element_id)?;
    }

    // Deactivate after click
    deactivate_click_mode(app, state).await
}

/// Handle hint input from the frontend
#[tauri::command]
pub async fn click_mode_input_hint(
    app: AppHandle,
    state: State<'_, AppState>,
    input: String,
) -> Result<Option<ClickableElement>, String> {
    let result = {
        let mut manager = state
            .click_mode_manager
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        // Process each character
        let mut matched_element = None;
        for c in input.chars() {
            match manager.handle_hint_input(c) {
                Ok(Some(element)) => {
                    matched_element = Some(element);
                    break;
                }
                Ok(None) => {
                    // Partial match, continue
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
        matched_element
    };

    if let Some(ref element) = result {
        // Click the matched element
        {
            let manager = state
                .click_mode_manager
                .lock()
                .map_err(|e| format!("Lock error: {}", e))?;
            manager.click_element(element.id)?;
        }

        // Deactivate after successful click
        deactivate_click_mode(app, state).await?;
    } else {
        // Emit updated filtered elements
        let filtered = {
            let manager = state
                .click_mode_manager
                .lock()
                .map_err(|e| format!("Lock error: {}", e))?;
            manager.get_filtered_elements()
        };
        let _ = app.emit("click-mode-filtered", &filtered);
    }

    Ok(result)
}

/// Get filtered elements based on current input
#[tauri::command]
pub async fn get_click_mode_elements(
    state: State<'_, AppState>,
) -> Result<Vec<ClickableElement>, String> {
    let manager = state
        .click_mode_manager
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    Ok(manager.get_filtered_elements())
}
