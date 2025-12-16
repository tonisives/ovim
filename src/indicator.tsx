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
type WidgetType = "None" | "Time" | "CharacterCount" | "LineCount" | "CharacterAndLineCount"

interface Settings {
  indicator_position: number
  indicator_opacity: number
  indicator_size: number
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

function Widget({ type }: { type: WidgetType }) {
  const [time, setTime] = useState(formatTime)

  useEffect(() => {
    if (type !== "Time") return

    const interval = setInterval(() => {
      setTime(formatTime())
    }, 1000)

    return () => clearInterval(interval)
  }, [type])

  if (type === "None") return null

  let content: string
  switch (type) {
    case "Time":
      content = time
      break
    case "CharacterCount":
      content = "-" // TODO: implement selection tracking
      break
    case "LineCount":
      content = "-" // TODO: implement selection tracking
      break
    case "CharacterAndLineCount":
      content = "-" // TODO: implement selection tracking
      break
    default:
      return null
  }

  return (
    <div
      style={{
        fontSize: "9px",
        opacity: 0.9,
        fontFamily: "monospace",
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

  const bgColor =
    mode === "insert"
      ? `rgba(76, 175, 80, ${opacity})`
      : mode === "normal"
        ? `rgba(33, 150, 243, ${opacity})`
        : `rgba(255, 152, 0, ${opacity})`

  const topWidget = settings?.top_widget ?? "None"
  const bottomWidget = settings?.bottom_widget ?? "None"

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        background: bgColor,
        borderRadius: "8px",
        fontFamily: "system-ui, -apple-system, sans-serif",
        color: "white",
        boxSizing: "border-box",
      }}
    >
      {topWidget !== "None" && <Widget type={topWidget} />}
      <div
        style={{
          fontSize: "36px",
          fontWeight: "bold",
          textTransform: "uppercase",
          lineHeight: 1,
          display: "flex",
          alignItems: "center",
        }}
      >
        {modeChar}
      </div>
      {bottomWidget !== "None" && <Widget type={bottomWidget} />}
    </div>
  )
}

ReactDOM.createRoot(document.getElementById("root")!).render(<Indicator />)
