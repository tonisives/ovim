import { useState, useEffect } from "react"
import type {
  Settings,
  NvimEditSettings as NvimEditSettingsType,
} from "./SettingsApp"
import {
  formatKeyWithModifiers,
  recordKey,
  cancelRecordKey,
  getKeyDisplayName,
} from "./keyRecording"

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

  const nvimEdit = settings.nvim_edit

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
    const isDefaultPath = currentPath === "" ||
      Object.values(DEFAULT_EDITOR_PATHS).includes(currentPath)

    if (isDefaultPath) {
      // Update both editor and path to the new editor's default
      updateNvimEdit({
        editor: newEditor,
        nvim_path: "" // Empty means use default
      })
    } else {
      // User has a custom path, keep it
      updateNvimEdit({ editor: newEditor })
    }
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

  return (
    <div className="settings-section">
      <h2>Edit Popup</h2>
      <p className="section-description">
        Press a shortcut while focused on any text field to edit its contents in your preferred terminal editor.
      </p>

      <div className="form-group">
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={nvimEdit.enabled}
            onChange={(e) => updateNvimEdit({ enabled: e.target.checked })}
          />
          Enable external editor feature
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
          <label htmlFor="editor">Editor</label>
          <select
            id="editor"
            value={nvimEdit.editor}
            onChange={(e) => handleEditorChange(e.target.value)}
            disabled={!nvimEdit.enabled}
          >
            {EDITOR_OPTIONS.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>
        </div>

        <div className="form-group editor-path-group">
          <label htmlFor="nvim-path">Path {nvimEdit.editor !== "custom" && "(optional)"}</label>
          <input
            type="text"
            id="nvim-path"
            value={nvimEdit.nvim_path}
            onChange={(e) => updateNvimEdit({ nvim_path: e.target.value })}
            placeholder={DEFAULT_EDITOR_PATHS[nvimEdit.editor] || ""}
            disabled={!nvimEdit.enabled}
          />
        </div>
      </div>

      <div className="form-group">
        <label htmlFor="terminal">Terminal</label>
        <select
          id="terminal"
          value={nvimEdit.terminal}
          onChange={(e) => updateNvimEdit({ terminal: e.target.value })}
          disabled={!nvimEdit.enabled}
        >
          {TERMINAL_OPTIONS.map((opt) => (
            <option key={opt.value} value={opt.value}>
              {opt.label}
            </option>
          ))}
        </select>
        {nvimEdit.terminal !== "alacritty" && (
          <div className="alert alert-warning">
            Limited support. Please use Alacritty for best performance and tested compatibility.
          </div>
        )}
      </div>

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
    </div>
  )
}
