# ovim

macOS system-wide Vim keybindings and modal editor.

ovim is a lightweight menu bar application that brings Vim's modal editing to every app on your Mac. Press a key to toggle between Insert and Normal mode anywhere - in your browser, text editors, terminal, or any other application.

![ovim Modes](docs/images/modes-animated.gif)

## Features

- **System-wide Vim modes** - Normal, Insert, and Visual modes work in any macOS application
- **Modal popup editor** - Open a full Neovim editor popup for complex edits, then paste back
- **Mode indicator** - Floating widget shows current mode with customizable position, size, and opacity
- **Configurable activation key** - Default is Caps Lock, customizable with modifier keys
- **Per-application ignore list** - Disable ovim for apps with their own Vim mode
- **Widgets** - Display battery status, caps lock state, or selection info

## Installation

### Homebrew

```bash
brew install --cask ovim
```

### GitHub Releases

Download the latest `.dmg` from the [Releases](https://github.com/tonisives/ovim/releases) page.

### Build from Source

```bash
git clone https://github.com/tonisives/ovim.git
cd ovim
pnpm install
pnpm tauri build
# Built app in src-tauri/target/release/bundle/
```

Requires [Rust](https://rustup.rs/), [Node.js](https://nodejs.org/) v18+, and [pnpm](https://pnpm.io/).

## Requirements

- macOS 10.15 (Catalina) or later
- **Accessibility permission** - Grant in System Settings > Privacy & Security > Accessibility

## Quick Start

1. Launch ovim - it appears in your menu bar
2. Grant Accessibility permission when prompted
3. Press **Caps Lock** to toggle between modes
4. Access Settings from the menu bar icon

## Vim Commands

### Mode Switching

| Key | Action |
| --- | ------ |
| `Esc` | Return to Normal mode |
| `i` / `I` | Insert at cursor / line start |
| `a` / `A` | Append after cursor / line end |
| `o` / `O` | Open line below / above |
| `v` | Enter Visual mode |
| `s` / `S` | Substitute character / line |

### Motions

| Key | Action |
| --- | ------ |
| `h` `j` `k` `l` | Left, down, up, right |
| `w` / `b` / `e` | Word forward / backward / end |
| `0` / `$` | Line start / end |
| `{` / `}` | Paragraph up / down |
| `gg` / `G` | Document start / end |
| `Ctrl+u` / `Ctrl+d` | Half page up / down |

### Operators + Text Objects

Operators combine with motions (e.g., `dw` deletes word, `y$` yanks to line end).

| Operator | Action |
| -------- | ------ |
| `d` | Delete |
| `y` | Yank (copy) |
| `c` | Change (delete + insert) |

| Text Object | Action |
| ----------- | ------ |
| `iw` / `aw` | Inner word / around word |

### Commands

| Key | Action |
| --- | ------ |
| `x` / `X` | Delete char under / before cursor |
| `D` / `C` / `Y` | Delete / change / yank to line end |
| `dd` / `yy` / `cc` | Delete / yank / change line |
| `J` | Join lines |
| `p` / `P` | Paste after / before cursor |
| `u` / `Ctrl+r` | Undo / redo |
| `>>` / `<<` | Indent / outdent line |

### Counts

Prefix with numbers: `5j` (move down 5), `3dw` (delete 3 words), `10x` (delete 10 chars).

## Screenshots

| Normal | Insert | Visual |
| ------ | ------ | ------ |
| ![Normal](docs/images/Component-2.png) | ![Insert](docs/images/Component-3.png) | ![Visual](docs/images/Component-4.png) |

![Indicator Position](docs/images/change-indicator-position.gif)

![Visual Mode](docs/images/visual-C-u-d.gif)

## License

MIT License - see [LICENSE](LICENSE) for details.
