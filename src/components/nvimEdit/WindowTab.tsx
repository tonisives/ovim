import type { NvimEditSettings } from "../SettingsApp"

interface Props {
  nvimEdit: NvimEditSettings
  onUpdate: (updates: Partial<NvimEditSettings>) => void
}

export function WindowTab({ nvimEdit, onUpdate }: Props) {
  return (
    <>
      <div className="form-group">
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={nvimEdit.popup_mode}
            onChange={(e) => onUpdate({ popup_mode: e.target.checked })}
            disabled={!nvimEdit.enabled}
          />
          Open as popup below text field
        </label>
        <span className="hint">
          Position the terminal window directly below the text field instead of opening fullscreen
        </span>
      </div>

      {nvimEdit.popup_mode && (
        <div className="form-row">
          <div className="form-group">
            <label htmlFor="popup-width">Popup width (px)</label>
            <input
              type="number"
              id="popup-width"
              value={nvimEdit.popup_width}
              onChange={(e) => onUpdate({ popup_width: parseInt(e.target.value) || 0 })}
              min={0}
              disabled={!nvimEdit.enabled}
            />
            <span className="hint">0 = match text field width</span>
          </div>
          <div className="form-group">
            <label htmlFor="popup-height">Popup height (px)</label>
            <input
              type="number"
              id="popup-height"
              value={nvimEdit.popup_height}
              onChange={(e) => onUpdate({ popup_height: parseInt(e.target.value) || 300 })}
              min={100}
              disabled={!nvimEdit.enabled}
            />
          </div>
        </div>
      )}

    </>
  )
}
