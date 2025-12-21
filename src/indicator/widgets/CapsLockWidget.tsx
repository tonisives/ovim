import { usePollingData } from "../usePollingData"

export function CapsLockWidget({ fontFamily }: { fontFamily: string }) {
  const capsLock = usePollingData<boolean>({
    command: "get_caps_lock_state",
    interval: 200,
    initialValue: false,
    eventName: "caps-lock-changed",
  })

  if (!capsLock) {
    return null
  }

  return (
    <div
      style={{
        fontSize: "9px",
        opacity: 0.9,
        fontFamily,
        whiteSpace: "nowrap",
        paddingTop: 2,
      }}
    >
      CAPS
    </div>
  )
}
