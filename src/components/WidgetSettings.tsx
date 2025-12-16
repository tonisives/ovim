import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "./SettingsApp";
import { AppList } from "./AppList";

interface Props {
  settings: Settings;
  onUpdate: (updates: Partial<Settings>) => void;
}

const WIDGET_OPTIONS = [
  { value: "None", label: "None" },
  { value: "Time", label: "Time (HH:MM)" },
  { value: "Date", label: "Date" },
  { value: "CharacterCount", label: "Selected chars" },
  { value: "LineCount", label: "Selected lines" },
  { value: "CharacterAndLineCount", label: "Chars + lines" },
  { value: "Battery", label: "Battery %" },
  { value: "CapsLock", label: "Caps Lock" },
  { value: "KeystrokeBuffer", label: "Pending keys" },
];

export function WidgetSettings({ settings, onUpdate }: Props) {
  const handleAddElectronApp = async () => {
    try {
      const bundleId = await invoke<string | null>("pick_app");
      if (bundleId && !settings.electron_apps.includes(bundleId)) {
        onUpdate({ electron_apps: [...settings.electron_apps, bundleId] });
      }
    } catch (e) {
      console.error("Failed to pick app:", e);
    }
  };

  const handleRemoveElectronApp = (bundleId: string) => {
    onUpdate({
      electron_apps: settings.electron_apps.filter((id) => id !== bundleId),
    });
  };

  return (
    <div className="settings-section">
      <h2>Widgets</h2>

      <div className="widget-pickers">
        <div className="form-group">
          <label htmlFor="top-widget">Top widget</label>
          <select
            id="top-widget"
            value={settings.top_widget}
            onChange={(e) => onUpdate({ top_widget: e.target.value })}
          >
            {WIDGET_OPTIONS.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>
        </div>

        <div className="form-group">
          <label htmlFor="bottom-widget">Bottom widget</label>
          <select
            id="bottom-widget"
            value={settings.bottom_widget}
            onChange={(e) => onUpdate({ bottom_widget: e.target.value })}
          >
            {WIDGET_OPTIONS.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>
        </div>
      </div>

      <p className="help-text">
        Accessibility is used to get the selected text. Check that it is enabled
        in Privacy settings.
      </p>

      <div className="electron-apps-section">
        <h3>Enable selection observing in Electron apps</h3>
        <AppList
          items={settings.electron_apps}
          onAdd={handleAddElectronApp}
          onRemove={handleRemoveElectronApp}
        />
        <p className="help-text">
          Observing selection in Electron apps requires more performance.
        </p>
        <p className="help-text warning">
          Removing app from the list requires a re-login.
        </p>
      </div>
    </div>
  );
}
