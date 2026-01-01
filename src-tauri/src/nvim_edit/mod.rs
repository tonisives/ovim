//! "Edit with Neovim" feature - open any text field in nvim via a keyboard shortcut

pub mod accessibility;
mod browser_scripting;
mod clipboard;
mod geometry;
mod rpc;
mod session;
pub mod terminals;
mod text_capture;

pub use session::EditSessionManager;

use crate::config::NvimEditSettings;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Trigger the "Edit with Neovim" flow
pub fn trigger_nvim_edit(
    manager: Arc<EditSessionManager>,
    settings: NvimEditSettings,
) -> Result<(), String> {
    // 1. Capture focus context (which app we're in)
    let focus_context = accessibility::capture_focus_context()
        .ok_or("No focused application found")?;
    log::info!("Captured focus context: {:?}", focus_context);

    // 2. Capture geometry info BEFORE any clipboard operations (which may change focus)
    log::info!("popup_mode={}, popup_width={}, popup_height={}", settings.popup_mode, settings.popup_width, settings.popup_height);
    let element_frame = accessibility::get_focused_element_frame();
    let window_frame = accessibility::get_focused_window_frame();
    log::info!("Element frame from accessibility: {:?}", element_frame.as_ref().map(|f| (f.x, f.y, f.width, f.height)));
    log::info!("Window frame: {:?}", window_frame.as_ref().map(|f| (f.x, f.y, f.width, f.height)));

    // 3. Capture text and get element frame (may use browser scripting as fallback)
    let capture_result = text_capture::capture_text_and_frame(
        &focus_context.app_bundle_id,
        element_frame,
    );
    let text = capture_result.text;
    let element_frame = capture_result.element_frame;

    // 4. Calculate window geometry if popup mode is enabled
    let geometry = geometry::calculate_popup_geometry(&settings, element_frame, window_frame);
    log::info!("Final geometry: {:?}", geometry);

    // 5. Start edit session (writes temp file, spawns terminal)
    let session_id = manager.start_session(focus_context, text.clone(), settings.clone(), geometry)?;
    log::info!("Started edit session: {}", session_id);

    // 6. Start RPC connection and live sync in background
    let session = manager.get_session(&session_id)
        .ok_or("Session not found immediately after creation")?;

    let live_sync_worked = Arc::new(AtomicBool::new(false));
    let rpc_handle = spawn_rpc_handler(
        &session,
        &settings,
        Arc::clone(&live_sync_worked),
    );

    // 7. Spawn main thread to wait for nvim to exit and restore text
    spawn_completion_handler(
        manager,
        session_id,
        rpc_handle,
        live_sync_worked,
    );

    Ok(())
}

/// Spawn the RPC handler thread for live sync
fn spawn_rpc_handler(
    session: &session::EditSession,
    settings: &NvimEditSettings,
    live_sync_worked: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    let socket_path = session.socket_path.clone();
    let focus_element = session.focus_context.focused_element.clone();
    let browser_type = browser_scripting::detect_browser_type(&session.focus_context.app_bundle_id);
    let live_sync_enabled = settings.live_sync_enabled;

    thread::spawn(move || {
        if !live_sync_enabled {
            log::info!("Live sync disabled, skipping RPC connection");
            return;
        }

        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                log::error!("Failed to create tokio runtime: {}", e);
                return;
            }
        };

        rt.block_on(async {
            log::info!("Attempting RPC connection to {:?}", socket_path);

            let sync_flag = Arc::clone(&live_sync_worked);
            let element_for_callback = focus_element.clone();

            let on_lines = Arc::new(move |lines: Vec<String>| {
                handle_live_sync_update(
                    &lines,
                    browser_type,
                    element_for_callback.as_ref(),
                    &sync_flag,
                );
            });

            match rpc::connect_to_nvim(&socket_path, on_lines).await {
                Ok(rpc_session) => {
                    log::info!("RPC connected, live sync enabled");
                    wait_for_nvim_exit(&socket_path).await;
                    let _ = rpc_session.detach().await;
                }
                Err(e) => {
                    log::warn!("RPC connection failed, falling back to clipboard-only mode: {}", e);
                }
            }
        });
    })
}

/// Handle a live sync update from nvim
fn handle_live_sync_update(
    lines: &[String],
    browser_type: Option<browser_scripting::BrowserType>,
    focus_element: Option<&accessibility::AXElementHandle>,
    sync_flag: &AtomicBool,
) {
    let text = lines.join("\n");
    let preview: String = text.lines().take(5).collect::<Vec<_>>().join("\\n");
    log::info!("Live sync: {} lines, {} chars, preview: {}", lines.len(), text.len(), preview);

    // For browsers, use browser scripting (JS) which works with code editors
    if let Some(bt) = browser_type {
        match browser_scripting::set_browser_element_text(bt, &text) {
            Ok(()) => {
                sync_flag.store(true, Ordering::SeqCst);
                log::info!("Live sync (browser JS): updated text field ({} chars)", text.len());
                return;
            }
            Err(e) => {
                log::debug!("Browser live sync failed: {}", e);
            }
        }
    }

    // For non-browsers, use accessibility API
    if let Some(element) = focus_element {
        match accessibility::set_element_text(element, &text) {
            Ok(()) => {
                sync_flag.store(true, Ordering::SeqCst);
                log::info!("Live sync (AX): updated text field ({} chars)", text.len());
            }
            Err(e) => {
                log::debug!("Accessibility live sync failed: {}", e);
            }
        }
    }
}

/// Wait for nvim to exit by monitoring the socket
async fn wait_for_nvim_exit(socket_path: &std::path::Path) {
    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if !rpc::socket_exists(socket_path) {
            log::info!("Socket removed, nvim has exited");
            break;
        }
    }
}

/// Spawn the completion handler thread that waits for nvim and restores text
fn spawn_completion_handler(
    manager: Arc<EditSessionManager>,
    session_id: uuid::Uuid,
    rpc_handle: thread::JoinHandle<()>,
    live_sync_worked: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        let Some(session) = manager.get_session(&session_id) else {
            log::error!("Session not found: {}", session_id);
            return;
        };

        log::info!("Waiting for process: {:?} (PID: {:?})", session.terminal_type, session.process_id);

        // For Custom terminal without PID, skip wait_for_process - RPC thread handles waiting
        let is_custom_without_pid = session.terminal_type == terminals::TerminalType::Custom
            && session.process_id.is_none();

        if is_custom_without_pid {
            log::info!("Custom terminal without PID, RPC thread will handle wait");
        } else if let Err(e) = terminals::wait_for_process(&session.terminal_type, session.process_id) {
            log::error!("Error waiting for terminal process: {}", e);
            manager.cancel_session(&session_id);
            return;
        }

        log::info!("Terminal process exited, reading edited file");

        // Wait for RPC thread to finish
        let _ = rpc_handle.join();

        // Restore focus to the original app immediately
        log::info!("Restoring focus immediately");
        if let Err(e) = accessibility::restore_focus(&session.focus_context) {
            log::error!("Error restoring focus: {}", e);
        }

        // Small delay to ensure file is written and focus is settled
        thread::sleep(Duration::from_millis(100));

        // Check if live sync was working
        let did_live_sync = live_sync_worked.load(Ordering::SeqCst);
        log::info!("Live sync status: {}", if did_live_sync { "worked" } else { "not used" });

        // Complete the session - skip clipboard paste if live sync worked
        if let Err(e) = complete_edit_session(&manager, &session_id, did_live_sync) {
            log::error!("Error completing edit session: {}", e);
        }

        // Clean up socket file
        let _ = std::fs::remove_file(&session.socket_path);

        // Clean up session
        manager.remove_session(&session_id);
    });
}

/// Complete the edit session: clean up temp file and optionally restore text via clipboard
fn complete_edit_session(
    manager: &EditSessionManager,
    session_id: &uuid::Uuid,
    live_sync_worked: bool,
) -> Result<(), String> {
    let session = manager.get_session(session_id)
        .ok_or("Session not found")?;

    log::info!("Reading temp file: {:?}", session.temp_file);

    // Check if file was modified by comparing modification times
    let current_mtime = std::fs::metadata(&session.temp_file)
        .and_then(|m| m.modified())
        .map_err(|e| format!("Failed to get current file mtime: {}", e))?;

    if current_mtime == session.file_mtime {
        log::info!("File not modified (nvim quit without saving), skipping restoration");
        let _ = std::fs::remove_file(&session.temp_file);
        return Ok(());
    }

    let edited_text = std::fs::read_to_string(&session.temp_file)
        .map_err(|e| format!("Failed to read temp file: {}", e))?;

    // Strip trailing newline that nvim adds (fixeol option)
    let edited_text = edited_text.strip_suffix('\n').unwrap_or(&edited_text).to_string();

    log::info!("Read {} chars from temp file", edited_text.len());

    // Clean up temp file
    let _ = std::fs::remove_file(&session.temp_file);

    // If live sync worked, text is already in the field - no need for clipboard paste
    if live_sync_worked {
        log::info!("Live sync worked, skipping clipboard paste");
        return Ok(());
    }

    // Small delay for focus to settle
    thread::sleep(Duration::from_millis(100));

    log::info!("Replacing text via clipboard (live sync was not available)");
    clipboard::replace_text_via_clipboard(&edited_text)?;

    log::info!("Successfully restored edited text");
    Ok(())
}
