interface SliderProps {
  label: string
  value: number
  min: number
  max: number
  step: number
  disabled?: boolean
  title?: string
  formatValue?: (value: number) => string
  formatMin?: string
  formatMax?: string
  onChange: (value: number) => void
}

export function Slider({
  label,
  value,
  min,
  max,
  step,
  disabled,
  title,
  formatValue,
  formatMin,
  formatMax,
  onChange,
}: SliderProps) {
  const displayValue = formatValue ? formatValue(value) : String(value)
  const displayMin = formatMin ?? String(min)
  const displayMax = formatMax ?? String(max)

  return (
    <div className="slider-group">
      <label title={title}>{label}</label>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(parseFloat(e.target.value))}
        disabled={disabled}
        title={title}
      />
      <div className="slider-labels">
        <span>{displayMin}</span>
        <span>{displayValue}</span>
        <span>{displayMax}</span>
      </div>
    </div>
  )
}
