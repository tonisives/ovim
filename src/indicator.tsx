import { useEffect, useState } from "react"
import ReactDOM from "react-dom/client"
import { listen } from "@tauri-apps/api/event"
import { invoke } from "@tauri-apps/api/core"
import {
  getCurrentWindow,
  LogicalSize,
  LogicalPosition,
  availableMonitors,
} from "@tauri-apps/api/window"

type VimMode = "insert" | "normal" | "visual"
type WidgetType = "None" | "Time" | "Date" | "CharacterCount" | "LineCount" | "CharacterAndLineCount" | "Battery" | "CapsLock" | "KeystrokeBuffer"

interface RgbColor {
  r: number
  g: number
  b: number
}

interface ModeColors {
  insert: RgbColor
  normal: RgbColor
  visual: RgbColor
}

interface Settings {
  indicator_position: number
  indicator_opacity: number
  indicator_size: number
  indicator_offset_x: number
  indicator_offset_y: number
  mode_colors: ModeColors
  indicator_font: string
  top_widget: WidgetType
  bottom_widget: WidgetType
}

const BASE_SIZE = 40

function formatTime(): string {
  const now = new Date()
  const hours = now.getHours().toString().padStart(2, "0")
  const minutes = now.getMinutes().toString().padStart(2, "0")
  return `${hours}:${minutes}`
}

function formatDate(): string {
  const now = new Date()
  const days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
  const day = days[now.getDay()]
  const date = now.getDate()
  return `${day} ${date}`
}

interface SelectionInfo {
  char_count: number
  line_count: number
}

interface BatteryInfo {
  percentage: number
  is_charging: boolean
}

function Widget({ type, fontFamily }: { type: WidgetType; fontFamily: string }) {
  const [time, setTime] = useState(formatTime)
  const [date, setDate] = useState(formatDate)
  const [selection, setSelection] = useState<SelectionInfo | null>(null)
  const [battery, setBattery] = useState<BatteryInfo | null>(null)
  const [capsLock, setCapsLock] = useState(false)
  const [pendingKeys, setPendingKeys] = useState("")

  // Time effect
  useEffect(() => {
    if (type !== "Time") return
    const interval = setInterval(() => setTime(formatTime()), 1000)
    return () => clearInterval(interval)
  }, [type])

  // Date effect
  useEffect(() => {
    if (type !== "Date") return
    const interval = setInterval(() => setDate(formatDate()), 60000)
    return () => clearInterval(interval)
  }, [type])

  // Selection effect
  useEffect(() => {
    if (!["CharacterCount", "LineCount", "CharacterAndLineCount"].includes(type)) return

    const fetchSelection = async () => {
      try {
        const info = await invoke<SelectionInfo>("get_selection_info")
        setSelection(info)
      } catch {
        setSelection(null)
      }
    }

    fetchSelection()
    const interval = setInterval(fetchSelection, 500)
    return () => clearInterval(interval)
  }, [type])

  // Battery effect
  useEffect(() => {
    if (type !== "Battery") return

    const fetchBattery = async () => {
      try {
        const info = await invoke<BatteryInfo | null>("get_battery_info")
        setBattery(info)
      } catch {
        setBattery(null)
      }
    }

    fetchBattery()
    const interval = setInterval(fetchBattery, 60000)
    return () => clearInterval(interval)
  }, [type])

  // Caps Lock effect
  useEffect(() => {
    if (type !== "CapsLock") return

    const fetchCapsLock = () => {
      invoke<boolean>("get_caps_lock_state").then(setCapsLock).catch(() => {})
    }

    fetchCapsLock()
    const interval = setInterval(fetchCapsLock, 200)

    const unlisten = listen<boolean>("caps-lock-changed", (event) => {
      setCapsLock(event.payload)
    })

    return () => {
      clearInterval(interval)
      unlisten.then((fn) => fn())
    }
  }, [type])

  // Keystroke buffer effect
  useEffect(() => {
    if (type !== "KeystrokeBuffer") return

    const fetchPendingKeys = () => {
      invoke<string>("get_pending_keys").then(setPendingKeys).catch(() => {})
    }

    fetchPendingKeys()
    const interval = setInterval(fetchPendingKeys, 100)

    const unlisten = listen<string>("pending-keys-changed", (event) => {
      setPendingKeys(event.payload)
    })

    return () => {
      clearInterval(interval)
      unlisten.then((fn) => fn())
    }
  }, [type])

  if (type === "None") return null

  let content: string
  switch (type) {
    case "Time":
      content = time
      break
    case "Date":
      content = date
      break
    case "CharacterCount":
      content = selection ? `${selection.char_count}c` : "-"
      break
    case "LineCount":
      content = selection ? `${selection.line_count}L` : "-"
      break
    case "CharacterAndLineCount":
      content = selection ? `${selection.char_count}c ${selection.line_count}L` : "-"
      break
    case "Battery":
      content = battery ? `${battery.percentage}%` : "-"
      break
    case "CapsLock":
      content = capsLock ? "CAPS" : ""
      break
    case "KeystrokeBuffer":
      content = pendingKeys || ""
      break
    default:
      return null
  }

  // Don't render empty content for CapsLock and KeystrokeBuffer
  if ((type === "CapsLock" || type === "KeystrokeBuffer") && !content) {
    return null
  }

  return (
    <div
      style={{
        fontSize: "9px",
        opacity: 0.9,
        fontFamily: fontFamily,
        whiteSpace: "nowrap",
        paddingTop: 2,
      }}
    >
      {content}
    </div>
  )
}

async function applyWindowSettings(settings: Settings) {
  const window = getCurrentWindow()
  const baseSize = Math.round(BASE_SIZE * settings.indicator_size)

  // Calculate height based on active widgets
  const widgetHeight = 12 // Height for each widget row (10px font + 4px margin)
  const hasTopWidget = settings.top_widget !== "None"
  const hasBottomWidget = settings.bottom_widget !== "None"
  const widgetCount = (hasTopWidget ? 1 : 0) + (hasBottomWidget ? 1 : 0)

  const width = baseSize - 4
  const height = baseSize + widgetCount * widgetHeight - 2

  // Get primary monitor dimensions
  const monitors = await availableMonitors()
  const monitor = monitors[0]

  if (!monitor) {
    console.error("No monitor found!")
    return
  }

  const screenWidth = monitor.size.width / monitor.scaleFactor
  const screenHeight = monitor.size.height / monitor.scaleFactor
  const padding = 20

  // Calculate position based on indicator_position (0-5 for 2x3 grid)
  // 0: Top Left, 1: Top Middle, 2: Top Right
  // 3: Bottom Left, 4: Bottom Middle, 5: Bottom Right
  let x: number
  let y: number

  const col = settings.indicator_position % 3
  const row = Math.floor(settings.indicator_position / 3)

  switch (col) {
    case 0: // Left
      x = padding
      break
    case 1: // Middle
      x = (screenWidth - width) / 2
      break
    case 2: // Right
      x = screenWidth - width - padding
      break
    default:
      x = padding
  }

  switch (row) {
    case 0: // Top
      y = padding + 30 // Account for menu bar
      break
    case 1: // Bottom
      y = screenHeight - height - padding
      break
    default:
      y = padding + 30
  }

  // Apply offsets
  x += settings.indicator_offset_x ?? 0
  y += settings.indicator_offset_y ?? 0

  try {
    await window.setSize(new LogicalSize(width, height))
    await window.setPosition(new LogicalPosition(Math.round(x), Math.round(y)))
    console.log("Window settings applied successfully")
  } catch (err) {
    console.error("Failed to apply window settings:", err)
  }
}

function Indicator() {
  const [mode, setMode] = useState<VimMode>("insert")
  const [settings, setSettings] = useState<Settings | null>(null)

  useEffect(() => {
    invoke<Settings>("get_settings")
      .then((s) => {
        setSettings(s)
        applyWindowSettings(s)
      })
      .catch((e) => console.error("Failed to get settings:", e))

    const unlistenSettings = listen<Settings>("settings-changed", (event) => {
      setSettings(event.payload)
      applyWindowSettings(event.payload)
    })

    return () => {
      unlistenSettings.then((fn) => fn())
    }
  }, [])

  useEffect(() => {
    invoke<string>("get_vim_mode")
      .then((m) => setMode(m as VimMode))
      .catch((e) => console.error("Failed to get initial mode:", e))

    // Listen for mode changes
    const unlisten = listen<string>("mode-change", (event) => {
      setMode(event.payload as VimMode)
    })

    return () => {
      unlisten.then((fn) => fn())
    }
  }, [])

  const modeChar = mode === "insert" ? "i" : mode === "normal" ? "n" : "v"
  const opacity = settings?.indicator_opacity ?? 0.9

  // Default colors if settings not loaded
  const defaultColors: ModeColors = {
    insert: { r: 74, g: 144, b: 217 },
    normal: { r: 232, g: 148, b: 74 },
    visual: { r: 155, g: 109, b: 215 },
  }

  const colors = settings?.mode_colors ?? defaultColors
  const color = mode === "insert" ? colors.insert : mode === "normal" ? colors.normal : colors.visual
  const bgColor = `rgba(${color.r}, ${color.g}, ${color.b}, ${opacity})`

  const fontFamily = settings?.indicator_font ?? "system-ui, -apple-system, sans-serif"
  const topWidget = settings?.top_widget ?? "None"
  const bottomWidget = settings?.bottom_widget ?? "None"

  // Build grid template based on which widgets are active
  const hasTop = topWidget !== "None"
  const hasBottom = bottomWidget !== "None"
  let gridTemplateRows = "1fr"
  if (hasTop && hasBottom) {
    gridTemplateRows = "auto 1fr auto"
  } else if (hasTop) {
    gridTemplateRows = "auto 1fr"
  } else if (hasBottom) {
    gridTemplateRows = "1fr auto"
  }

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        display: "grid",
        gridTemplateRows,
        alignItems: "center",
        justifyItems: "center",
        background: bgColor,
        borderRadius: "8px",
        fontFamily: fontFamily,
        color: "white",
        boxSizing: "border-box",
        overflow: "hidden",
        paddingBottom: "1px",
      }}
    >
      {hasTop && <Widget type={topWidget} fontFamily={fontFamily} />}
      <div
        style={{
          display: "grid",
          placeItems: "center",
          width: "100%",
          height: "100%",
        }}
      >
        <span
          style={{
            fontSize: "36px",
            fontWeight: "bold",
            textTransform: "uppercase",
            lineHeight: "0.75em",
            display: "block",
            transform: "translateY(1px)",
          }}
        >
          {modeChar}
        </span>
      </div>
      {hasBottom && <Widget type={bottomWidget} fontFamily={fontFamily} />}
    </div>
  )
}

ReactDOM.createRoot(document.getElementById("root")!).render(<Indicator />)
