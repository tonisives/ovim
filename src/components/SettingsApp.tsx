import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { GeneralSettings } from "./GeneralSettings";
import { IndicatorSettings } from "./IndicatorSettings";
import { WidgetSettings } from "./WidgetSettings";
import { IgnoredAppsSettings } from "./IgnoredAppsSettings";
import { NvimEditSettings } from "./NvimEditSettings";

export interface VimKeyModifiers {
  shift: boolean;
  control: boolean;
  option: boolean;
  command: boolean;
}

export interface NvimEditSettings {
  enabled: boolean;
  shortcut_key: string;
  shortcut_modifiers: VimKeyModifiers;
  terminal: string;
  nvim_path: string;
}

export interface Settings {
  vim_key: string;
  vim_key_modifiers: VimKeyModifiers;
  indicator_position: number;
  indicator_opacity: number;
  indicator_size: number;
  ignored_apps: string[];
  launch_at_login: boolean;
  show_in_menu_bar: boolean;
  top_widget: string;
  bottom_widget: string;
  electron_apps: string[];
  nvim_edit: NvimEditSettings;
}

type TabId = "general" | "indicator" | "widgets" | "ignored" | "nvim";

export function SettingsApp() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [activeTab, setActiveTab] = useState<TabId>("general");

  useEffect(() => {
    invoke<Settings>("get_settings")
      .then(setSettings)
      .catch((e) => console.error("Failed to load settings:", e));
  }, []);

  const updateSettings = async (updates: Partial<Settings>) => {
    if (!settings) return;

    const newSettings = { ...settings, ...updates };
    setSettings(newSettings);

    try {
      await invoke("set_settings", { newSettings });
    } catch (e) {
      console.error("Failed to save settings:", e);
    }
  };

  if (!settings) {
    return <div className="loading">Loading settings...</div>;
  }

  const tabs: { id: TabId; label: string; icon: string }[] = [
    { id: "general", label: "General", icon: "gear" },
    { id: "indicator", label: "Indicator", icon: "diamond" },
    { id: "widgets", label: "Widgets", icon: "ruler" },
    { id: "ignored", label: "Ignored Apps", icon: "pause" },
    { id: "nvim", label: "Nvim Edit", icon: "edit" },
  ];

  return (
    <div className="settings-container">
      <div className="tabs">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            className={`tab ${activeTab === tab.id ? "active" : ""}`}
            onClick={() => setActiveTab(tab.id)}
          >
            <span className="tab-icon">{getIcon(tab.icon)}</span>
            {tab.label}
          </button>
        ))}
      </div>

      <div className="tab-content">
        {activeTab === "general" && (
          <GeneralSettings settings={settings} onUpdate={updateSettings} />
        )}
        {activeTab === "indicator" && (
          <IndicatorSettings settings={settings} onUpdate={updateSettings} />
        )}
        {activeTab === "widgets" && (
          <WidgetSettings settings={settings} onUpdate={updateSettings} />
        )}
        {activeTab === "ignored" && (
          <IgnoredAppsSettings settings={settings} onUpdate={updateSettings} />
        )}
        {activeTab === "nvim" && (
          <NvimEditSettings settings={settings} onUpdate={updateSettings} />
        )}
      </div>

    </div>
  );
}

function getIcon(name: string): string {
  const icons: Record<string, string> = {
    gear: "\u2699",
    diamond: "\u25C6",
    ruler: "\u25A6",
    pause: "\u23F8",
    edit: "\u270E",
  };
  return icons[name] || "";
}
