import { useState, useEffect } from "react"
import { invoke } from "@tauri-apps/api/core"
import type { Settings } from "./SettingsApp"

interface Props {
  settings: Settings
  onUpdate: (updates: Partial<Settings>) => void
}

interface PermissionStatus {
  accessibility: boolean
  capture_running: boolean
}

export function GeneralSettings({ settings, onUpdate }: Props) {
  const [permissionStatus, setPermissionStatus] = useState<PermissionStatus | null>(null)
  const [version, setVersion] = useState<string>("")

  useEffect(() => {
    invoke<string>("get_version")
      .then(setVersion)
      .catch((e) => console.error("Failed to get version:", e))
  }, [])

  useEffect(() => {
    const checkPermissions = () => {
      invoke<PermissionStatus>("get_permission_status")
        .then(setPermissionStatus)
        .catch((e) => console.error("Failed to get permission status:", e))
    }
    checkPermissions()
    const interval = setInterval(checkPermissions, 2000)
    return () => clearInterval(interval)
  }, [])

  const handleOpenAccessibility = () => {
    invoke("open_accessibility_settings").catch(console.error)
  }

  const handleOpenInputMonitoring = () => {
    invoke("open_input_monitoring_settings").catch(console.error)
  }

  const handleRequestPermission = async () => {
    await invoke("request_permission")
    const status = await invoke<PermissionStatus>("get_permission_status")
    setPermissionStatus(status)
  }

  const permissionsOk = permissionStatus?.accessibility && permissionStatus?.capture_running

  return (
    <div className="settings-section">
      <h2>General Settings</h2>

      {version && (
        <div className="version-info">
          ovim v{version}
        </div>
      )}

      {permissionStatus && !permissionsOk && (
        <div className="permission-warning">
          <div className="permission-title">Permissions Required</div>
          <div className="permission-items">
            {!permissionStatus.accessibility && (
              <div className="permission-item">
                <span className="permission-status missing">Accessibility</span>
                <button type="button" className="permission-btn" onClick={handleRequestPermission}>
                  Request
                </button>
                <button type="button" className="permission-btn secondary" onClick={handleOpenAccessibility}>
                  Open Settings
                </button>
              </div>
            )}
            {!permissionStatus.capture_running && (
              <div className="permission-item">
                <span className="permission-status missing">Input Monitoring</span>
                <button type="button" className="permission-btn" onClick={handleOpenInputMonitoring}>
                  Open Settings
                </button>
              </div>
            )}
          </div>
          <div className="permission-hint">
            Grant permissions and restart the app for changes to take effect.
          </div>
        </div>
      )}

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
            checked={settings.auto_update_enabled}
            onChange={(e) => onUpdate({ auto_update_enabled: e.target.checked })}
          />
          Automatically check for updates
        </label>
      </div>

    </div>
  )
}
