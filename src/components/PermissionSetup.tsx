import { useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"

export type PermissionStatus = {
  accessibility: boolean
  input_monitoring: boolean
  capture_running: boolean
}

type PermissionSetupProps = {
  onboarding?: boolean
}

export let PermissionSetup = ({ onboarding = false }: PermissionSetupProps) => {
  let [permissionStatus, setPermissionStatus] = useState<PermissionStatus | null>(null)
  let [completionStarted, setCompletionStarted] = useState(false)
  let [errorMessage, setErrorMessage] = useState("")

  useEffect(() => {
    let checkPermissions = () => {
      invoke<PermissionStatus>("get_permission_status")
        .then(setPermissionStatus)
        .catch((error) => setErrorMessage(String(error)))
    }

    checkPermissions()
    let interval = window.setInterval(checkPermissions, 1000)
    return () => window.clearInterval(interval)
  }, [])

  useEffect(() => {
    if (!onboarding || completionStarted || !permissionStatus?.accessibility || !permissionStatus.input_monitoring) {
      return
    }

    setCompletionStarted(true)
    invoke("complete_permission_setup").catch((error) => {
      setCompletionStarted(false)
      setErrorMessage(String(error))
    })
  }, [completionStarted, onboarding, permissionStatus])

  let handleAllowAccessibility = () => {
    setErrorMessage("")
    invoke("open_accessibility_settings").catch((error) => setErrorMessage(String(error)))
  }

  let handleAllowInputMonitoring = () => {
    setErrorMessage("")
    invoke("open_input_monitoring_settings").catch((error) => setErrorMessage(String(error)))
  }

  if (!permissionStatus) {
    return <div className="permission-loading">Checking permissions...</div>
  }

  if (!onboarding && permissionStatus.accessibility && permissionStatus.input_monitoring) {
    return null
  }

  return (
    <div className={`permission-setup ${onboarding ? "permission-setup-onboarding" : ""}`}>
      <div className="permission-card">
        <div className="permission-copy">
          <div className="permission-name">Accessibility</div>
          <div className="permission-description">
            Lets ovim read focused controls and move between interface elements.
          </div>
        </div>
        {permissionStatus.accessibility ? (
          <span className="permission-allowed">Allowed</span>
        ) : (
          <button type="button" className="permission-allow" onClick={handleAllowAccessibility}>
            Allow
          </button>
        )}
      </div>

      <div className="permission-card">
        <div className="permission-copy">
          <div className="permission-name">Input Monitoring</div>
          <div className="permission-description">
            Lets ovim respond to keyboard shortcuts across your Mac.
          </div>
        </div>
        {permissionStatus.input_monitoring ? (
          <span className="permission-allowed">Allowed</span>
        ) : (
          <button type="button" className="permission-allow" onClick={handleAllowInputMonitoring}>
            Allow
          </button>
        )}
      </div>

      {errorMessage && <div className="permission-error">{errorMessage}</div>}
    </div>
  )
}
