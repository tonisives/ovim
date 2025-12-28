/** Represents a clickable UI element */
export interface ClickableElement {
  /** Unique identifier for this element */
  id: number
  /** The hint label (e.g., "A", "SD", "FG") */
  hint: string
  /** Element position x in screen coordinates */
  x: number
  /** Element position y in screen coordinates */
  y: number
  /** Element width */
  width: number
  /** Element height */
  height: number
  /** Element role (button, link, menuitem, etc.) */
  role: string
  /** Element title/label text */
  title: string
}

/** Click mode state from backend */
export type ClickModeState =
  | { type: "Inactive" }
  | { type: "ShowingHints"; input_buffer: string; element_count: number }
  | { type: "Searching"; query: string; match_count: number }

/** Style settings for click mode from backend */
export interface ClickModeStyleSettings {
  hint_opacity: number
  hint_font_size: number
  hint_bg_color: string
  hint_text_color: string
  show_search_bar: boolean
}
