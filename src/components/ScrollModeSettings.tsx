import { useCallback } from "react"
import { invoke } from "@tauri-apps/api/core"
import type { Settings, ScrollModeSettings } from "./SettingsApp"
import { AppList } from "./AppList"
import { Slider } from "./common"

interface Props {
  settings: Settings
  onUpdate: (updates: Partial<Settings>) => void
}

export function ScrollModeSettingsComponent({ settings, onUpdate }: Props) {
  const scrollMode = settings.scroll_mode

  const updateScrollMode = useCallback(
    (updates: Partial<ScrollModeSettings>) => {
      onUpdate({
        scroll_mode: { ...scrollMode, ...updates },
      })
    },
    [scrollMode, onUpdate],
  )

  const handleAddEnabledApp = useCallback(async () => {
    try {
      const bundleId = await invoke<string | null>("pick_app")
      if (bundleId && !scrollMode.enabled_apps.includes(bundleId)) {
        updateScrollMode({
          enabled_apps: [...scrollMode.enabled_apps, bundleId],
        })
      }
    } catch (e) {
      console.error("Failed to pick app:", e)
    }
  }, [scrollMode.enabled_apps, updateScrollMode])

  const handleRemoveEnabledApp = useCallback(
    (bundleId: string) => {
      updateScrollMode({
        enabled_apps: scrollMode.enabled_apps.filter((id) => id !== bundleId),
      })
    },
    [scrollMode.enabled_apps, updateScrollMode],
  )

  const handleAddManualApp = useCallback(
    (bundleId: string) => {
      if (!scrollMode.enabled_apps.includes(bundleId)) {
        updateScrollMode({
          enabled_apps: [...scrollMode.enabled_apps, bundleId],
        })
      }
    },
    [scrollMode.enabled_apps, updateScrollMode],
  )

  const handleAddBlocklistApp = useCallback(async () => {
    try {
      const bundleId = await invoke<string | null>("pick_app")
      if (bundleId && !scrollMode.overlay_blocklist.includes(bundleId)) {
        updateScrollMode({
          overlay_blocklist: [...scrollMode.overlay_blocklist, bundleId],
        })
      }
    } catch (e) {
      console.error("Failed to pick app:", e)
    }
  }, [scrollMode.overlay_blocklist, updateScrollMode])

  const handleAddManualBlocklistApp = useCallback(
    (bundleId: string) => {
      if (!scrollMode.overlay_blocklist.includes(bundleId)) {
        updateScrollMode({
          overlay_blocklist: [...scrollMode.overlay_blocklist, bundleId],
        })
      }
    },
    [scrollMode.overlay_blocklist, updateScrollMode],
  )

  const handleRemoveBlocklistApp = useCallback(
    (bundleId: string) => {
      updateScrollMode({
        overlay_blocklist: scrollMode.overlay_blocklist.filter((id) => id !== bundleId),
      })
    },
    [scrollMode.overlay_blocklist, updateScrollMode],
  )

  return (
    <div className="settings-section">
      <div className="section-header">
        <h2>Scroll Mode</h2>
      </div>
      <p className="section-description">
        Vimium-style keyboard navigation for scrolling pages. Use h/j/k/l to scroll,
        gg/G for top/bottom, d/u for half-page, and more.
      </p>

      {/* Enable/Disable Toggle */}
      <div className="form-group">
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={scrollMode.enabled}
            onChange={(e) => updateScrollMode({ enabled: e.target.checked })}
          />
          Enable Scroll Mode
        </label>
        <span className="hint">
          When enabled, scroll shortcuts work when vim is in Insert mode
        </span>
      </div>

      {/* Scroll Step */}
      <div className="indicator-controls" style={{ marginTop: 16 }}>
        <Slider
          label="Scroll Speed"
          value={scrollMode.scroll_step}
          min={50}
          max={300}
          step={10}
          disabled={!scrollMode.enabled}
          formatValue={(v) => `${v}px`}
          formatMin="50px"
          formatMax="300px"
          onChange={(v) => updateScrollMode({ scroll_step: v })}
        />
      </div>

      {/* Keyboard Shortcuts Reference */}
      <div className="color-settings">
        <h3>Keyboard Shortcuts</h3>
        <div className="shortcuts-table">
          <table>
            <tbody>
              <tr><td className="shortcut-key">h / j / k / l</td><td>Scroll left / down / up / right</td></tr>
              <tr><td className="shortcut-key">gg</td><td>Scroll to top</td></tr>
              <tr><td className="shortcut-key">G</td><td>Scroll to bottom</td></tr>
              <tr><td className="shortcut-key">d / u</td><td>Half page down / up</td></tr>
              <tr><td className="shortcut-key">/</td><td>Open find (Cmd+F)</td></tr>
              <tr><td className="shortcut-key">H / L</td><td>History back / forward</td></tr>
              <tr><td className="shortcut-key">r / R</td><td>Reload / Hard reload</td></tr>
            </tbody>
          </table>
        </div>
      </div>

      {/* Enabled Apps */}
      <div className="color-settings">
        <h3>Enabled Applications</h3>
        <p className="help-text">
          Scroll mode only works in these apps. Browsers and common system apps are enabled by default.
        </p>
        <AppList
          items={scrollMode.enabled_apps}
          onAdd={handleAddEnabledApp}
          onAddManual={handleAddManualApp}
          onRemove={handleRemoveEnabledApp}
        />
      </div>

      {/* Overlay Blocklist */}
      <div className="color-settings">
        <h3>Overlay Blocklist</h3>
        <p className="help-text">
          Scroll mode is disabled when these apps have visible windows (e.g., Keyboard Maestro palettes).
        </p>
        <AppList
          items={scrollMode.overlay_blocklist}
          onAdd={handleAddBlocklistApp}
          onAddManual={handleAddManualBlocklistApp}
          onRemove={handleRemoveBlocklistApp}
        />
      </div>
    </div>
  )
}
