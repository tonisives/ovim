import { useState, useEffect, useCallback } from "react"
import { invoke } from "@tauri-apps/api/core"
import type { Settings, NvimEditSettings as NvimEditSettingsType } from "./SettingsApp"
import { useKeyRecording } from "../hooks/useKeyRecording"
import { type PathValidation } from "./nvimEdit/constants"
import { ConfigTab } from "./nvimEdit/ConfigTab"
import { WindowTab } from "./nvimEdit/WindowTab"

interface Props {
  settings: Settings
  onUpdate: (updates: Partial<Settings>) => void
  activeTab: "nvim-config" | "nvim-window"
}

export function NvimEditSettings({ settings, onUpdate, activeTab }: Props) {
  const [validation, setValidation] = useState<PathValidation | null>(null)
  const [isValidating, setIsValidating] = useState(false)
  const [showErrorDialog, setShowErrorDialog] = useState<"terminal" | "editor" | null>(null)

  const nvimEdit = settings.nvim_edit

  const updateNvimEdit = useCallback(
    (updates: Partial<NvimEditSettingsType>) => {
      onUpdate({
        nvim_edit: { ...nvimEdit, ...updates },
      })
    },
    [nvimEdit, onUpdate],
  )

  const { isRecording, displayName, handleRecordKey, handleCancelRecord } = useKeyRecording({
    key: nvimEdit.shortcut_key,
    modifiers: nvimEdit.shortcut_modifiers,
    onKeyRecorded: (key, modifiers) => {
      updateNvimEdit({
        shortcut_key: key,
        shortcut_modifiers: modifiers,
      })
    },
  })

  // Validate paths when settings change
  const validatePaths = useCallback(async () => {
    if (!nvimEdit.enabled) {
      setValidation(null)
      return
    }

    setIsValidating(true)
    try {
      const result = await invoke<PathValidation>("validate_nvim_edit_paths", {
        terminalType: nvimEdit.terminal,
        terminalPath: nvimEdit.terminal_path,
        editorType: nvimEdit.editor,
        editorPath: nvimEdit.nvim_path,
      })
      setValidation(result)
    } catch (e) {
      console.error("Failed to validate paths:", e)
      setValidation(null)
    } finally {
      setIsValidating(false)
    }
  }, [
    nvimEdit.enabled,
    nvimEdit.terminal,
    nvimEdit.terminal_path,
    nvimEdit.editor,
    nvimEdit.nvim_path,
  ])

  useEffect(() => {
    validatePaths()
  }, [validatePaths])

  const errorCount = validation
    ? (validation.terminal_valid ? 0 : 1) + (validation.editor_valid ? 0 : 1)
    : 0

  return (
    <div className="settings-section">
      <div className="section-header">
        <h2>Edit Popup</h2>
        {nvimEdit.enabled && errorCount > 0 && (
          <span className="error-badge" title="Configuration errors detected">
            {errorCount} {errorCount === 1 ? "error" : "errors"}
          </span>
        )}
        {isValidating && <span className="validating-badge">Checking...</span>}
      </div>
      <p className="section-description">
        Press a shortcut while focused on any text field to edit its contents in your preferred
        terminal editor.
      </p>

      {showErrorDialog && (
        <div className="error-dialog-overlay" onClick={() => setShowErrorDialog(null)}>
          <div className="error-dialog" onClick={(e) => e.stopPropagation()}>
            <h3>{showErrorDialog === "terminal" ? "Terminal Not Found" : "Editor Not Found"}</h3>
            <p>
              {showErrorDialog === "terminal"
                ? validation?.terminal_error
                : validation?.editor_error}
            </p>
            <div className="error-dialog-buttons">
              <button onClick={() => setShowErrorDialog(null)}>Close</button>
            </div>
          </div>
        </div>
      )}

      {activeTab === "nvim-config" && (
        <ConfigTab
          nvimEdit={nvimEdit}
          validation={validation}
          isRecording={isRecording}
          displayName={displayName}
          onUpdate={updateNvimEdit}
          onRecordKey={handleRecordKey}
          onCancelRecord={handleCancelRecord}
          onShowErrorDialog={setShowErrorDialog}
        />
      )}

      {activeTab === "nvim-window" && <WindowTab nvimEdit={nvimEdit} onUpdate={updateNvimEdit} />}
    </div>
  )
}
