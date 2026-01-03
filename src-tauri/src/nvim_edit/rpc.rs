//! Neovim RPC communication for live buffer sync
//!
//! This module connects to a running neovim instance via Unix socket
//! and subscribes to buffer change notifications to enable live text sync.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use nvim_rs::compat::tokio::Compat;
use nvim_rs::create::tokio::new_path;
use nvim_rs::rpc::handler::Handler;
use nvim_rs::{Buffer, Neovim, Value};
use tokio::io::WriteHalf;
use tokio::net::UnixStream;
use tokio::sync::Mutex;

/// Type alias for the neovim connection writer
type NvimWriter = Compat<WriteHalf<UnixStream>>;

/// Callback type for buffer line changes
/// Receives the full buffer content as a vector of lines
pub type OnLinesCallback = Arc<dyn Fn(Vec<String>) + Send + Sync>;

/// Handler for neovim RPC notifications
#[derive(Clone)]
pub struct BufferHandler {
    /// Callback invoked when buffer lines change
    on_lines: OnLinesCallback,
    /// Current buffer content (reconstructed from events)
    buffer_lines: Arc<Mutex<Vec<String>>>,
    /// Flag to track if live sync is working
    live_sync_active: Arc<Mutex<bool>>,
}

impl BufferHandler {
    /// Create a new buffer handler with the given callback
    pub fn new(on_lines: OnLinesCallback) -> Self {
        Self {
            on_lines,
            buffer_lines: Arc::new(Mutex::new(Vec::new())),
            live_sync_active: Arc::new(Mutex::new(false)),
        }
    }

    /// Set the initial buffer content
    pub async fn set_initial_content(&self, lines: Vec<String>) {
        let mut buffer = self.buffer_lines.lock().await;
        *buffer = lines;
        *self.live_sync_active.lock().await = true;
    }

    /// Check if live sync is active
    #[allow(dead_code)]
    pub async fn is_live_sync_active(&self) -> bool {
        *self.live_sync_active.lock().await
    }
}

#[async_trait]
impl Handler for BufferHandler {
    type Writer = NvimWriter;

    async fn handle_notify(
        &self,
        name: String,
        args: Vec<Value>,
        _neovim: Neovim<Self::Writer>,
    ) {
        match name.as_str() {
            "nvim_buf_lines_event" => {
                // Event args: [buf, changedtick, firstline, lastline, linedata, more]
                if args.len() >= 5 {
                    let firstline = args[2].as_i64().unwrap_or(0) as usize;
                    let lastline = args[3].as_i64().unwrap_or(0) as usize;
                    let linedata = &args[4];

                    // Extract new lines from the event
                    // Note: Empty lines come as empty strings "", not null/nil
                    // We use map instead of filter_map to preserve all lines including empty ones
                    let new_lines: Vec<String> = if let Value::Array(arr) = linedata {
                        arr.iter()
                            .map(|v| v.as_str().unwrap_or("").to_string())
                            .collect()
                    } else {
                        Vec::new()
                    };

                    // Update our buffer state
                    let mut buffer = self.buffer_lines.lock().await;

                    // Handle the line replacement
                    if firstline <= buffer.len() {
                        // Remove old lines in the range
                        let end = lastline.min(buffer.len());
                        if firstline < end {
                            buffer.drain(firstline..end);
                        }
                        // Insert new lines
                        for (i, line) in new_lines.into_iter().enumerate() {
                            if firstline + i <= buffer.len() {
                                buffer.insert(firstline + i, line);
                            } else {
                                buffer.push(line);
                            }
                        }
                    } else {
                        // Append to buffer
                        buffer.extend(new_lines);
                    }

                    // Mark live sync as active
                    *self.live_sync_active.lock().await = true;

                    // Call the callback with the full buffer
                    let full_content = buffer.clone();
                    drop(buffer);
                    (self.on_lines)(full_content);
                }
            }
            "nvim_buf_changedtick_event" => {
                // Buffer changed but no line data - ignore
                log::debug!("Buffer changedtick event received");
            }
            "nvim_buf_detach_event" => {
                log::info!("Buffer detach event received");
                *self.live_sync_active.lock().await = false;
            }
            _ => {
                log::debug!("Unhandled notification: {}", name);
            }
        }
    }

    async fn handle_request(
        &self,
        name: String,
        _args: Vec<Value>,
        _neovim: Neovim<Self::Writer>,
    ) -> Result<Value, Value> {
        log::debug!("Unhandled request: {}", name);
        Err(Value::from(format!("Unknown request: {}", name)))
    }
}

/// Active RPC session with neovim
pub struct NvimRpcSession {
    /// The neovim client
    neovim: Neovim<NvimWriter>,
    /// The buffer we're attached to
    buffer: Buffer<NvimWriter>,
    /// The handler (for checking state)
    #[allow(dead_code)]
    handler: BufferHandler,
}

impl NvimRpcSession {
    /// Check if live sync is currently working
    #[allow(dead_code)]
    pub async fn is_live_sync_active(&self) -> bool {
        self.handler.is_live_sync_active().await
    }

    /// Get the full buffer content from neovim
    #[allow(dead_code)]
    pub async fn get_buffer_content(&self) -> Result<String, String> {
        let lines = self
            .buffer
            .get_lines(0, -1, false)
            .await
            .map_err(|e| format!("Failed to get buffer lines: {}", e))?;
        Ok(lines.join("\n"))
    }

    /// Detach from the buffer
    pub async fn detach(&self) -> Result<(), String> {
        self.buffer
            .detach()
            .await
            .map_err(|e| format!("Failed to detach: {}", e))?;
        Ok(())
    }

    /// Set cursor position in nvim (1-based line and column)
    pub async fn set_cursor(&self, line: usize, column: usize) -> Result<(), String> {
        // nvim uses 1-based line numbers and 0-based column
        let nvim_line = (line + 1) as i64;
        let nvim_col = column as i64;

        self.neovim
            .get_current_win()
            .await
            .map_err(|e| format!("Failed to get current window: {}", e))?
            .set_cursor((nvim_line, nvim_col))
            .await
            .map_err(|e| format!("Failed to set cursor: {}", e))?;

        Ok(())
    }

    /// Get cursor position from nvim (returns 0-based line and column)
    pub async fn get_cursor(&self) -> Result<(usize, usize), String> {
        let (line, col) = self.neovim
            .get_current_win()
            .await
            .map_err(|e| format!("Failed to get current window: {}", e))?
            .get_cursor()
            .await
            .map_err(|e| format!("Failed to get cursor: {}", e))?;

        // Convert from nvim's 1-based line to 0-based
        Ok(((line - 1) as usize, col as usize))
    }
}

/// Connect to a running neovim instance via Unix socket
///
/// Retries connection with exponential backoff since nvim takes time to start.
/// Returns None if connection fails after all retries.
pub async fn connect_to_nvim(
    socket_path: &Path,
    on_lines: OnLinesCallback,
) -> Result<NvimRpcSession, String> {
    let handler = BufferHandler::new(on_lines);

    // Retry with exponential backoff
    let mut delay = Duration::from_millis(100);
    let max_delay = Duration::from_secs(5);
    let mut total_waited = Duration::ZERO;

    let (neovim, io_handler) = loop {
        match new_path(socket_path, handler.clone()).await {
            Ok(result) => break result,
            Err(e) => {
                if total_waited >= max_delay {
                    return Err(format!(
                        "Failed to connect to nvim socket after {:?}: {}",
                        total_waited, e
                    ));
                }
                log::debug!(
                    "Waiting for nvim socket ({}ms elapsed): {}",
                    total_waited.as_millis(),
                    e
                );
                tokio::time::sleep(delay).await;
                total_waited += delay;
                delay = (delay * 2).min(Duration::from_secs(1));
            }
        }
    };

    // Spawn the IO handler in background
    tokio::spawn(async move {
        match io_handler.await {
            Ok(Ok(())) => log::debug!("Neovim IO handler finished normally"),
            Ok(Err(e)) => {
                // Log at debug level since disconnect is expected when nvim exits
                log::debug!("Neovim IO handler finished with error: {:?}", e);
            }
            Err(e) => {
                log::debug!("Neovim IO handler task failed: {:?}", e);
            }
        }
    });

    log::info!("Connected to nvim at {:?}", socket_path);

    // Get the current buffer
    let buffer = neovim
        .get_current_buf()
        .await
        .map_err(|e| format!("Failed to get current buffer: {}", e))?;

    // Get initial buffer content
    let initial_lines = buffer
        .get_lines(0, -1, false)
        .await
        .map_err(|e| format!("Failed to get initial lines: {}", e))?;

    handler.set_initial_content(initial_lines).await;

    // Attach to buffer for change notifications
    // send_buffer=true to get initial content, opts empty
    let attached = buffer
        .attach(true, vec![])
        .await
        .map_err(|e| format!("Failed to attach to buffer: {}", e))?;

    if !attached {
        return Err("Buffer attach returned false".to_string());
    }

    log::info!("Attached to buffer for live sync");

    Ok(NvimRpcSession {
        neovim,
        buffer,
        handler,
    })
}

/// Check if the socket file exists
pub fn socket_exists(socket_path: &Path) -> bool {
    socket_path.exists()
}
