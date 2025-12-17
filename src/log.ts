import { invoke } from "@tauri-apps/api/core";

// Logger that writes to /tmp/ovim-webview.log via Rust backend
export const log = {
  info: (message: string) => {
    invoke("webview_log", { level: "info", message }).catch(console.error);
  },
  warn: (message: string) => {
    invoke("webview_log", { level: "warn", message }).catch(console.error);
  },
  error: (message: string) => {
    invoke("webview_log", { level: "error", message }).catch(console.error);
  },
  debug: (message: string) => {
    invoke("webview_log", { level: "debug", message }).catch(console.error);
  },
};
