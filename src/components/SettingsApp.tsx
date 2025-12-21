import { useEffect, useState, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
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
  popup_mode: boolean;
  popup_width: number;
  popup_height: number;
}

export interface RgbColor {
  r: number;
  g: number;
  b: number;
}

export interface ModeColors {
  insert: RgbColor;
  normal: RgbColor;
  visual: RgbColor;
}

export interface Settings {
  enabled: boolean;
  vim_key: string;
  vim_key_modifiers: VimKeyModifiers;
  indicator_position: number;
  indicator_opacity: number;
  indicator_size: number;
  indicator_offset_x: number;
  indicator_offset_y: number;
  mode_colors: ModeColors;
  indicator_font: string;
  ignored_apps: string[];
  launch_at_login: boolean;
  show_in_menu_bar: boolean;
  top_widget: string;
  bottom_widget: string;
  electron_apps: string[];
  nvim_edit: NvimEditSettings;
}

type TabId = "general" | "indicator" | "widgets" | "ignored" | "nvim";

const MIN_HEIGHT = 400;
const MAX_HEIGHT = 800;
const WINDOW_WIDTH = 600;
const TABS_HEIGHT = 45; // Height of tabs bar

export function SettingsApp() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [activeTab, setActiveTab] = useState<TabId>("general");
  const contentRef = useRef<HTMLDivElement>(null);

  const resizeWindow = useCallback(async () => {
    if (!contentRef.current) return;

    // Get the scrollHeight of the content (actual content height)
    const contentHeight = contentRef.current.scrollHeight;
    // Add tabs height and padding buffer
    const totalHeight = contentHeight + TABS_HEIGHT + 40;
    // Clamp between min and max
    const newHeight = Math.min(MAX_HEIGHT, Math.max(MIN_HEIGHT, totalHeight));

    try {
      const window = getCurrentWindow();
      await window.setSize(new LogicalSize(WINDOW_WIDTH, newHeight));
    } catch (e) {
      console.error("Failed to resize window:", e);
    }
  }, []);

  useEffect(() => {
    invoke<Settings>("get_settings")
      .then(setSettings)
      .catch((e) => console.error("Failed to load settings:", e));
  }, []);

  // Resize window when tab changes or settings change
  useEffect(() => {
    // Small delay to let content render
    const timer = setTimeout(resizeWindow, 50);
    return () => clearTimeout(timer);
  }, [activeTab, settings, resizeWindow]);

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

  const inPlaceModeTabs: { id: TabId; label: string; icon: string }[] = [
    { id: "indicator", label: "Indicator", icon: "diamond" },
    { id: "widgets", label: "Widgets", icon: "ruler" },
    { id: "ignored", label: "Ignored Apps", icon: "pause" },
  ];

  return (
    <div className="settings-container">
      <div className="tabs">
        <button
          className={`tab ${activeTab === "general" ? "active" : ""}`}
          onClick={() => setActiveTab("general")}
        >
          <span className="tab-icon">{getIcon("gear")}</span>
          General
        </button>

        <div className="tab-group">
          <span className="tab-group-label">In-Place Mode</span>
          <div className="tab-group-tabs">
            {inPlaceModeTabs.map((tab) => (
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
        </div>

        <button
          className={`tab ${activeTab === "nvim" ? "active" : ""}`}
          onClick={() => setActiveTab("nvim")}
        >
          <span className="tab-icon">{getIcon("edit")}</span>
          Edit Popup
        </button>
      </div>

      <div className="tab-content" ref={contentRef}>
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
