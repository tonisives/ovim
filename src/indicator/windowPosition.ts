import {
  getCurrentWindow,
  LogicalSize,
  LogicalPosition,
  availableMonitors,
} from "@tauri-apps/api/window"
import type { Settings } from "./types"

const BASE_SIZE = 40

export async function applyWindowSettings(settings: Settings): Promise<void> {
  const window = getCurrentWindow()

  if (!settings.enabled || !settings.indicator_visible) {
    await window.hide()
    return
  }
  await window.show()

  const baseSize = Math.round(BASE_SIZE * settings.indicator_size)

  // Calculate height based on active widgets
  const widgetHeight = 12
  const hasTopWidget = settings.top_widget !== "None"
  const hasBottomWidget = settings.bottom_widget !== "None"
  const widgetCount = (hasTopWidget ? 1 : 0) + (hasBottomWidget ? 1 : 0)

  const width = baseSize - 4
  const height = baseSize + widgetCount * widgetHeight - 2

  const monitors = await availableMonitors()
  const monitor = monitors[0]

  if (!monitor) {
    console.error("No monitor found!")
    return
  }

  const screenWidth = monitor.size.width / monitor.scaleFactor
  const screenHeight = monitor.size.height / monitor.scaleFactor
  const padding = 20

  const { x, y } = calculatePosition(
    settings.indicator_position,
    screenWidth,
    screenHeight,
    width,
    height,
    padding,
    settings.indicator_offset_x ?? 0,
    settings.indicator_offset_y ?? 0,
  )

  try {
    await window.setSize(new LogicalSize(width, height))
    await window.setPosition(new LogicalPosition(Math.round(x), Math.round(y)))
  } catch (err) {
    console.error("Failed to apply window settings:", err)
  }
}

function calculatePosition(
  position: number,
  screenWidth: number,
  screenHeight: number,
  width: number,
  height: number,
  padding: number,
  offsetX: number,
  offsetY: number,
): { x: number; y: number } {
  const col = position % 3
  const row = Math.floor(position / 3)

  let x: number
  let y: number

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

  return { x: x + offsetX, y: y + offsetY }
}
