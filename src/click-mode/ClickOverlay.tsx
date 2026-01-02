import { CSSProperties } from "react"
import { HintLabel } from "./HintLabel"
import { useClickModeEvents, useClickModeKeyboard } from "./hooks"
import "./click-overlay.css"

/** Main overlay component that displays hint labels */
export function ClickOverlay() {
  const {
    elements,
    isActive,
    windowOffset,
    styleSettings,
    inputBuffer,
    setInputBuffer,
  } = useClickModeEvents()

  useClickModeKeyboard({
    isActive,
    elements,
    inputBuffer,
    setInputBuffer,
  })

  if (!isActive) {
    return null
  }

  const overlayStyle: CSSProperties = {
    position: "fixed",
    top: 0,
    left: 0,
    width: "100vw",
    height: "100vh",
    backgroundColor: "rgba(0, 0, 0, 0.1)",
    pointerEvents: "auto",
  }

  const inputIndicatorStyle: CSSProperties = {
    position: "fixed",
    top: 20,
    left: "50%",
    transform: "translateX(-50%)",
    backgroundColor: "rgba(0, 0, 0, 0.85)",
    color: styleSettings.hint_bg_color,
    fontFamily: "SF Mono, Monaco, Menlo, monospace",
    fontSize: "24px",
    fontWeight: 700,
    padding: "8px 20px",
    borderRadius: "8px",
    zIndex: 1000000,
    minWidth: "60px",
    textAlign: "center",
    letterSpacing: "2px",
    boxShadow: "0 4px 12px rgba(0,0,0,0.3)",
  }

  return (
    <div style={overlayStyle} className="click-overlay-container">
      {/* Show current input at top of screen */}
      {styleSettings.show_search_bar && inputBuffer && (
        <div style={inputIndicatorStyle} className="input-indicator">
          {inputBuffer}
        </div>
      )}

      {/* Render hint labels for each element */}
      {elements.map((element, index) => (
        <HintLabel
          key={element.id}
          element={element}
          inputBuffer={inputBuffer}
          styleSettings={styleSettings}
          animationDelay={index * 5}
          windowOffset={windowOffset}
        />
      ))}
    </div>
  )
}
