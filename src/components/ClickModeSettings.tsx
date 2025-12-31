import { useCallback } from "react"
import type { Settings, ClickModeSettings } from "./SettingsApp"
import { useKeyRecording } from "../hooks/useKeyRecording"

interface Props {
  settings: Settings
  onUpdate: (updates: Partial<Settings>) => void
}

export function ClickModeSettingsComponent({ settings, onUpdate }: Props) {
  const clickMode = settings.click_mode

  const updateClickMode = useCallback(
    (updates: Partial<ClickModeSettings>) => {
      onUpdate({
        click_mode: { ...clickMode, ...updates },
      })
    },
    [clickMode, onUpdate],
  )

  const { isRecording, displayName, handleRecordKey, handleCancelRecord } = useKeyRecording({
    key: clickMode.shortcut_key,
    modifiers: clickMode.shortcut_modifiers,
    onKeyRecorded: (key, modifiers) => {
      updateClickMode({
        shortcut_key: key,
        shortcut_modifiers: modifiers,
      })
    },
  })

  return (
    <div className="settings-section">
      <div className="section-header">
        <h2>Click Mode <span className="beta-badge">BETA</span></h2>
      </div>
      <p className="section-description">
        Press a shortcut to show hint labels on clickable elements. Type the hint to click without using the mouse.
      </p>

      {/* Enable/Disable Toggle */}
      <div className="form-group">
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={clickMode.enabled}
            onChange={(e) => updateClickMode({ enabled: e.target.checked })}
          />
          Enable Click Mode feature
        </label>
      </div>

      {/* Keyboard Shortcut */}
      <div className="form-group">
        <label>Keyboard shortcut</label>
        <div className="key-display">
          {isRecording ? (
            <button type="button" className="record-key-btn recording" onClick={handleCancelRecord}>
              Press any key...
            </button>
          ) : (
            <>
              <span className="current-key">{displayName || clickMode.shortcut_key}</span>
              <button
                type="button"
                className="record-key-btn"
                onClick={handleRecordKey}
                disabled={!clickMode.enabled}
              >
                Record Key
              </button>
            </>
          )}
        </div>
      </div>

      {/* Hint Characters */}
      <div className="form-group">
        <label>Hint characters</label>
        <input
          type="text"
          value={clickMode.hint_chars}
          onChange={(e) => updateClickMode({ hint_chars: e.target.value })}
          placeholder="asfghjklqwetyuiopzxvbm"
          disabled={!clickMode.enabled}
        />
        <span className="hint">Characters for hints (r, c, d, n reserved for action switching)</span>
      </div>

      {/* Hint Appearance Section */}
      <div className="color-settings">
        <h3>Hint Appearance</h3>

        <div className="indicator-controls">
          {/* Font Size */}
          <div className="slider-group">
            <label>Font Size</label>
            <input
              type="range"
              min="8"
              max="24"
              step="1"
              value={clickMode.hint_font_size}
              onChange={(e) => updateClickMode({ hint_font_size: parseInt(e.target.value) })}
              disabled={!clickMode.enabled}
            />
            <div className="slider-labels">
              <span>8px</span>
              <span>{clickMode.hint_font_size}px</span>
              <span>24px</span>
            </div>
          </div>

          {/* Opacity */}
          <div className="slider-group">
            <label>Opacity</label>
            <input
              type="range"
              min="0.5"
              max="1"
              step="0.05"
              value={clickMode.hint_opacity}
              onChange={(e) => updateClickMode({ hint_opacity: parseFloat(e.target.value) })}
              disabled={!clickMode.enabled}
            />
            <div className="slider-labels">
              <span>50%</span>
              <span>{Math.round(clickMode.hint_opacity * 100)}%</span>
              <span>100%</span>
            </div>
          </div>
        </div>

        {/* Colors */}
        <div className="color-pickers">
          <div className="color-picker-group">
            <label>Background</label>
            <div className="color-input-wrapper">
              <input
                type="color"
                value={clickMode.hint_bg_color}
                onChange={(e) => updateClickMode({ hint_bg_color: e.target.value })}
                disabled={!clickMode.enabled}
              />
              <span className="color-hex">{clickMode.hint_bg_color}</span>
            </div>
          </div>

          <div className="color-picker-group">
            <label>Text</label>
            <div className="color-input-wrapper">
              <input
                type="color"
                value={clickMode.hint_text_color}
                onChange={(e) => updateClickMode({ hint_text_color: e.target.value })}
                disabled={!clickMode.enabled}
              />
              <span className="color-hex">{clickMode.hint_text_color}</span>
            </div>
          </div>

          {/* Preview */}
          <div className="color-picker-group">
            <label>Preview</label>
            <span
              style={{
                backgroundColor: clickMode.hint_bg_color,
                color: clickMode.hint_text_color,
                fontSize: `${clickMode.hint_font_size}px`,
                opacity: clickMode.hint_opacity,
                padding: "2px 6px",
                borderRadius: "3px",
                fontFamily: "SF Mono, Monaco, Menlo, monospace",
                fontWeight: 700,
                letterSpacing: "0.5px",
                border: "1px solid rgba(0,0,0,0.2)",
              }}
            >
              {clickMode.hint_chars.slice(0, 2).toUpperCase() || "AS"}
            </span>
          </div>
        </div>
      </div>

      {/* Show Search Bar Toggle */}
      <div className="form-group" style={{ marginTop: 16 }}>
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={clickMode.show_search_bar}
            onChange={(e) => updateClickMode({ show_search_bar: e.target.checked })}
            disabled={!clickMode.enabled}
          />
          Show search bar when active
        </label>
        <span className="hint">Display current input at top of screen</span>
      </div>

      {/* Advanced Settings Section */}
      <div className="color-settings">
        <h3>Advanced</h3>
        <p className="help-text">
          Settings for troubleshooting. Increase limits if hints are missing in apps with complex UIs (e.g., Electron apps like Slack).
        </p>

        <div className="indicator-controls">
          {/* Max Depth */}
          <div className="slider-group">
            <label title="How deep to traverse the UI hierarchy. Electron apps (Slack, Discord, VS Code) need higher values (30-50).">
              Max Depth
            </label>
            <input
              type="range"
              min="5"
              max="50"
              step="5"
              value={clickMode.max_depth ?? 10}
              onChange={(e) => updateClickMode({ max_depth: parseInt(e.target.value) })}
              disabled={!clickMode.enabled}
              title="How deep to traverse the UI hierarchy. Electron apps (Slack, Discord, VS Code) need higher values (30-50)."
            />
            <div className="slider-labels">
              <span>5</span>
              <span>{clickMode.max_depth ?? 10}</span>
              <span>50</span>
            </div>
          </div>

          {/* Max Elements */}
          <div className="slider-group">
            <label title="Maximum number of clickable elements to find. Increase if some buttons are missing.">
              Max Elements
            </label>
            <input
              type="range"
              min="100"
              max="1000"
              step="50"
              value={clickMode.max_elements ?? 500}
              onChange={(e) => updateClickMode({ max_elements: parseInt(e.target.value) })}
              disabled={!clickMode.enabled}
              title="Maximum number of clickable elements to find. Increase if some buttons are missing."
            />
            <div className="slider-labels">
              <span>100</span>
              <span>{clickMode.max_elements ?? 500}</span>
              <span>1000</span>
            </div>
          </div>

          {/* Stabilization Delay */}
          <div className="slider-group">
            <label title="Wait time before scanning UI elements. Increase if hints appear before the UI is ready.">
              Stabilization Delay
            </label>
            <input
              type="range"
              min="0"
              max="300"
              step="10"
              value={clickMode.ax_stabilization_delay_ms}
              onChange={(e) => updateClickMode({ ax_stabilization_delay_ms: parseInt(e.target.value) })}
              disabled={!clickMode.enabled}
              title="Wait time before scanning UI elements. Increase if hints appear before the UI is ready."
            />
            <div className="slider-labels">
              <span>0ms</span>
              <span>{clickMode.ax_stabilization_delay_ms}ms</span>
              <span>300ms</span>
            </div>
          </div>

          {/* Cache Duration */}
          <div className="slider-group">
            <label title="How long to reuse scanned elements. Higher values make repeated activations faster.">
              Cache Duration
            </label>
            <input
              type="range"
              min="0"
              max="2000"
              step="100"
              value={clickMode.cache_ttl_ms}
              onChange={(e) => updateClickMode({ cache_ttl_ms: parseInt(e.target.value) })}
              disabled={!clickMode.enabled}
              title="How long to reuse scanned elements. Higher values make repeated activations faster."
            />
            <div className="slider-labels">
              <span>0ms</span>
              <span>{clickMode.cache_ttl_ms}ms</span>
              <span>2000ms</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
