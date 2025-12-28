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
      <div className="setting-row">
        <label className="setting-label">
          <span>Enable Click Mode</span>
          <span className="setting-description">
            Show hint labels on UI elements for keyboard-driven clicking
          </span>
        </label>
        <label className="toggle">
          <input
            type="checkbox"
            checked={clickMode.enabled}
            onChange={(e) => updateClickMode({ enabled: e.target.checked })}
          />
          <span className="toggle-slider"></span>
        </label>
      </div>

      {clickMode.enabled && (
        <>
          {/* Shortcut */}
          <div className="setting-row">
            <label className="setting-label">
              <span>Activation Shortcut</span>
              <span className="setting-description">
                Press this shortcut to enter click mode
              </span>
            </label>
            <div className="shortcut-input">
              {isRecording ? (
                <div className="recording-indicator">
                  <span>Press any key...</span>
                  <button className="cancel-btn" onClick={handleCancelRecord}>
                    Cancel
                  </button>
                </div>
              ) : (
                <button className="shortcut-btn" onClick={handleRecordKey}>
                  {displayName}
                </button>
              )}
            </div>
          </div>

          {/* Hint Characters */}
          <div className="setting-row">
            <label className="setting-label">
              <span>Hint Characters</span>
              <span className="setting-description">
                Characters used for hint labels (home row first for speed)
              </span>
            </label>
            <input
              type="text"
              className="text-input"
              value={clickMode.hint_chars}
              onChange={(e) => updateClickMode({ hint_chars: e.target.value })}
              placeholder="asdfghjklqwertyuiopzxcvbnm"
            />
          </div>

          {/* Hint Styling Section */}
          <div className="subsection">
            <h3>Hint Appearance</h3>

            {/* Font Size */}
            <div className="setting-row">
              <label className="setting-label">
                <span>Font Size</span>
                <span className="setting-description">
                  Size of hint label text in pixels
                </span>
              </label>
              <div className="slider-with-value">
                <input
                  type="range"
                  min="8"
                  max="24"
                  step="1"
                  value={clickMode.hint_font_size}
                  onChange={(e) =>
                    updateClickMode({ hint_font_size: parseInt(e.target.value) })
                  }
                />
                <span className="slider-value">{clickMode.hint_font_size}px</span>
              </div>
            </div>

            {/* Opacity */}
            <div className="setting-row">
              <label className="setting-label">
                <span>Opacity</span>
                <span className="setting-description">
                  Transparency of hint labels
                </span>
              </label>
              <div className="slider-with-value">
                <input
                  type="range"
                  min="0.5"
                  max="1"
                  step="0.05"
                  value={clickMode.hint_opacity}
                  onChange={(e) =>
                    updateClickMode({ hint_opacity: parseFloat(e.target.value) })
                  }
                />
                <span className="slider-value">
                  {Math.round(clickMode.hint_opacity * 100)}%
                </span>
              </div>
            </div>

            {/* Background Color */}
            <div className="setting-row">
              <label className="setting-label">
                <span>Background Color</span>
                <span className="setting-description">
                  Hint label background color
                </span>
              </label>
              <div className="color-input-wrapper">
                <input
                  type="color"
                  className="color-input"
                  value={clickMode.hint_bg_color}
                  onChange={(e) => updateClickMode({ hint_bg_color: e.target.value })}
                />
                <span className="color-value">{clickMode.hint_bg_color}</span>
              </div>
            </div>

            {/* Text Color */}
            <div className="setting-row">
              <label className="setting-label">
                <span>Text Color</span>
                <span className="setting-description">
                  Hint label text color
                </span>
              </label>
              <div className="color-input-wrapper">
                <input
                  type="color"
                  className="color-input"
                  value={clickMode.hint_text_color}
                  onChange={(e) => updateClickMode({ hint_text_color: e.target.value })}
                />
                <span className="color-value">{clickMode.hint_text_color}</span>
              </div>
            </div>

            {/* Preview */}
            <div className="setting-row">
              <label className="setting-label">
                <span>Preview</span>
              </label>
              <div className="hint-preview">
                <span
                  className="hint-label-preview"
                  style={{
                    backgroundColor: clickMode.hint_bg_color,
                    color: clickMode.hint_text_color,
                    fontSize: `${clickMode.hint_font_size}px`,
                    opacity: clickMode.hint_opacity,
                    padding: "2px 4px",
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
          <div className="setting-row">
            <label className="setting-label">
              <span>Show Search Bar</span>
              <span className="setting-description">
                Display current input at top of screen when active
              </span>
            </label>
            <label className="toggle">
              <input
                type="checkbox"
                checked={clickMode.show_search_bar}
                onChange={(e) => updateClickMode({ show_search_bar: e.target.checked })}
              />
              <span className="toggle-slider"></span>
            </label>
          </div>
        </>
      )}
    </div>
  )
}
