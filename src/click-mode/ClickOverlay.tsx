import { useEffect, useState, CSSProperties } from "react"
import { listen } from "@tauri-apps/api/event"
import { invoke } from "@tauri-apps/api/core"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { HintLabel } from "./HintLabel"
import { ClickableElement, ClickModeState, ClickModeStyleSettings } from "./types"
import "./click-overlay.css"

/** Main overlay component that displays hint labels */
export function ClickOverlay() {
  const [elements, setElements] = useState<ClickableElement[]>([])
  const [inputBuffer, setInputBuffer] = useState("")
  const [isActive, setIsActive] = useState(false)
  const [styleSettings, setStyleSettings] = useState<ClickModeStyleSettings>({
    hint_opacity: 1,
    hint_font_size: 11,
    hint_bg_color: "#FFCC00",
    hint_text_color: "#000000",
    show_search_bar: true,
  })

  // Fetch settings on mount
  useEffect(() => {
    invoke<{ click_mode: ClickModeStyleSettings }>("get_settings")
      .then((settings) => {
        if (settings.click_mode) {
          setStyleSettings({
            hint_opacity: settings.click_mode.hint_opacity,
            hint_font_size: settings.click_mode.hint_font_size,
            hint_bg_color: settings.click_mode.hint_bg_color,
            hint_text_color: settings.click_mode.hint_text_color,
            show_search_bar: settings.click_mode.show_search_bar,
          })
        }
      })
      .catch((e) => console.error("Failed to get settings:", e))
  }, [])

  useEffect(() => {
    const currentWindow = getCurrentWindow()

    // Listen for activation event
    const unlistenActivate = listen<ClickableElement[]>(
      "click-mode-activated",
      async (event) => {
        console.log("Click mode activated with", event.payload.length, "elements")
        setElements(event.payload)
        setInputBuffer("")
        setIsActive(true)

        // Refresh settings on activation
        try {
          const settings = await invoke<{ click_mode: ClickModeStyleSettings }>("get_settings")
          if (settings.click_mode) {
            setStyleSettings({
              hint_opacity: settings.click_mode.hint_opacity,
              hint_font_size: settings.click_mode.hint_font_size,
              hint_bg_color: settings.click_mode.hint_bg_color,
              hint_text_color: settings.click_mode.hint_text_color,
              show_search_bar: settings.click_mode.show_search_bar,
            })
          }
        } catch (e) {
          console.error("Failed to refresh settings:", e)
        }

        // Show and position the overlay window
        await currentWindow.show()
        await currentWindow.setFocus()
      }
    )

    // Listen for deactivation event
    const unlistenDeactivate = listen("click-mode-deactivated", async () => {
      console.log("Click mode deactivated")
      setIsActive(false)
      setElements([])
      setInputBuffer("")
      await currentWindow.hide()
    })

    // Listen for filtered elements update
    const unlistenFiltered = listen<ClickableElement[]>(
      "click-mode-filtered",
      (event) => {
        setElements(event.payload)
      }
    )

    // Listen for state changes from backend
    const unlistenState = listen<ClickModeState>("click-mode-state", (event) => {
      const state = event.payload
      if (state.type === "ShowingHints") {
        setInputBuffer(state.input_buffer)
      } else if (state.type === "Searching") {
        setInputBuffer(state.query)
      }
    })

    // Cleanup listeners on unmount
    return () => {
      unlistenActivate.then((fn) => fn())
      unlistenDeactivate.then((fn) => fn())
      unlistenFiltered.then((fn) => fn())
      unlistenState.then((fn) => fn())
    }
  }, [])

  // Handle keyboard input in the overlay
  useEffect(() => {
    if (!isActive) return

    const handleKeyDown = async (e: KeyboardEvent) => {
      e.preventDefault()
      e.stopPropagation()

      if (e.key === "Escape") {
        await invoke("deactivate_click_mode")
        return
      }

      if (e.key === "Backspace") {
        setInputBuffer((prev) => prev.slice(0, -1))
        // TODO: Send backspace to backend
        return
      }

      // Handle alphanumeric input
      if (e.key.length === 1 && /[a-zA-Z0-9]/.test(e.key)) {
        const newInput = inputBuffer + e.key.toUpperCase()
        setInputBuffer(newInput)

        // Check for exact match
        const matchedElement = elements.find(
          (el) => el.hint.toUpperCase() === newInput
        )

        if (matchedElement) {
          // Perform click (or right-click if Shift held)
          if (e.shiftKey) {
            await invoke("click_mode_right_click_element", {
              elementId: matchedElement.id,
            })
          } else {
            await invoke("click_mode_click_element", {
              elementId: matchedElement.id,
            })
          }
        } else {
          // Filter elements
          const filtered = elements.filter((el) =>
            el.hint.toUpperCase().startsWith(newInput)
          )

          if (filtered.length === 0) {
            // No matches - could beep or flash
            console.log("No matching hints")
          } else if (filtered.length === 1) {
            // Single match - auto-click (or right-click if Shift held)
            if (e.shiftKey) {
              await invoke("click_mode_right_click_element", {
                elementId: filtered[0].id,
              })
            } else {
              await invoke("click_mode_click_element", {
                elementId: filtered[0].id,
              })
            }
          }
        }
      }
    }

    window.addEventListener("keydown", handleKeyDown)
    return () => window.removeEventListener("keydown", handleKeyDown)
  }, [isActive, elements, inputBuffer])

  if (!isActive) {
    return null
  }

  const overlayStyle: CSSProperties = {
    position: "fixed",
    top: 0,
    left: 0,
    width: "100vw",
    height: "100vh",
    backgroundColor: "rgba(0, 0, 0, 0.05)",
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
        />
      ))}
    </div>
  )
}
