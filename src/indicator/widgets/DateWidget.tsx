import { useIntervalValue } from "../usePollingData"

function formatDate(): string {
  const now = new Date()
  const days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
  const day = days[now.getDay()]
  const date = now.getDate()
  return `${day} ${date}`
}

export function DateWidget({ fontFamily }: { fontFamily: string }) {
  const date = useIntervalValue(formatDate, 60000)

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
      {date}
    </div>
  )
}
