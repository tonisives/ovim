import { usePollingData } from "../usePollingData"
import type { BatteryInfo } from "../types"

export function BatteryWidget({ fontFamily }: { fontFamily: string }) {
  const battery = usePollingData<BatteryInfo | null>({
    command: "get_battery_info",
    interval: 60000,
    initialValue: null,
  })

  const content = battery ? `${battery.percentage}%` : "-"

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
      {content}
    </div>
  )
}
