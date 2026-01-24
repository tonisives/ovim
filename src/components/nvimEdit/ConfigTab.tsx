import { open } from "@tauri-apps/plugin-dialog"
import { invoke } from "@tauri-apps/api/core"
import type { NvimEditSettings } from "../SettingsApp"
import {
  type PathValidation,
  TERMINAL_OPTIONS,
  DEFAULT_TERMINAL_PATHS,
  EDITOR_OPTIONS,
  DEFAULT_EDITOR_PATHS,
} from "./constants"

interface Props {
  nvimEdit: NvimEditSettings
  validation: PathValidation | null
  isRecording: boolean
  displayName: string | null
  onUpdate: (updates: Partial<NvimEditSettings>) => void
  onRecordKey: () => void
  onCancelRecord: () => void
  onShowErrorDialog: (type: "terminal" | "editor") => void
}

export function ConfigTab({
  nvimEdit,
  validation,
  isRecording,
  displayName,
  onUpdate,
  onRecordKey,
  onCancelRecord,
  onShowErrorDialog,
}: Props) {
  const handleEditorChange = (newEditor: string) => {
    const currentPath = nvimEdit.nvim_path
    const isDefaultPath =
      currentPath === "" || Object.values(DEFAULT_EDITOR_PATHS).includes(currentPath)

    if (isDefaultPath) {
      onUpdate({
        editor: newEditor,
        nvim_path: "",
      })
    } else {
      onUpdate({ editor: newEditor })
    }
  }

  const handleTerminalChange = (newTerminal: string) => {
    onUpdate({
      terminal: newTerminal,
      terminal_path: "",
    })
  }

  return (
    <>
      <div className="form-group">
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={nvimEdit.enabled}
            onChange={(e) => onUpdate({ enabled: e.target.checked })}
          />
          Enable Edit Popup feature
        </label>
      </div>

      <div className="form-group">
        <label>Keyboard shortcut</label>
        <div className="key-display">
          <button
            type="button"
            className={`current-key clickable${isRecording ? " recording" : ""}`}
            onClick={isRecording ? onCancelRecord : onRecordKey}
            disabled={!nvimEdit.enabled && !isRecording}
          >
            {isRecording ? "Press any key..." : displayName || nvimEdit.shortcut_key}
          </button>
        </div>
      </div>

      <div className="form-row editor-row">
        <div className="form-group">
          <label htmlFor="editor">
            Editor
            {validation && !validation.editor_valid && nvimEdit.enabled && (
              <button
                type="button"
                className="inline-error-badge"
                onClick={() => onShowErrorDialog("editor")}
                title={validation.editor_error || "Editor not found"}
              >
                !
              </button>
            )}
          </label>
          <select
            id="editor"
            value={nvimEdit.editor}
            onChange={(e) => handleEditorChange(e.target.value)}
            disabled={!nvimEdit.enabled}
            className={
              validation && !validation.editor_valid && nvimEdit.enabled ? "input-error" : ""
            }
          >
            {EDITOR_OPTIONS.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>
        </div>

        <div className="form-group editor-path-group">
          <label htmlFor="nvim-path">Path {nvimEdit.editor !== "custom" && ""}</label>
          <div className="path-input-row">
            <input
              type="text"
              id="nvim-path"
              value={nvimEdit.nvim_path}
              onChange={(e) => onUpdate({ nvim_path: e.target.value })}
              placeholder={
                validation?.editor_resolved_path || DEFAULT_EDITOR_PATHS[nvimEdit.editor] || ""
              }
              disabled={!nvimEdit.enabled}
              className={
                validation && !validation.editor_valid && nvimEdit.enabled ? "input-error" : ""
              }
            />
            <button
              type="button"
              className="browse-btn"
              onClick={async () => {
                const file = await open({
                  multiple: false,
                  directory: false,
                  defaultPath: "/opt/homebrew/bin",
                })
                if (file) {
                  onUpdate({ nvim_path: file })
                }
              }}
              disabled={!nvimEdit.enabled}
              title="Browse for editor"
            >
              ...
            </button>
          </div>
          {validation &&
            validation.editor_valid &&
            nvimEdit.enabled &&
            validation.editor_resolved_path && (
              <span className="resolved-path" title={validation.editor_resolved_path}>
                Found: {validation.editor_resolved_path.split("/").pop()}
              </span>
            )}
        </div>
      </div>

      <div className="form-row terminal-row">
        <div className="form-group">
          <label htmlFor="terminal">
            Terminal
            {validation && !validation.terminal_valid && nvimEdit.enabled && (
              <button
                type="button"
                className="inline-error-badge"
                onClick={() => onShowErrorDialog("terminal")}
                title={validation.terminal_error || "Terminal not found"}
              >
                !
              </button>
            )}
          </label>
          <select
            id="terminal"
            value={nvimEdit.terminal}
            onChange={(e) => handleTerminalChange(e.target.value)}
            disabled={!nvimEdit.enabled}
            className={
              validation && !validation.terminal_valid && nvimEdit.enabled ? "input-error" : ""
            }
          >
            {TERMINAL_OPTIONS.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>
        </div>

        <div className="form-group terminal-path-group">
          <label htmlFor="terminal-path">Path</label>
          <div className="path-input-row">
            <input
              type="text"
              id="terminal-path"
              value={nvimEdit.terminal_path}
              onChange={(e) => onUpdate({ terminal_path: e.target.value })}
              placeholder={DEFAULT_TERMINAL_PATHS[nvimEdit.terminal] || "auto-detect"}
              disabled={!nvimEdit.enabled}
              className={
                validation && !validation.terminal_valid && nvimEdit.enabled ? "input-error" : ""
              }
            />
            <button
              type="button"
              className="browse-btn"
              onClick={async () => {
                const file = await open({
                  multiple: false,
                  directory: false,
                  defaultPath: "/Applications",
                })
                if (file) {
                  const lowerPath = file.toLowerCase()
                  let detectedTerminal: string | null = null
                  if (lowerPath.includes("alacritty")) {
                    detectedTerminal = "alacritty"
                  } else if (lowerPath.includes("kitty")) {
                    detectedTerminal = "kitty"
                  } else if (lowerPath.includes("wezterm")) {
                    detectedTerminal = "wezterm"
                  } else if (lowerPath.includes("ghostty")) {
                    detectedTerminal = "ghostty"
                  } else if (lowerPath.includes("iterm")) {
                    detectedTerminal = "iterm"
                  } else if (lowerPath.includes("terminal.app")) {
                    detectedTerminal = "default"
                  }

                  if (detectedTerminal) {
                    onUpdate({ terminal_path: file, terminal: detectedTerminal })
                  } else {
                    const appName = file.split("/").pop()?.replace(".app", "") || file
                    const supportedList = TERMINAL_OPTIONS.map((t) => t.label).join(", ")
                    alert(
                      `"${appName}" is not a supported terminal.\n\nSupported terminals: ${supportedList}`,
                    )
                  }
                }
              }}
              disabled={!nvimEdit.enabled}
              title="Browse for terminal"
            >
              ...
            </button>
          </div>
          {validation &&
            validation.terminal_valid &&
            nvimEdit.enabled &&
            validation.terminal_resolved_path && (
              <span className="resolved-path" title={validation.terminal_resolved_path}>
                Found: {validation.terminal_resolved_path.split("/").pop()}
              </span>
            )}
        </div>
      </div>

      {nvimEdit.terminal !== "alacritty" && (
        <div className="alert alert-warning">
          Limited support. Please use Alacritty for best performance and tested compatibility.
        </div>
      )}

      <div className="form-group">
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={nvimEdit.use_custom_script ?? false}
            onChange={(e) => onUpdate({ use_custom_script: e.target.checked })}
            disabled={!nvimEdit.enabled}
          />
          Use custom launcher script
        </label>
        <span className="hint">
          Use a script to spawn the editor. Customize PATH, use tmux popups, etc.
        </span>
      </div>

      {nvimEdit.use_custom_script && (
        <div className="form-group">
          <div className="path-input-row">
            <button
              type="button"
              className="edit-script-btn"
              onClick={async () => {
                try {
                  await invoke("open_launcher_script")
                } catch (e) {
                  console.error("Failed to open launcher script:", e)
                  alert(`Failed to open launcher script: ${e}`)
                }
              }}
              disabled={!nvimEdit.enabled}
            >
              Edit Launcher Script
            </button>
          </div>
          <span className="hint">~/Library/Application Support/ovim/terminal-launcher.sh</span>
        </div>
      )}

      <div className="form-group">
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={nvimEdit.live_sync_enabled}
            onChange={(e) => onUpdate({ live_sync_enabled: e.target.checked })}
            disabled={!nvimEdit.enabled}
          />
          Live sync text field
          <span className="beta-badge">BETA</span>
        </label>
        <span className="hint">
          Sync changes to the original text field as you type in the editor. Only works with Neovim.
        </span>
      </div>

      <div className="form-group">
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={nvimEdit.clipboard_mode ?? false}
            onChange={(e) => onUpdate({ clipboard_mode: e.target.checked })}
            disabled={!nvimEdit.enabled}
          />
          Clipboard mode
        </label>
        <span className="hint">
          Use Cmd+A/Cmd+C/Cmd+V for text capture and restore. Disables smart detection and cursor
          tracking. Use this if you experience issues with specific apps.
        </span>
      </div>
    </>
  )
}
