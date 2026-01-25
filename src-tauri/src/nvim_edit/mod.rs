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

use crate::config::{NvimEditSettings, Settings};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Trigger the "Edit with Neovim" flow
/// `shared_settings` is optional - if provided, filetype changes will update the in-memory state
pub fn trigger_nvim_edit(
    manager: Arc<EditSessionManager>,
    settings: NvimEditSettings,
    shared_settings: Option<Arc<Mutex<Settings>>>,
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
        settings.clipboard_mode,
    );
    let text = capture_result.text;
    let element_frame = capture_result.element_frame;
    let initial_cursor = capture_result.cursor_position;
    let browser_type = capture_result.browser_type;

    if let Some(ref cursor) = initial_cursor {
        log::info!("Initial cursor position: line={}, col={}", cursor.line, cursor.column);
    }

    // 4. Determine domain key for filetype persistence
    // For browsers, use the hostname. For native apps, use bundle ID.
    let domain_key = if let Some(bt) = browser_type {
        browser_scripting::get_browser_hostname(bt)
            .unwrap_or_else(|| focus_context.app_bundle_id.clone())
    } else {
        focus_context.app_bundle_id.clone()
    };
    log::info!("Domain key for filetype: {}", domain_key);

    // 5. Look up saved filetype for this domain
    let saved_filetype = settings.get_filetype_for_domain(&domain_key).map(|s| s.to_string());
    if let Some(ref ft) = saved_filetype {
        log::info!("Found saved filetype for domain '{}': {}", domain_key, ft);
    }

    // 6. Calculate window geometry if popup mode is enabled
    let geometry = geometry::calculate_popup_geometry(&settings, element_frame, window_frame);
    log::info!("Final geometry: {:?}", geometry);

    // 7. Start edit session (writes temp file, spawns terminal)
    let session_id = manager.start_session(
        focus_context,
        text.clone(),
        settings.clone(),
        geometry,
        domain_key,
        saved_filetype.as_deref(),
    )?;
    log::info!("Started edit session: {}", session_id);

    // 8. Start RPC connection and live sync in background
    // If clipboard_mode is enabled, skip live sync entirely
    let session = manager.get_session(&session_id)
        .ok_or("Session not found immediately after creation")?;

    let live_sync_worked = Arc::new(AtomicBool::new(false));
    let clipboard_mode = settings.clipboard_mode;

    let rpc_handle = if clipboard_mode {
        // In clipboard mode, don't do live sync - but still wait for editor to exit
        log::info!("Clipboard mode enabled, skipping live sync");
        let process_id = session.process_id;
        thread::spawn(move || {
            wait_for_editor_exit(process_id);
            None
        })
    } else {
        spawn_rpc_handler(
            &session,
            &settings,
            Arc::clone(&live_sync_worked),
            browser_type,
            initial_cursor,
        )
    };

    // 9. Spawn main thread to wait for nvim to exit and restore text
    spawn_completion_handler(
        manager,
        session_id,
        rpc_handle,
        live_sync_worked,
        browser_type,
        clipboard_mode,
        shared_settings,
    );

    Ok(())
}

/// Result from RPC handler including final cursor position and filetype
struct RpcResult {
    final_cursor: Option<browser_scripting::CursorPosition>,
    filetype: Option<String>,
}

/// Check if the editor process is still running
fn editor_process_exists(pid: Option<u32>) -> bool {
    if let Some(pid) = pid {
        // Check if process exists by sending signal 0
        unsafe { libc::kill(pid as i32, 0) == 0 }
    } else {
        true // Can't check without PID, assume exists
    }
}

/// Wait for editor process to exit (used when live sync is disabled)
fn wait_for_editor_exit(process_id: Option<u32>) {
    loop {
        if !editor_process_exists(process_id) {
            log::info!("Editor process exited");
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// Spawn the RPC handler thread for live sync
/// Returns a handle that can be joined to get the final cursor position
fn spawn_rpc_handler(
    session: &session::EditSession,
    settings: &NvimEditSettings,
    live_sync_worked: Arc<AtomicBool>,
    browser_type: Option<browser_scripting::BrowserType>,
    initial_cursor: Option<browser_scripting::CursorPosition>,
) -> thread::JoinHandle<Option<RpcResult>> {
    let socket_path = session.socket_path.clone();
    let focus_element = session.focus_context.focused_element.clone();
    let live_sync_enabled = settings.live_sync_enabled;
    let process_id = session.process_id;

    thread::spawn(move || {
        if !live_sync_enabled {
            log::info!("Live sync disabled, skipping RPC connection");
            // Still need to wait for editor to exit
            wait_for_editor_exit(process_id);
            return None;
        }

        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                log::error!("Failed to create tokio runtime: {}", e);
                return None;
            }
        };

        rt.block_on(async {
            log::info!("Attempting RPC connection to {:?}", socket_path);

            let sync_flag = Arc::clone(&live_sync_worked);
            let element_for_callback = focus_element.clone();
            let cached_element_id = Arc::new(std::sync::Mutex::new(None::<String>));
            let cached_id_for_callback = Arc::clone(&cached_element_id);

            let on_lines = Arc::new(move |lines: Vec<String>| {
                handle_live_sync_update(
                    &lines,
                    browser_type,
                    element_for_callback.as_ref(),
                    &sync_flag,
                    &cached_id_for_callback,
                );
            });

            match rpc::connect_to_nvim(&socket_path, on_lines).await {
                Ok(rpc_session) => {
                    log::info!("RPC connected, live sync enabled");

                    // Set nvim cursor to match browser's initial cursor position
                    if let Some(cursor) = initial_cursor {
                        if let Err(e) = rpc_session.set_cursor(cursor.line, cursor.column).await {
                            log::debug!("Failed to set initial nvim cursor: {}", e);
                        } else {
                            log::info!("Set nvim cursor to line={}, col={}", cursor.line, cursor.column);
                        }
                    }

                    // Poll cursor position and filetype periodically until window closes or socket removed
                    // Check window first (faster for Cmd+W), then socket (faster for :q)
                    let mut last_cursor: Option<browser_scripting::CursorPosition> = None;
                    let mut last_filetype: Option<String> = None;
                    loop {
                        // Try to get current cursor position first
                        match rpc_session.get_cursor().await {
                            Ok((line, col)) => {
                                last_cursor = Some(browser_scripting::CursorPosition { line, column: col });
                            }
                            Err(_) => {
                                // RPC failed, nvim is probably closing
                                log::info!("RPC get_cursor failed, nvim closing");
                                break;
                            }
                        }

                        // Also get filetype during polling (so we capture it before nvim closes)
                        if let Ok(ft) = rpc_session.get_filetype().await {
                            if !ft.is_empty() && ft != "text" {
                                last_filetype = Some(ft);
                            }
                        }

                        // Check if editor process is gone (fast for Cmd+W close)
                        if !editor_process_exists(process_id) {
                            log::info!("Editor process exited");
                            break;
                        }

                        // Check socket (fast for :q/:wq)
                        if !rpc::socket_exists(&socket_path) {
                            log::info!("Socket removed, nvim has exited");
                            break;
                        }

                        // Poll every 100ms (window check is ~50ms via AppleScript)
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }

                    if let Some(ref cursor) = last_cursor {
                        log::info!("Final nvim cursor: line={}, col={}", cursor.line, cursor.column);
                    }

                    // Use the filetype we captured during polling
                    let filetype = last_filetype;
                    if let Some(ref ft) = filetype {
                        log::info!("Final filetype: {}", ft);
                    }

                    let _ = rpc_session.detach().await;

                    Some(RpcResult { final_cursor: last_cursor, filetype })
                }
                Err(e) => {
                    log::warn!("RPC connection failed, falling back to clipboard-only mode: {}", e);
                    None
                }
            }
        })
    })
}

/// Handle a live sync update from nvim
fn handle_live_sync_update(
    lines: &[String],
    browser_type: Option<browser_scripting::BrowserType>,
    focus_element: Option<&accessibility::AXElementHandle>,
    sync_flag: &AtomicBool,
    cached_element_id: &std::sync::Mutex<Option<String>>,
) {
    let text = lines.join("\n");
    let preview: String = text.lines().take(5).collect::<Vec<_>>().join("\\n");
    log::info!("Live sync: {} lines, {} chars, preview: {}", lines.len(), text.len(), preview);

    // For browsers, use browser scripting (JS) which works with code editors
    let mut skip_ax_fallback = false;
    if let Some(bt) = browser_type {
        // Get cached element ID if any
        let target_id = cached_element_id.lock().ok().and_then(|g| g.clone());
        match browser_scripting::set_browser_element_text(bt, &text, target_id.as_deref()) {
            Ok(new_element_id) => {
                sync_flag.store(true, Ordering::SeqCst);
                log::info!("Live sync (browser JS): updated text field ({} chars)", text.len());
                // Cache the element ID for subsequent calls
                if let Some(id) = new_element_id {
                    if let Ok(mut guard) = cached_element_id.lock() {
                        *guard = Some(id);
                    }
                }
                return;
            }
            Err(e) => {
                log::info!("Browser live sync failed: {}", e);
                // Lexical editors don't respond to AX value changes either,
                // so skip the AX fallback and rely on clipboard mode
                if e.contains("unsupported_lexical") {
                    log::info!("Lexical editor detected - will use clipboard mode on exit");
                    skip_ax_fallback = true;
                }
            }
        }
    }

    // For non-browsers (or browsers where JS didn't work), use accessibility API
    // Skip for Lexical editors since they ignore AX value changes
    if !skip_ax_fallback {
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
}

/// Spawn the completion handler thread that waits for nvim and restores text
fn spawn_completion_handler(
    manager: Arc<EditSessionManager>,
    session_id: uuid::Uuid,
    rpc_handle: thread::JoinHandle<Option<RpcResult>>,
    live_sync_worked: Arc<AtomicBool>,
    browser_type: Option<browser_scripting::BrowserType>,
    clipboard_mode: bool,
    shared_settings: Option<Arc<Mutex<Settings>>>,
) {
    thread::spawn(move || {
        let Some(session) = manager.get_session(&session_id) else {
            log::error!("Session not found: {}", session_id);
            return;
        };

        // Wait for RPC thread to finish - this detects nvim exit via socket removal
        // This is faster than waiting for process exit on Cmd+W window close
        log::info!("Waiting for nvim to exit (via RPC thread)");
        let rpc_result = rpc_handle.join().ok().flatten();
        let final_cursor = rpc_result.as_ref().and_then(|r| r.final_cursor);
        let final_filetype = rpc_result.and_then(|r| r.filetype);

        // Save the filetype for this domain if we got one
        if let Some(ref ft) = final_filetype {
            log::info!("Saving filetype '{}' for domain '{}'", ft, session.domain_key);

            // Update in-memory settings if we have shared state
            if let Some(ref shared) = shared_settings {
                let mut settings = shared.lock().unwrap();
                settings.nvim_edit.set_filetype_for_domain(session.domain_key.clone(), ft.clone());
                log::info!("Updated in-memory settings with filetype");
            } else {
                // Fallback: just save to file (will be loaded on next restart)
                let mut settings = Settings::load();
                settings.nvim_edit.set_filetype_for_domain(session.domain_key.clone(), ft.clone());
            }
        }

        log::info!("Nvim exited, restoring focus");

        // Small delay to let the system settle after window close
        thread::sleep(Duration::from_millis(50));

        // Restore focus to the original app, with retry
        for attempt in 0..3 {
            match accessibility::restore_focus(&session.focus_context) {
                Ok(()) => break,
                Err(e) => {
                    if attempt < 2 {
                        log::info!("Retry {} restoring focus: {}", attempt + 1, e);
                        thread::sleep(Duration::from_millis(50));
                    } else {
                        log::error!("Error restoring focus after retries: {}", e);
                    }
                }
            }
        }

        // Check if live sync was working (but ignore if clipboard_mode is enabled)
        let did_live_sync = if clipboard_mode {
            false // Force clipboard paste in clipboard mode
        } else {
            live_sync_worked.load(Ordering::SeqCst)
        };
        log::info!("Live sync status: {}, clipboard_mode: {}", if did_live_sync { "worked" } else { "not used" }, clipboard_mode);

        // Complete the session - skip clipboard paste if live sync worked
        if let Err(e) = complete_edit_session(&manager, &session_id, did_live_sync) {
            log::error!("Error completing edit session: {}", e);
        }

        // Restore cursor position in browser if we have it
        if let (Some(bt), Some(cursor)) = (browser_type, final_cursor) {
            log::info!("Restoring browser cursor to line={}, col={}", cursor.line, cursor.column);
            match browser_scripting::set_browser_cursor_position(bt, cursor.line, cursor.column) {
                Ok(()) => log::info!("Browser cursor restored successfully"),
                Err(e) => log::info!("Failed to restore browser cursor: {}", e),
            }
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
