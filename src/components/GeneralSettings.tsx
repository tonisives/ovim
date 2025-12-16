import type { Settings } from "./SettingsApp"

interface Props {
  settings: Settings
  onUpdate: (updates: Partial<Settings>) => void
}

const VIM_KEY_OPTIONS = [
  { value: "caps_lock", label: "Caps Lock" },
  { value: "escape", label: "Escape" },
  { value: "right_control", label: "Right Control" },
  { value: "right_option", label: "Right Option" },
]

const ICON_STYLE_OPTIONS = [
  { value: true, label: "Menu Bar" },
  { value: false, label: "Hidden" },
]

export function GeneralSettings({ settings, onUpdate }: Props) {
  return (
    <div className="settings-section">
      <h2>General Settings</h2>

      <div className="form-group">
        <label htmlFor="vim-key">Vim mode key</label>
        <select
          id="vim-key"
          value={settings.vim_key}
          onChange={(e) => onUpdate({ vim_key: e.target.value })}
        >
          {VIM_KEY_OPTIONS.map((opt) => (
            <option key={opt.value} value={opt.value}>
              {opt.label}
            </option>
          ))}
        </select>
      </div>

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
        <label htmlFor="icon-style">Menu bar icon</label>
        <select
          id="icon-style"
          value={settings.show_in_menu_bar ? "true" : "false"}
          onChange={(e) => onUpdate({ show_in_menu_bar: e.target.value === "true" })}
        >
          {ICON_STYLE_OPTIONS.map((opt) => (
            <option key={String(opt.value)} value={String(opt.value)}>
              {opt.label}
            </option>
          ))}
        </select>
      </div>
    </div>
  )
}
