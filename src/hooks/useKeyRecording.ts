import { useState, useEffect, useCallback } from "react"
import {
  type VimKeyModifiers,
  formatKeyWithModifiers,
  recordKey,
  cancelRecordKey,
  getKeyDisplayName,
} from "../components/keyRecording"

interface UseKeyRecordingOptions {
  key: string
  modifiers: VimKeyModifiers
  onKeyRecorded: (key: string, modifiers: VimKeyModifiers) => void
}

interface UseKeyRecordingResult {
  isRecording: boolean
  displayName: string | null
  handleRecordKey: () => Promise<void>
  handleCancelRecord: () => void
}

export function useKeyRecording({
  key,
  modifiers,
  onKeyRecorded,
}: UseKeyRecordingOptions): UseKeyRecordingResult {
  const [isRecording, setIsRecording] = useState(false)
  const [displayName, setDisplayName] = useState<string | null>(null)

  useEffect(() => {
    getKeyDisplayName(key)
      .then((name) => {
        if (name) {
          setDisplayName(formatKeyWithModifiers(name, modifiers))
        } else {
          setDisplayName(null)
        }
      })
      .catch(() => setDisplayName(null))
  }, [key, modifiers])

  // Cancel recording when window loses focus
  useEffect(() => {
    if (!isRecording) return

    const handleBlur = () => {
      cancelRecordKey().catch(() => {})
      setIsRecording(false)
    }

    window.addEventListener("blur", handleBlur)
    return () => window.removeEventListener("blur", handleBlur)
  }, [isRecording])

  const handleRecordKey = useCallback(async () => {
    setIsRecording(true)
    try {
      const recorded = await recordKey()
      onKeyRecorded(recorded.name, recorded.modifiers)
      const formatted = formatKeyWithModifiers(recorded.display_name, recorded.modifiers)
      setDisplayName(formatted)
    } catch (e) {
      console.error("Failed to record key:", e)
    } finally {
      setIsRecording(false)
    }
  }, [onKeyRecorded])

  const handleCancelRecord = useCallback(() => {
    cancelRecordKey().catch(() => {})
    setIsRecording(false)
  }, [])

  return {
    isRecording,
    displayName,
    handleRecordKey,
    handleCancelRecord,
  }
}
