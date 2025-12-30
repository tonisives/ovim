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
        <h2>Click Mode</h2>
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
          Timing settings for troubleshooting. Increase delays if some hints are missing on slower systems.
        </p>

        <div className="indicator-controls">
          {/* Stabilization Delay */}
          <div className="slider-group">
            <label>Stabilization Delay</label>
            <input
              type="range"
              min="0"
              max="300"
              step="10"
              value={clickMode.ax_stabilization_delay_ms}
              onChange={(e) => updateClickMode({ ax_stabilization_delay_ms: parseInt(e.target.value) })}
              disabled={!clickMode.enabled}
            />
            <div className="slider-labels">
              <span>0ms</span>
              <span>{clickMode.ax_stabilization_delay_ms}ms</span>
              <span>300ms</span>
            </div>
          </div>

          {/* Cache Duration */}
          <div className="slider-group">
            <label>Cache Duration</label>
            <input
              type="range"
              min="0"
              max="2000"
              step="100"
              value={clickMode.cache_ttl_ms}
              onChange={(e) => updateClickMode({ cache_ttl_ms: parseInt(e.target.value) })}
              disabled={!clickMode.enabled}
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
