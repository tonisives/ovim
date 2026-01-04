import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { GeneralSettings } from "./GeneralSettings";
import { IndicatorSettings } from "./IndicatorSettings";
import { WidgetSettings } from "./WidgetSettings";
import { IgnoredAppsSettings } from "./IgnoredAppsSettings";
import { NvimEditSettings } from "./NvimEditSettings";
import { ClickModeSettingsComponent } from "./ClickModeSettings";
import { ScrollModeSettingsComponent } from "./ScrollModeSettings";

export interface VimKeyModifiers {
  shift: boolean;
  control: boolean;
  option: boolean;
  command: boolean;
}

export type DoubleTapModifier = "none" | "command" | "option" | "control" | "shift" | "escape";

export interface NvimEditSettings {
  enabled: boolean;
  shortcut_key: string;
  shortcut_modifiers: VimKeyModifiers;
  terminal: string;
  terminal_path: string;
  editor: string;
  nvim_path: string;
  popup_mode: boolean;
  popup_width: number;
  popup_height: number;
  live_sync_enabled: boolean;
  use_custom_script: boolean;
  clipboard_mode: boolean;
  double_tap_modifier: DoubleTapModifier;
  domain_filetypes: Record<string, string>;
}

export interface ClickModeSettings {
  enabled: boolean;
  shortcut_key: string;
  shortcut_modifiers: VimKeyModifiers;
  double_tap_modifier: DoubleTapModifier;
  hint_chars: string;
  show_search_bar: boolean;
  hint_opacity: number;
  hint_font_size: number;
  hint_bg_color: string;
  hint_text_color: string;
  // Advanced timing settings
  ax_stabilization_delay_ms: number;
  cache_ttl_ms: number;
  // Advanced traversal settings
  max_depth: number;
  max_elements: number;
}

export interface ScrollModeSettings {
  enabled: boolean;
  scroll_step: number;
  enabled_apps: string[];
  overlay_blocklist: string[];
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
  indicator_visible: boolean;
  show_mode_in_menu_bar: boolean;
  mode_colors: ModeColors;
  indicator_font: string;
  ignored_apps: string[];
  launch_at_login: boolean;
  show_in_menu_bar: boolean;
  top_widget: string;
  bottom_widget: string;
  electron_apps: string[];
  nvim_edit: NvimEditSettings;
  click_mode: ClickModeSettings;
  scroll_mode: ScrollModeSettings;
  auto_update_enabled: boolean;
}

type TabId = "general" | "indicator" | "widgets" | "ignored" | "nvim-config" | "nvim-window" | "click-mode" | "scroll-mode";

export function SettingsApp() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [activeTab, setActiveTab] = useState<TabId>("general");

  useEffect(() => {
    // Load settings and domain filetypes separately
    // (domain_filetypes are stored in a separate file)
    Promise.all([
      invoke<Settings>("get_settings"),
      invoke<Record<string, string>>("get_domain_filetypes"),
    ])
      .then(([loadedSettings, domainFiletypes]) => {
        // Merge domain_filetypes into nvim_edit settings
        setSettings({
          ...loadedSettings,
          nvim_edit: {
            ...loadedSettings.nvim_edit,
            domain_filetypes: domainFiletypes,
          },
        });
      })
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

  const inPlaceModeTabs: { id: TabId; label: string; icon: string }[] = [
    { id: "indicator", label: "Indicator", icon: "diamond" },
    { id: "widgets", label: "Widgets", icon: "ruler" },
    { id: "ignored", label: "Ignored Apps", icon: "pause" },
  ];

  const editPopupTabs: { id: TabId; label: string; icon: string }[] = [
    { id: "nvim-config", label: "Config", icon: "gear" },
    { id: "nvim-window", label: "Window", icon: "window" },
  ];

  const clickModeTabs: { id: TabId; label: string; icon: string }[] = [
    { id: "click-mode", label: "Settings", icon: "cursor" },
  ];

  const scrollModeTabs: { id: TabId; label: string; icon: string }[] = [
    { id: "scroll-mode", label: "Settings", icon: "scroll" },
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

        <div className="tab-group">
          <span className="tab-group-label">Edit Popup</span>
          <div className="tab-group-tabs">
            {editPopupTabs.map((tab) => (
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

        <div className="tab-group">
          <span className="tab-group-label">Click Mode</span>
          <div className="tab-group-tabs">
            {clickModeTabs.map((tab) => (
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

        <div className="tab-group">
          <span className="tab-group-label">Scroll Mode</span>
          <div className="tab-group-tabs">
            {scrollModeTabs.map((tab) => (
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
        {(activeTab === "nvim-config" || activeTab === "nvim-window") && (
          <NvimEditSettings settings={settings} onUpdate={updateSettings} activeTab={activeTab} />
        )}
        {activeTab === "click-mode" && (
          <ClickModeSettingsComponent settings={settings} onUpdate={updateSettings} />
        )}
        {activeTab === "scroll-mode" && (
          <ScrollModeSettingsComponent settings={settings} onUpdate={updateSettings} />
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
    window: "\u25A1",
    cursor: "\u2316",
    scroll: "\u21C5",
  };
  return icons[name] || "";
}
