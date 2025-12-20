import { useState, useEffect } from "react"
import { invoke } from "@tauri-apps/api/core"
import type {
  Settings,
  VimKeyModifiers,
  NvimEditSettings as NvimEditSettingsType,
} from "./SettingsApp"

interface Props {
  settings: Settings
  onUpdate: (updates: Partial<Settings>) => void
}

interface RecordedKey {
  name: string
  display_name: string
  modifiers: VimKeyModifiers
}

const TERMINAL_OPTIONS = [
  { value: "alacritty", label: "Alacritty" },
  { value: "ghostty", label: "Ghostty" },
  { value: "kitty", label: "Kitty" },
  { value: "wezterm", label: "WezTerm" },
  { value: "iterm", label: "iTerm2" },
  { value: "default", label: "Terminal.app" },
]

function formatKeyWithModifiers(displayName: string, modifiers: VimKeyModifiers): string {
  const parts: string[] = []
  if (modifiers.control) parts.push("Ctrl")
  if (modifiers.option) parts.push("Opt")
  if (modifiers.shift) parts.push("Shift")
  if (modifiers.command) parts.push("Cmd")
  parts.push(displayName)
  return parts.join(" + ")
}

export function NvimEditSettings({ settings, onUpdate }: Props) {
  const [isRecording, setIsRecording] = useState(false)
  const [displayName, setDisplayName] = useState<string | null>(null)

  const nvimEdit = settings.nvim_edit

  useEffect(() => {
    invoke<string | null>("get_key_display_name", { keyName: nvimEdit.shortcut_key })
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

  const handleRecordKey = async () => {
    setIsRecording(true)
    try {
      const recorded = await invoke<RecordedKey>("record_key")
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
    invoke("cancel_record_key").catch(() => {})
    setIsRecording(false)
  }

  return (
    <div className="settings-section">
      <h2>Edit with Neovim</h2>
      <p className="section-description">
        Press a shortcut while focused on any text field to edit its contents in Neovim.
      </p>

      <div className="form-group">
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={nvimEdit.enabled}
            onChange={(e) => updateNvimEdit({ enabled: e.target.checked })}
          />
          Enable "Edit with Neovim" feature
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
            Terminal programs with tabs have limited support. Use Alacritty for best performance and
            tested compatibility.
          </div>
        )}
      </div>

      <div className="form-group">
        <label htmlFor="nvim-path">Neovim path</label>
        <input
          type="text"
          id="nvim-path"
          value={nvimEdit.nvim_path}
          onChange={(e) => updateNvimEdit({ nvim_path: e.target.value })}
          placeholder="nvim"
          disabled={!nvimEdit.enabled}
        />
        <span className="hint">Path to nvim binary (use "nvim" if it's in your PATH)</span>
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
