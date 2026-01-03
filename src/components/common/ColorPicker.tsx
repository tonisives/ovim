interface ColorPickerProps {
  label: string
  value: string
  disabled?: boolean
  onChange: (value: string) => void
}

export function ColorPicker({ label, value, disabled, onChange }: ColorPickerProps) {
  return (
    <div className="color-picker-group">
      <label>{label}</label>
      <div className="color-input-wrapper">
        <input
          type="color"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          disabled={disabled}
        />
        <span className="color-hex">{value}</span>
      </div>
    </div>
  )
}
