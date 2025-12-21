import type { WidgetType } from "../types"
import { TimeWidget } from "./TimeWidget"
import { DateWidget } from "./DateWidget"
import { SelectionWidget } from "./SelectionWidget"
import { BatteryWidget } from "./BatteryWidget"
import { CapsLockWidget } from "./CapsLockWidget"
import { KeystrokeBufferWidget } from "./KeystrokeBufferWidget"

interface WidgetProps {
  type: WidgetType
  fontFamily: string
}

export function Widget({ type, fontFamily }: WidgetProps) {
  switch (type) {
    case "None":
      return null
    case "Time":
      return <TimeWidget fontFamily={fontFamily} />
    case "Date":
      return <DateWidget fontFamily={fontFamily} />
    case "CharacterCount":
      return <SelectionWidget fontFamily={fontFamily} showChars showLines={false} />
    case "LineCount":
      return <SelectionWidget fontFamily={fontFamily} showChars={false} showLines />
    case "CharacterAndLineCount":
      return <SelectionWidget fontFamily={fontFamily} showChars showLines />
    case "Battery":
      return <BatteryWidget fontFamily={fontFamily} />
    case "CapsLock":
      return <CapsLockWidget fontFamily={fontFamily} />
    case "KeystrokeBuffer":
      return <KeystrokeBufferWidget fontFamily={fontFamily} />
    default:
      return null
  }
}
