import { useIntervalValue } from "../usePollingData"

function formatTime(): string {
  const now = new Date()
  const hours = now.getHours().toString().padStart(2, "0")
  const minutes = now.getMinutes().toString().padStart(2, "0")
  return `${hours}:${minutes}`
}

export function TimeWidget({ fontFamily }: { fontFamily: string }) {
  const time = useIntervalValue(formatTime, 1000)

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
      {time}
    </div>
  )
}
