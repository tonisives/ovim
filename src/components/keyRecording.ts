import { invoke } from "@tauri-apps/api/core"

export interface VimKeyModifiers {
  shift: boolean
  control: boolean
  option: boolean
  command: boolean
}

export interface RecordedKey {
  name: string
  display_name: string
  modifiers: VimKeyModifiers
}

export function formatKeyWithModifiers(
  displayName: string,
  modifiers: VimKeyModifiers,
): string {
  const parts: string[] = []
  if (modifiers.control) parts.push("Ctrl")
  if (modifiers.option) parts.push("Opt")
  if (modifiers.shift) parts.push("Shift")
  if (modifiers.command) parts.push("Cmd")
  parts.push(displayName)
  return parts.join(" + ")
}

export function hasAnyModifier(modifiers: VimKeyModifiers): boolean {
  return modifiers.shift || modifiers.control || modifiers.option || modifiers.command
}

export async function recordKey(): Promise<RecordedKey> {
  return invoke<RecordedKey>("record_key")
}

export async function cancelRecordKey(): Promise<void> {
  await invoke("cancel_record_key")
}

export async function getKeyDisplayName(keyName: string): Promise<string | null> {
  return invoke<string | null>("get_key_display_name", { keyName })
}
