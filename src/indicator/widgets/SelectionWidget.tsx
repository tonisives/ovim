import { usePollingData } from "../usePollingData"
import type { SelectionInfo } from "../types"

interface SelectionWidgetProps {
  fontFamily: string
  showChars: boolean
  showLines: boolean
}

export function SelectionWidget({
  fontFamily,
  showChars,
  showLines,
}: SelectionWidgetProps) {
  const selection = usePollingData<SelectionInfo | null>({
    command: "get_selection_info",
    interval: 500,
    initialValue: null,
  })

  let content: string
  if (!selection) {
    content = "-"
  } else if (showChars && showLines) {
    content = `${selection.char_count}c ${selection.line_count}L`
  } else if (showChars) {
    content = `${selection.char_count}c`
  } else {
    content = `${selection.line_count}L`
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
      {content}
    </div>
  )
}
