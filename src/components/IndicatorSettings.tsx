import { useState, useEffect } from "react";
import type { Settings, RgbColor, ModeColors } from "./SettingsApp";
import {
  formatKeyWithModifiers,
  hasAnyModifier,
  recordKey,
  cancelRecordKey,
  getKeyDisplayName,
} from "./keyRecording";

const PRESET_KEYS = [
  { value: "caps_lock", label: "Caps Lock" },
  { value: "escape", label: "Escape" },
  { value: "right_control", label: "Right Control" },
  { value: "right_option", label: "Right Option" },
];

interface Props {
  settings: Settings;
  onUpdate: (updates: Partial<Settings>) => void;
}

const POSITION_OPTIONS = [
  { value: 0, label: "Top Left" },
  { value: 1, label: "Top Middle" },
  { value: 2, label: "Top Right" },
  { value: 3, label: "Bottom Left" },
  { value: 4, label: "Bottom Middle" },
  { value: 5, label: "Bottom Right" },
];

const FONT_OPTIONS = [
  { value: "system-ui, -apple-system, sans-serif", label: "System (Default)" },
  { value: "SF Pro Display, -apple-system, sans-serif", label: "SF Pro Display" },
  { value: "SF Pro Rounded, -apple-system, sans-serif", label: "SF Pro Rounded" },
  { value: "Helvetica Neue, sans-serif", label: "Helvetica Neue" },
  { value: "Arial, sans-serif", label: "Arial" },
  { value: "SF Mono, Monaco, monospace", label: "SF Mono" },
  { value: "Menlo, monospace", label: "Menlo" },
  { value: "Monaco, monospace", label: "Monaco" },
  { value: "Courier New, monospace", label: "Courier New" },
];

function rgbToHex(color: RgbColor): string {
  const toHex = (n: number) => n.toString(16).padStart(2, "0");
  return `#${toHex(color.r)}${toHex(color.g)}${toHex(color.b)}`;
}

function hexToRgb(hex: string): RgbColor {
  const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
  if (!result) return { r: 128, g: 128, b: 128 };
  return {
    r: parseInt(result[1], 16),
    g: parseInt(result[2], 16),
    b: parseInt(result[3], 16),
  };
}

export function IndicatorSettings({ settings, onUpdate }: Props) {
  const [isRecording, setIsRecording] = useState(false);
  const [displayName, setDisplayName] = useState<string | null>(null);

  useEffect(() => {
    getKeyDisplayName(settings.vim_key)
      .then((name) => {
        if (name && hasAnyModifier(settings.vim_key_modifiers)) {
          setDisplayName(formatKeyWithModifiers(name, settings.vim_key_modifiers));
        } else {
          setDisplayName(name);
        }
      })
      .catch(() => setDisplayName(null));
  }, [settings.vim_key, settings.vim_key_modifiers]);

  const handleRecordKey = async () => {
    setIsRecording(true);
    try {
      const recorded = await recordKey();
      onUpdate({
        vim_key: recorded.name,
        vim_key_modifiers: recorded.modifiers,
      });
      const formatted = formatKeyWithModifiers(recorded.display_name, recorded.modifiers);
      setDisplayName(formatted);
    } catch (e) {
      console.error("Failed to record key:", e);
    } finally {
      setIsRecording(false);
    }
  };

  const handleCancelRecord = () => {
    cancelRecordKey().catch(() => {});
    setIsRecording(false);
  };

  const handlePresetSelect = (value: string) => {
    onUpdate({
      vim_key: value,
      vim_key_modifiers: { shift: false, control: false, option: false, command: false },
    });
  };

  const isPresetKey = PRESET_KEYS.some((k) => k.value === settings.vim_key) && !hasAnyModifier(settings.vim_key_modifiers);

  const updateModeColor = (mode: keyof ModeColors, hex: string) => {
    const newColors = {
      ...settings.mode_colors,
      [mode]: hexToRgb(hex),
    };
    onUpdate({ mode_colors: newColors });
  };

  return (
    <div className="settings-section">
      <h2>Indicator</h2>

      <div className="indicator-controls">
        <div className="form-group checkbox-group">
          <label className="checkbox-label">
            <input
              type="checkbox"
              checked={settings.enabled}
              onChange={(e) => onUpdate({ enabled: e.target.checked })}
            />
            <span>Enable vim mode and indicator</span>
          </label>
          <p className="setting-description">
            When disabled, all key presses pass through normally and the indicator is hidden.
          </p>
        </div>

        <div className="form-group checkbox-group">
          <label className="checkbox-label">
            <input
              type="checkbox"
              checked={settings.indicator_visible}
              onChange={(e) => onUpdate({ indicator_visible: e.target.checked })}
            />
            <span>Show floating indicator</span>
          </label>
          <p className="setting-description">
            Display the mode indicator as a floating window on screen.
          </p>
        </div>

        <div className="form-group checkbox-group">
          <label className="checkbox-label">
            <input
              type="checkbox"
              checked={settings.show_mode_in_menu_bar}
              onChange={(e) => onUpdate({ show_mode_in_menu_bar: e.target.checked })}
            />
            <span>Show mode in menu bar</span>
          </label>
          <p className="setting-description">
            Display the current mode (N/I/V) in the menu bar icon.
          </p>
        </div>

        <div className="form-group">
          <label htmlFor="vim-key">Vim mode key</label>
          <div className="key-selector">
            <select
              id="vim-key"
              value={isPresetKey ? settings.vim_key : ""}
              onChange={(e) => handlePresetSelect(e.target.value)}
              disabled={isRecording}
            >
              {PRESET_KEYS.map((opt) => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
              {!isPresetKey && (
                <option value="" disabled>
                  {displayName || settings.vim_key}
                </option>
              )}
            </select>
            {isRecording ? (
              <button
                type="button"
                className="record-key-btn recording"
                onClick={handleCancelRecord}
              >
                Press any key...
              </button>
            ) : (
              <button
                type="button"
                className="record-key-btn"
                onClick={handleRecordKey}
              >
                Record Key
              </button>
            )}
          </div>
        </div>

        <div className="slider-group">
          <label>
            Alpha: {Math.round(settings.indicator_opacity * 100)}%
          </label>
          <input
            type="range"
            min="0"
            max="100"
            value={settings.indicator_opacity * 100}
            onChange={(e) =>
              onUpdate({ indicator_opacity: Number(e.target.value) / 100 })
            }
          />
          <div className="slider-labels">
            <span>0%</span>
            <span>100%</span>
          </div>
        </div>

        <div className="position-group">
          <label>Location</label>
          <div className="position-options">
            {POSITION_OPTIONS.map((opt) => (
              <label key={opt.value} className="radio-label">
                <input
                  type="radio"
                  name="position"
                  value={opt.value}
                  checked={settings.indicator_position === opt.value}
                  onChange={() => onUpdate({ indicator_position: opt.value })}
                />
                {opt.label}
              </label>
            ))}
          </div>
        </div>

        <div className="slider-group">
          <label>
            X Offset: {settings.indicator_offset_x}px
          </label>
          <input
            type="range"
            min="-200"
            max="200"
            value={settings.indicator_offset_x}
            onChange={(e) =>
              onUpdate({ indicator_offset_x: Number(e.target.value) })
            }
          />
          <div className="slider-labels">
            <span>-200</span>
            <span>0</span>
            <span>+200</span>
          </div>
        </div>

        <div className="slider-group">
          <label>
            Y Offset: {settings.indicator_offset_y}px
          </label>
          <input
            type="range"
            min="-200"
            max="200"
            value={settings.indicator_offset_y}
            onChange={(e) =>
              onUpdate({ indicator_offset_y: Number(e.target.value) })
            }
          />
          <div className="slider-labels">
            <span>-200</span>
            <span>0</span>
            <span>+200</span>
          </div>
        </div>

        <div className="slider-group">
          <label>
            Size: {settings.indicator_size.toFixed(1)}x
          </label>
          <input
            type="range"
            min="50"
            max="200"
            value={settings.indicator_size * 100}
            onChange={(e) =>
              onUpdate({ indicator_size: Number(e.target.value) / 100 })
            }
          />
          <div className="slider-labels">
            <span>small</span>
            <span>big</span>
          </div>
        </div>
      </div>

      <div className="color-settings">
        <h3>Mode Colors</h3>
        <div className="color-pickers">
          <div className="color-picker-group">
            <label>Insert Mode</label>
            <div className="color-input-wrapper">
              <input
                type="color"
                value={rgbToHex(settings.mode_colors.insert)}
                onChange={(e) => updateModeColor("insert", e.target.value)}
              />
              <span className="color-hex">{rgbToHex(settings.mode_colors.insert)}</span>
            </div>
          </div>

          <div className="color-picker-group">
            <label>Normal Mode</label>
            <div className="color-input-wrapper">
              <input
                type="color"
                value={rgbToHex(settings.mode_colors.normal)}
                onChange={(e) => updateModeColor("normal", e.target.value)}
              />
              <span className="color-hex">{rgbToHex(settings.mode_colors.normal)}</span>
            </div>
          </div>

          <div className="color-picker-group">
            <label>Visual Mode</label>
            <div className="color-input-wrapper">
              <input
                type="color"
                value={rgbToHex(settings.mode_colors.visual)}
                onChange={(e) => updateModeColor("visual", e.target.value)}
              />
              <span className="color-hex">{rgbToHex(settings.mode_colors.visual)}</span>
            </div>
          </div>
        </div>
      </div>

      <div className="font-settings">
        <h3>Font</h3>
        <div className="form-group">
          <select
            value={settings.indicator_font}
            onChange={(e) => onUpdate({ indicator_font: e.target.value })}
          >
            {FONT_OPTIONS.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>
        </div>
        <div className="font-preview">
          <span
            className="font-preview-text"
            style={{ fontFamily: settings.indicator_font }}
          >
            n i v
          </span>
        </div>
      </div>
    </div>
  );
}
