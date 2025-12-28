import { useEffect, useState, useCallback } from "react"
import { listen } from "@tauri-apps/api/event"
import { invoke } from "@tauri-apps/api/core"
import { openUrl } from "@tauri-apps/plugin-opener"
import { Widget } from "./widgets"
import { applyWindowSettings } from "./windowPosition"
import type { VimMode, Settings, ModeColors } from "./types"

interface PendingUpdate {
  version: string
}

const defaultColors: ModeColors = {
  insert: { r: 74, g: 144, b: 217 },
  normal: { r: 232, g: 148, b: 74 },
  visual: { r: 155, g: 109, b: 215 },
}

export function Indicator() {
  const [mode, setMode] = useState<VimMode>("insert")
  const [settings, setSettings] = useState<Settings | null>(null)
  const [isHoverable, setIsHoverable] = useState(false)
  const [isHovered, setIsHovered] = useState(false)
  const [pendingUpdate, setPendingUpdate] = useState<PendingUpdate | null>(null)

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

  // Poll for mouse position and Cmd key state
  useEffect(() => {
    let intervalId: ReturnType<typeof setInterval> | null = null
    let lastHoverState = false
    let lastCmdState = false

    const checkState = async () => {
      try {
        // Always check if mouse is over indicator
        const mouseOver = await invoke<boolean>("is_mouse_over_indicator")

        // Update hover state
        if (mouseOver !== lastHoverState) {
          lastHoverState = mouseOver
          setIsHovered(mouseOver)
        }

        // Only check Cmd key and update window when hovering
        if (mouseOver) {
          const cmdPressed = await invoke<boolean>("is_command_key_pressed")

          if (cmdPressed !== lastCmdState) {
            lastCmdState = cmdPressed
            setIsHoverable(cmdPressed)
            await invoke("set_indicator_ignores_mouse", { ignore: !cmdPressed })
          }
        } else {
          // Reset when not hovering
          if (lastCmdState) {
            lastCmdState = false
            setIsHoverable(false)
            await invoke("set_indicator_ignores_mouse", { ignore: true })
          }
        }
      } catch (e) {
        console.error("Failed to check state:", e)
      }
    }

    // Poll every 100ms
    intervalId = setInterval(checkState, 100)

    return () => {
      if (intervalId) {
        clearInterval(intervalId)
      }
      // Reset to ignore mouse events when component unmounts
      invoke("set_indicator_ignores_mouse", { ignore: true }).catch(() => {})
    }
  }, []) // No dependencies - run once

  // Listen for update-installed events
  useEffect(() => {
    const unlisten = listen<PendingUpdate>("update-installed", (event) => {
      setPendingUpdate(event.payload)
      // Make window clickable when update is available
      invoke("set_indicator_clickable", { clickable: true }).catch((e) =>
        console.error("Failed to make indicator clickable:", e)
      )
    })

    return () => {
      unlisten.then((fn) => fn())
    }
  }, [])

  const handleRestartClick = async (e: React.MouseEvent) => {
    e.preventDefault()
    e.stopPropagation()
    // Open GitHub releases page
    const url = `https://github.com/tonisives/ovim/releases/tag/v${pendingUpdate?.version}`
    await openUrl(url).catch((err) => console.error("Failed to open releases:", err))
    // Small delay to ensure browser opens before restart
    setTimeout(() => {
      invoke("restart_app").catch((e) => console.error("Failed to restart:", e))
    }, 500)
  }

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

  const handleOpenSettings = useCallback(async () => {
    try {
      await invoke("open_settings_window")
    } catch (e) {
      console.error("Failed to open settings:", e)
    }
  }, [])

  // Show overlay when hovering
  const showOverlay = isHovered

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
        cursor: isHoverable ? "pointer" : "default",
        position: "relative",
      }}
      onClick={isHoverable ? handleOpenSettings : undefined}
    >
      {/* Update badge overlay - covers entire indicator */}
      {pendingUpdate && (
        <button
          onClick={handleRestartClick}
          title={`Update to v${pendingUpdate.version} ready - click to restart`}
          style={{
            position: "absolute",
            top: 0,
            left: 0,
            right: 0,
            bottom: 0,
            borderRadius: "8px",
            background: "#30d158",
            border: "none",
            cursor: "pointer",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontSize: "20px",
            fontWeight: "bold",
            color: "white",
            zIndex: 10,
            padding: 0,
          }}
        >
          ↑
        </button>
      )}
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

      {/* Settings overlay when hovering */}
      {showOverlay && (
        <div
          style={{
            position: "absolute",
            inset: 0,
            background: "rgba(0, 0, 0, 0.6)",
            borderRadius: "8px",
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            gap: "4px",
          }}
        >
          <svg
            width="20"
            height="20"
            viewBox="0 0 24 24"
            fill="none"
            stroke="white"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <circle cx="12" cy="12" r="3" />
            <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
          </svg>
          {!isHoverable && (
            <span style={{ fontSize: "8px", opacity: 0.8 }}>hold Cmd</span>
          )}
        </div>
      )}
    </div>
  )
}
