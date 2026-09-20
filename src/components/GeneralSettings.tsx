import { useState, useEffect } from "react"
import { invoke } from "@tauri-apps/api/core"
import type { Settings } from "./SettingsApp"
import { PermissionSetup } from "./PermissionSetup"

interface Props {
  settings: Settings
  onUpdate: (updates: Partial<Settings>) => void
}

export function GeneralSettings({ settings, onUpdate }: Props) {
  const [version, setVersion] = useState<string>("")

  useEffect(() => {
    invoke<string>("get_version")
      .then(setVersion)
      .catch((e) => console.error("Failed to get version:", e))
  }, [])

  return (
    <div className="settings-section">
      <h2>General Settings</h2>

      {version && (
        <div className="version-info">
          ovim v{version}
        </div>
      )}

      <PermissionSetup />

      <div className="form-group">
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={settings.launch_at_login}
            onChange={(e) => onUpdate({ launch_at_login: e.target.checked })}
          />
          Launch ovim at login
        </label>
      </div>

      <div className="form-group">
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={settings.show_in_dock}
            onChange={(e) => onUpdate({ show_in_dock: e.target.checked })}
          />
          Show ovim in the Dock
        </label>
      </div>

      <div className="form-group">
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={settings.auto_update_enabled}
            onChange={(e) => onUpdate({ auto_update_enabled: e.target.checked })}
          />
          Automatically check for updates
        </label>
      </div>

    </div>
  )
}
