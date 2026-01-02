import { useEffect, useState, useCallback } from "react"
import { listen } from "@tauri-apps/api/event"
import { invoke } from "@tauri-apps/api/core"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { ClickableElement, ClickModeStyleSettings } from "../types"

interface WindowOffset {
  x: number
  y: number
}

interface ActivatedPayload {
  elements: ClickableElement[]
  window_offset: [number, number]
}

interface UseClickModeEventsResult {
  elements: ClickableElement[]
  isActive: boolean
  windowOffset: WindowOffset
  styleSettings: ClickModeStyleSettings
  inputBuffer: string
  setInputBuffer: (buffer: string) => void
}

const defaultStyleSettings: ClickModeStyleSettings = {
  hint_opacity: 1,
  hint_font_size: 11,
  hint_bg_color: "#FFCC00",
  hint_text_color: "#000000",
  show_search_bar: true,
}

export function useClickModeEvents(): UseClickModeEventsResult {
  const [elements, setElements] = useState<ClickableElement[]>([])
  const [inputBuffer, setInputBuffer] = useState("")
  const [isActive, setIsActive] = useState(false)
  const [windowOffset, setWindowOffset] = useState<WindowOffset>({ x: 0, y: 0 })
  const [styleSettings, setStyleSettings] = useState<ClickModeStyleSettings>(defaultStyleSettings)

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

  // Refresh settings helper
  const refreshSettings = useCallback(async () => {
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
  }, [])

  useEffect(() => {
    const currentWindow = getCurrentWindow()

    // Listen for activation event
    const unlistenActivate = listen<ActivatedPayload>(
      "click-mode-activated",
      async (event) => {
        const { elements: newElements, window_offset } = event.payload
        setElements(newElements)
        setInputBuffer("")
        setIsActive(true)
        setWindowOffset({ x: window_offset[0], y: window_offset[1] })

        // Refresh settings on activation
        await refreshSettings()

        // Show and position the overlay window
        await currentWindow.show()
        await currentWindow.setFocus()
      }
    )

    // Listen for deactivation event
    const unlistenDeactivate = listen("click-mode-deactivated", async () => {
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
    const unlistenState = listen<{ type: string; input_buffer?: string; query?: string }>(
      "click-mode-state",
      (event) => {
        const state = event.payload
        if (state.type === "ShowingHints" && state.input_buffer) {
          setInputBuffer(state.input_buffer)
        } else if (state.type === "Searching" && state.query) {
          setInputBuffer(state.query)
        }
      }
    )

    // Cleanup listeners on unmount
    return () => {
      unlistenActivate.then((fn) => fn())
      unlistenDeactivate.then((fn) => fn())
      unlistenFiltered.then((fn) => fn())
      unlistenState.then((fn) => fn())
    }
  }, [refreshSettings])

  return {
    elements,
    isActive,
    windowOffset,
    styleSettings,
    inputBuffer,
    setInputBuffer,
  }
}
