export interface PathValidation {
  terminal_valid: boolean
  terminal_resolved_path: string
  terminal_error: string | null
  editor_valid: boolean
  editor_resolved_path: string
  editor_error: string | null
}

export const TERMINAL_OPTIONS = [
  { value: "alacritty", label: "Alacritty" },
  { value: "kitty", label: "Kitty" },
  { value: "wezterm", label: "WezTerm" },
  { value: "iterm", label: "iTerm2" },
  { value: "ghostty", label: "Ghostty" },
  { value: "default", label: "Terminal.app" },
]

export const DEFAULT_TERMINAL_PATHS: Record<string, string> = {
  alacritty: "/Applications/Alacritty.app/Contents/MacOS/alacritty",
  kitty: "/Applications/kitty.app/Contents/MacOS/kitty",
  wezterm: "/Applications/WezTerm.app/Contents/MacOS/wezterm",
  ghostty: "/Applications/Ghostty.app/Contents/MacOS/ghostty",
  iterm: "",
  default: "",
}

export const EDITOR_OPTIONS = [
  { value: "neovim", label: "Neovim" },
  { value: "vim", label: "Vim" },
  { value: "helix", label: "Helix" },
  { value: "custom", label: "Custom" },
]

export const DEFAULT_EDITOR_PATHS: Record<string, string> = {
  neovim: "nvim",
  vim: "vim",
  helix: "hx",
  custom: "",
}
