import { usePollingData } from "../usePollingData"

export function KeystrokeBufferWidget({ fontFamily }: { fontFamily: string }) {
  const pendingKeys = usePollingData<string>({
    command: "get_pending_keys",
    interval: 100,
    initialValue: "",
    eventName: "pending-keys-changed",
  })

  if (!pendingKeys) {
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
      {pendingKeys}
    </div>
  )
}
