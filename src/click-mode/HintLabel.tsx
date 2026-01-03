import { CSSProperties, useMemo } from "react"
import { ClickableElement, ClickModeStyleSettings } from "./types"

interface WindowOffset {
  x: number
  y: number
}

interface HintLabelProps {
  element: ClickableElement
  inputBuffer: string
  styleSettings: ClickModeStyleSettings
  animationDelay?: number
  windowOffset: WindowOffset
}

/** Individual hint label that appears on a clickable element */
export function HintLabel({
  element,
  inputBuffer,
  styleSettings,
  animationDelay = 0,
  windowOffset,
}: HintLabelProps) {
  const { hint, x, y } = element
  const inputUpper = inputBuffer.toUpperCase()

  // Determine which part of the hint matches the input
  const matchedPart = hint.startsWith(inputUpper) ? inputUpper : ""
  const unmatchedPart = hint.slice(matchedPart.length)

  // Check if this hint is filtered out
  const isFiltered = inputBuffer.length > 0 && !hint.startsWith(inputUpper)
  if (isFiltered) {
    return null
  }

  // Convert screen coordinates to window-relative coordinates
  // Element x,y are in screen coordinates, we need to subtract the window's position
  const labelX = x - windowOffset.x + 2
  const labelY = y - windowOffset.y + 2

  const labelStyle: CSSProperties = useMemo(
    () => ({
      position: "fixed",
      left: labelX,
      top: labelY,
      backgroundColor: styleSettings.hint_bg_color,
      color: styleSettings.hint_text_color,
      fontFamily: "SF Mono, Monaco, Menlo, monospace",
      fontSize: `${styleSettings.hint_font_size}px`,
      fontWeight: 700,
      padding: "1px 4px",
      borderRadius: "3px",
      boxShadow: "0 1px 4px rgba(0,0,0,0.4)",
      zIndex: 999999,
      pointerEvents: "none",
      whiteSpace: "nowrap",
      letterSpacing: "0.5px",
      border: "1px solid rgba(0,0,0,0.15)",
      opacity: styleSettings.hint_opacity,
      animation: `hint-fade-in 0.1s ease-out ${animationDelay}ms both`,
      textTransform: "uppercase",
    }),
    [labelX, labelY, styleSettings, animationDelay]
  )

  const matchedStyle: CSSProperties = {
    opacity: 0.4,
    textDecoration: "none",
  }

  return (
    <div style={labelStyle} className="hint-label">
      {matchedPart && <span style={matchedStyle}>{matchedPart}</span>}
      <span>{unmatchedPart}</span>
    </div>
  )
}
