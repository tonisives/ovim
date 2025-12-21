import { useEffect, useState } from "react"
import { listen } from "@tauri-apps/api/event"
import { invoke } from "@tauri-apps/api/core"

interface PollingOptions<T> {
  /** Command to invoke to fetch data */
  command: string
  /** Polling interval in milliseconds */
  interval: number
  /** Initial value */
  initialValue: T
  /** Optional event name to listen for real-time updates */
  eventName?: string
}

/**
 * Hook for polling data from Tauri backend with optional event-based updates
 */
export function usePollingData<T>({
  command,
  interval,
  initialValue,
  eventName,
}: PollingOptions<T>): T {
  const [data, setData] = useState<T>(initialValue)

  useEffect(() => {
    const fetchData = async () => {
      try {
        const result = await invoke<T>(command)
        setData(result)
      } catch {
        // Keep previous value on error
      }
    }

    // Initial fetch
    fetchData()

    // Set up polling
    const intervalId = setInterval(fetchData, interval)

    // Set up event listener if event name provided
    let cleanupEvent: (() => void) | undefined
    if (eventName) {
      const unlisten = listen<T>(eventName, (event) => {
        setData(event.payload)
      })

      cleanupEvent = () => {
        unlisten.then((fn) => fn())
      }
    }

    return () => {
      clearInterval(intervalId)
      cleanupEvent?.()
    }
  }, [command, interval, eventName])

  return data
}

/**
 * Hook for data that only needs local state updates (no backend fetching)
 */
export function useIntervalValue<T>(
  getValue: () => T,
  interval: number,
): T {
  const [value, setValue] = useState(getValue)

  useEffect(() => {
    const intervalId = setInterval(() => {
      setValue(getValue())
    }, interval)

    return () => clearInterval(intervalId)
  }, [getValue, interval])

  return value
}
