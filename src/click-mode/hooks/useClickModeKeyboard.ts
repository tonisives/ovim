import { useEffect } from "react"
import { invoke } from "@tauri-apps/api/core"
import { ClickableElement } from "../types"

interface UseClickModeKeyboardOptions {
  isActive: boolean
  elements: ClickableElement[]
  inputBuffer: string
  setInputBuffer: (buffer: string) => void
}

export function useClickModeKeyboard({
  isActive,
  elements,
  inputBuffer,
  setInputBuffer,
}: UseClickModeKeyboardOptions): void {
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
        setInputBuffer(inputBuffer.slice(0, -1))
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

          if (filtered.length === 1) {
            // Single match - auto-click
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
  }, [isActive, elements, inputBuffer, setInputBuffer])
}
