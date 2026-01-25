import { useCallback } from "react"
import type { Settings, ClickModeSettings } from "./SettingsApp"
import { useKeyRecording } from "../hooks/useKeyRecording"
import { Slider, ColorPicker } from "./common"

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
          <button
            type="button"
            className={`current-key clickable${isRecording ? " recording" : ""}`}
            onClick={isRecording ? handleCancelRecord : handleRecordKey}
            disabled={!clickMode.enabled && !isRecording}
          >
            {isRecording ? "Press any key..." : displayName || clickMode.shortcut_key}
          </button>
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
      <HintAppearanceSection
        clickMode={clickMode}
        updateClickMode={updateClickMode}
      />

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
      <AdvancedSettingsSection
        clickMode={clickMode}
        updateClickMode={updateClickMode}
      />
    </div>
  )
}

interface SectionProps {
  clickMode: ClickModeSettings
  updateClickMode: (updates: Partial<ClickModeSettings>) => void
}

function HintAppearanceSection({ clickMode, updateClickMode }: SectionProps) {
  return (
    <div className="color-settings">
      <h3>Hint Appearance</h3>

      <div className="indicator-controls">
        <Slider
          label="Font Size"
          value={clickMode.hint_font_size}
          min={8}
          max={24}
          step={1}
          disabled={!clickMode.enabled}
          formatValue={(v) => `${v}px`}
          formatMin="8px"
          formatMax="24px"
          onChange={(v) => updateClickMode({ hint_font_size: v })}
        />

        <Slider
          label="Opacity"
          value={clickMode.hint_opacity}
          min={0.5}
          max={1}
          step={0.05}
          disabled={!clickMode.enabled}
          formatValue={(v) => `${Math.round(v * 100)}%`}
          formatMin="50%"
          formatMax="100%"
          onChange={(v) => updateClickMode({ hint_opacity: v })}
        />
      </div>

      <div className="color-pickers">
        <ColorPicker
          label="Background"
          value={clickMode.hint_bg_color}
          disabled={!clickMode.enabled}
          onChange={(v) => updateClickMode({ hint_bg_color: v })}
        />

        <ColorPicker
          label="Text"
          value={clickMode.hint_text_color}
          disabled={!clickMode.enabled}
          onChange={(v) => updateClickMode({ hint_text_color: v })}
        />

        <HintPreview clickMode={clickMode} />
      </div>
    </div>
  )
}

function HintPreview({ clickMode }: { clickMode: ClickModeSettings }) {
  return (
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
  )
}

function AdvancedSettingsSection({ clickMode, updateClickMode }: SectionProps) {
  return (
    <div className="color-settings">
      <h3>Advanced</h3>
      <p className="help-text">
        Settings for troubleshooting. Increase limits if hints are missing in apps with complex UIs (e.g., Electron apps like Slack).
      </p>

      <div className="indicator-controls">
        <Slider
          label="Max Depth"
          title="How deep to traverse the UI hierarchy. Electron apps (Slack, Discord, VS Code) need higher values (30-50)."
          value={clickMode.max_depth ?? 10}
          min={5}
          max={50}
          step={5}
          disabled={!clickMode.enabled}
          onChange={(v) => updateClickMode({ max_depth: v })}
        />

        <Slider
          label="Max Elements"
          title="Maximum number of clickable elements to find. Increase if some buttons are missing."
          value={clickMode.max_elements ?? 500}
          min={100}
          max={1000}
          step={50}
          disabled={!clickMode.enabled}
          onChange={(v) => updateClickMode({ max_elements: v })}
        />

        <Slider
          label="Stabilization Delay"
          title="Wait time before scanning UI elements. Increase if hints appear before the UI is ready."
          value={clickMode.ax_stabilization_delay_ms}
          min={0}
          max={300}
          step={10}
          disabled={!clickMode.enabled}
          formatValue={(v) => `${v}ms`}
          formatMin="0ms"
          formatMax="300ms"
          onChange={(v) => updateClickMode({ ax_stabilization_delay_ms: v })}
        />

        <Slider
          label="Cache Duration"
          title="How long to reuse scanned elements. Higher values make repeated activations faster."
          value={clickMode.cache_ttl_ms}
          min={0}
          max={2000}
          step={100}
          disabled={!clickMode.enabled}
          formatValue={(v) => `${v}ms`}
          formatMin="0ms"
          formatMax="2000ms"
          onChange={(v) => updateClickMode({ cache_ttl_ms: v })}
        />
      </div>
    </div>
  )
}
