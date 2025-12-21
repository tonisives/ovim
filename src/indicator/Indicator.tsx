import { useEffect, useState } from "react"
import { listen } from "@tauri-apps/api/event"
import { invoke } from "@tauri-apps/api/core"
import { Widget } from "./widgets"
import { applyWindowSettings } from "./windowPosition"
import type { VimMode, Settings, ModeColors } from "./types"

const defaultColors: ModeColors = {
  insert: { r: 74, g: 144, b: 217 },
  normal: { r: 232, g: 148, b: 74 },
  visual: { r: 155, g: 109, b: 215 },
}

export function Indicator() {
  const [mode, setMode] = useState<VimMode>("insert")
  const [settings, setSettings] = useState<Settings | null>(null)

  useEffect(() => {
    invoke<Settings>("get_settings")
      .then(async (s) => {
        setSettings(s)
        await applyWindowSettings(s)
      })
      .catch((e) => console.error("Failed to get settings:", e))

    const unlistenSettings = listen<Settings>("settings-changed", async (event) => {
      setSettings(event.payload)
      await applyWindowSettings(event.payload)
    })

    return () => {
      unlistenSettings.then((fn) => fn())
    }
  }, [])

  useEffect(() => {
    invoke<string>("get_vim_mode")
      .then((m) => setMode(m as VimMode))
      .catch((e) => console.error("Failed to get initial mode:", e))

    const unlisten = listen<string>("mode-change", (event) => {
      setMode(event.payload as VimMode)
    })

    return () => {
      unlisten.then((fn) => fn())
    }
  }, [])

  const modeChar = mode === "insert" ? "i" : mode === "normal" ? "n" : "v"
  const opacity = settings?.indicator_opacity ?? 0.9
  const colors = settings?.mode_colors ?? defaultColors
  const color = colors[mode]
  const bgColor = `rgb(${color.r}, ${color.g}, ${color.b})`

  const fontFamily = settings?.indicator_font ?? "system-ui, -apple-system, sans-serif"
  const topWidget = settings?.top_widget ?? "None"
  const bottomWidget = settings?.bottom_widget ?? "None"

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
        fontFamily,
        color: "white",
        boxSizing: "border-box",
        overflow: "hidden",
        paddingBottom: "1px",
        opacity,
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
