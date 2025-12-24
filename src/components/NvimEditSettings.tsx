import { useState, useEffect, useCallback } from "react"
import { open } from "@tauri-apps/plugin-dialog"
import { invoke } from "@tauri-apps/api/core"
import type { Settings, NvimEditSettings as NvimEditSettingsType } from "./SettingsApp"
import {
  formatKeyWithModifiers,
  recordKey,
  cancelRecordKey,
  getKeyDisplayName,
} from "./keyRecording"

interface PathValidation {
  terminal_valid: boolean
  terminal_resolved_path: string
  terminal_error: string | null
  editor_valid: boolean
  editor_resolved_path: string
  editor_error: string | null
}

interface Props {
  settings: Settings
  onUpdate: (updates: Partial<Settings>) => void
}

const TERMINAL_OPTIONS = [
  { value: "alacritty", label: "Alacritty" },
  { value: "kitty", label: "Kitty" },
  { value: "wezterm", label: "WezTerm" },
  { value: "iterm", label: "iTerm2" },
  { value: "default", label: "Terminal.app" },
]

const DEFAULT_TERMINAL_PATHS: Record<string, string> = {
  alacritty: "/Applications/Alacritty.app/Contents/MacOS/alacritty",
  kitty: "/Applications/kitty.app/Contents/MacOS/kitty",
  wezterm: "/Applications/WezTerm.app/Contents/MacOS/wezterm",
  iterm: "",
  default: "",
}

const EDITOR_OPTIONS = [
  { value: "neovim", label: "Neovim" },
  { value: "vim", label: "Vim" },
  { value: "helix", label: "Helix" },
  { value: "custom", label: "Custom" },
]

const DEFAULT_EDITOR_PATHS: Record<string, string> = {
  neovim: "nvim",
  vim: "vim",
  helix: "hx",
  custom: "",
}

export function NvimEditSettings({ settings, onUpdate }: Props) {
  const [isRecording, setIsRecording] = useState(false)
  const [displayName, setDisplayName] = useState<string | null>(null)
  const [validation, setValidation] = useState<PathValidation | null>(null)
  const [isValidating, setIsValidating] = useState(false)
  const [showErrorDialog, setShowErrorDialog] = useState<"terminal" | "editor" | null>(null)

  const nvimEdit = settings.nvim_edit

  // Validate paths when settings change
  const validatePaths = useCallback(async () => {
    if (!nvimEdit.enabled) {
      setValidation(null)
      return
    }

    setIsValidating(true)
    try {
      const result = await invoke<PathValidation>("validate_nvim_edit_paths", {
        terminalType: nvimEdit.terminal,
        terminalPath: nvimEdit.terminal_path,
        editorType: nvimEdit.editor,
        editorPath: nvimEdit.nvim_path,
      })
      setValidation(result)
    } catch (e) {
      console.error("Failed to validate paths:", e)
      setValidation(null)
    } finally {
      setIsValidating(false)
    }
  }, [
    nvimEdit.enabled,
    nvimEdit.terminal,
    nvimEdit.terminal_path,
    nvimEdit.editor,
    nvimEdit.nvim_path,
  ])

  // Run validation on mount and when relevant settings change
  useEffect(() => {
    validatePaths()
  }, [validatePaths])

  useEffect(() => {
    getKeyDisplayName(nvimEdit.shortcut_key)
      .then((name) => {
        if (name) {
          setDisplayName(formatKeyWithModifiers(name, nvimEdit.shortcut_modifiers))
        } else {
          setDisplayName(null)
        }
      })
      .catch(() => setDisplayName(null))
  }, [nvimEdit.shortcut_key, nvimEdit.shortcut_modifiers])

  const updateNvimEdit = (updates: Partial<NvimEditSettingsType>) => {
    onUpdate({
      nvim_edit: { ...nvimEdit, ...updates },
    })
  }

  const handleEditorChange = (newEditor: string) => {
    const currentPath = nvimEdit.nvim_path

    // Check if current path is empty or matches a default editor path
    const isDefaultPath =
      currentPath === "" || Object.values(DEFAULT_EDITOR_PATHS).includes(currentPath)

    if (isDefaultPath) {
      // Update both editor and path to the new editor's default
      updateNvimEdit({
        editor: newEditor,
        nvim_path: "", // Empty means use default
      })
    } else {
      // User has a custom path, keep it
      updateNvimEdit({ editor: newEditor })
    }
  }

  const handleTerminalChange = (newTerminal: string) => {
    // Always clear the path when switching terminals
    // This is simpler and safer - the new terminal will use auto-detection
    updateNvimEdit({
      terminal: newTerminal,
      terminal_path: "",
    })
  }

  const handleRecordKey = async () => {
    setIsRecording(true)
    try {
      const recorded = await recordKey()
      updateNvimEdit({
        shortcut_key: recorded.name,
        shortcut_modifiers: recorded.modifiers,
      })
      const formatted = formatKeyWithModifiers(recorded.display_name, recorded.modifiers)
      setDisplayName(formatted)
    } catch (e) {
      console.error("Failed to record key:", e)
    } finally {
      setIsRecording(false)
    }
  }

  const handleCancelRecord = () => {
    cancelRecordKey().catch(() => {})
    setIsRecording(false)
  }

  // Count validation errors
  const errorCount = validation
    ? (validation.terminal_valid ? 0 : 1) + (validation.editor_valid ? 0 : 1)
    : 0

  return (
    <div className="settings-section">
      <div className="section-header">
        <h2>Edit Popup</h2>
        {nvimEdit.enabled && errorCount > 0 && (
          <span className="error-badge" title="Configuration errors detected">
            {errorCount} {errorCount === 1 ? "error" : "errors"}
          </span>
        )}
        {isValidating && <span className="validating-badge">Checking...</span>}
      </div>
      <p className="section-description">
        Press a shortcut while focused on any text field to edit its contents in your preferred
        terminal editor.
      </p>

      {/* Error dialog */}
      {showErrorDialog && (
        <div className="error-dialog-overlay" onClick={() => setShowErrorDialog(null)}>
          <div className="error-dialog" onClick={(e) => e.stopPropagation()}>
            <h3>{showErrorDialog === "terminal" ? "Terminal Not Found" : "Editor Not Found"}</h3>
            <p>
              {showErrorDialog === "terminal"
                ? validation?.terminal_error
                : validation?.editor_error}
            </p>
            <div className="error-dialog-buttons">
              <button onClick={() => setShowErrorDialog(null)}>Close</button>
            </div>
          </div>
        </div>
      )}

      <div className="form-group">
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={nvimEdit.enabled}
            onChange={(e) => updateNvimEdit({ enabled: e.target.checked })}
          />
          Enable Edit Popup feature
        </label>
      </div>

      <div className="form-group">
        <label>Keyboard shortcut</label>
        <div className="key-display">
          {isRecording ? (
            <button type="button" className="record-key-btn recording" onClick={handleCancelRecord}>
              Press any key...
            </button>
          ) : (
            <>
              <span className="current-key">{displayName || nvimEdit.shortcut_key}</span>
              <button
                type="button"
                className="record-key-btn"
                onClick={handleRecordKey}
                disabled={!nvimEdit.enabled}
              >
                Record Key
              </button>
            </>
          )}
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
                onClick={() => setShowErrorDialog("editor")}
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
              onChange={(e) => updateNvimEdit({ nvim_path: e.target.value })}
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
                  updateNvimEdit({ nvim_path: file })
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
                onClick={() => setShowErrorDialog("terminal")}
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
              onChange={(e) => updateNvimEdit({ terminal_path: e.target.value })}
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
                  // Detect terminal type from path
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
                    updateNvimEdit({ terminal_path: file, terminal: detectedTerminal })
                  } else {
                    // Not a recognized terminal - don't update, show alert
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
            checked={nvimEdit.popup_mode}
            onChange={(e) => updateNvimEdit({ popup_mode: e.target.checked })}
            disabled={!nvimEdit.enabled}
          />
          Open as popup below text field
        </label>
        <span className="hint">
          Position the terminal window directly below the text field instead of opening fullscreen
        </span>
      </div>

      {nvimEdit.popup_mode && (
        <div className="form-row">
          <div className="form-group">
            <label htmlFor="popup-width">Popup width (px)</label>
            <input
              type="number"
              id="popup-width"
              value={nvimEdit.popup_width}
              onChange={(e) => updateNvimEdit({ popup_width: parseInt(e.target.value) || 0 })}
              min={0}
              disabled={!nvimEdit.enabled}
            />
            <span className="hint">0 = match text field width</span>
          </div>
          <div className="form-group">
            <label htmlFor="popup-height">Popup height (px)</label>
            <input
              type="number"
              id="popup-height"
              value={nvimEdit.popup_height}
              onChange={(e) => updateNvimEdit({ popup_height: parseInt(e.target.value) || 300 })}
              min={100}
              disabled={!nvimEdit.enabled}
            />
          </div>
        </div>
      )}

      <div className="form-group">
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={nvimEdit.live_sync_enabled}
            onChange={(e) => updateNvimEdit({ live_sync_enabled: e.target.checked })}
            disabled={!nvimEdit.enabled}
          />
          Live sync text field
          <span className="beta-badge">BETA</span>
        </label>
        <span className="hint">
          Sync changes to the original text field as you type in the editor. Only works with Neovim.
        </span>
      </div>
    </div>
  )
}
