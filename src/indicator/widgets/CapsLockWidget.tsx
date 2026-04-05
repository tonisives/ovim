import { usePollingData } from "../usePollingData"

export function CapsLockWidget({ fontFamily }: { fontFamily: string }) {
  const capsLock = usePollingData<boolean>({
    command: "get_caps_lock_state",
    interval: 200,
    initialValue: false,
    eventName: "caps-lock-changed",
  })

  return (
    <div
      style={{
        fontSize: "9px",
        opacity: capsLock ? 0.9 : 0.3,
        fontFamily,
        whiteSpace: "nowrap",
        paddingTop: 2,
      }}
    >
      {capsLock ? "CAPS" : "caps"}
    </div>
  )
}
