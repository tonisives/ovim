import {
  getCurrentWindow,
  LogicalSize,
  LogicalPosition,
  availableMonitors,
} from "@tauri-apps/api/window"
import type { Settings, RowItem } from "./types"

const BASE_SIZE = 40
const WIDGET_ROW_HEIGHT = 12
const MODE_CHAR_ROW_HEIGHT = 22

function computeIndicatorHeight(rows: RowItem[], scale: number): number {
  let totalHeight = 0
  for (const row of rows) {
    if (row.type === "ModeChar") {
      totalHeight += row.size * MODE_CHAR_ROW_HEIGHT
    } else {
      totalHeight += WIDGET_ROW_HEIGHT
    }
  }
  return Math.round(totalHeight * scale) - 2
}

export async function applyWindowSettings(settings: Settings): Promise<void> {
  const window = getCurrentWindow()

  if (!settings.enabled || !settings.indicator_visible) {
    await window.hide()
    return
  }
  // Only show if currently hidden to avoid stealing focus from other windows
  if (!(await window.isVisible())) {
    await window.show()
  }

  const scale = settings.indicator_size
  const rows: RowItem[] = settings.indicator_rows ?? [{ type: "ModeChar", size: 2 }]

  const width = Math.round(BASE_SIZE * scale) - 4
  const height = computeIndicatorHeight(rows, scale)

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
