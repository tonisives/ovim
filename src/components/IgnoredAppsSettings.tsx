import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "./SettingsApp";
import { AppList } from "./AppList";

interface Props {
  settings: Settings;
  onUpdate: (updates: Partial<Settings>) => void;
}

export function IgnoredAppsSettings({ settings, onUpdate }: Props) {
  const handleAddApp = async () => {
    try {
      const bundleId = await invoke<string | null>("pick_app");
      if (bundleId && !settings.ignored_apps.includes(bundleId)) {
        onUpdate({ ignored_apps: [...settings.ignored_apps, bundleId] });
      }
    } catch (e) {
      console.error("Failed to pick app:", e);
    }
  };

  const handleAddManualApp = (bundleId: string) => {
    if (!settings.ignored_apps.includes(bundleId)) {
      onUpdate({ ignored_apps: [...settings.ignored_apps, bundleId] });
    }
  };

  const handleRemoveApp = (bundleId: string) => {
    onUpdate({
      ignored_apps: settings.ignored_apps.filter((id) => id !== bundleId),
    });
  };

  return (
    <div className="settings-section">
      <h2>Ignored Apps</h2>
      <p className="section-description">
        Vim modifications are disabled in these applications.
      </p>

      <AppList
        items={settings.ignored_apps}
        onAdd={handleAddApp}
        onAddManual={handleAddManualApp}
        onRemove={handleRemoveApp}
      />
    </div>
  );
}
