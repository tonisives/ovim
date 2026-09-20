import { PermissionSetup } from "./PermissionSetup"
import "../permissions.css"

export let PermissionsApp = () => (
  <main className="permissions-window">
    <div className="permissions-mark">OV</div>
    <h1>Allow ovim to control your Mac</h1>
    <p className="permissions-intro">
      ovim needs these two macOS permissions to provide Vim controls in every app.
    </p>
    <PermissionSetup onboarding />
    <p className="permissions-footnote">
      Each Allow button opens the correct System Settings page. Drag the ovim icon from the helper into the app list if needed.
    </p>
  </main>
)
