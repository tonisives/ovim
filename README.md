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

## Edit Popup

Press `ge` in Normal mode to open the edit popup - a full Neovim editor window for complex text editing. This is useful when you need more powerful editing capabilities than the basic Vim keybindings provide.

![Edit Popup](docs/images/edit-popup.gif)

**How it works:**

1. Select text in any application (optional)
2. Press `ge` to open the edit popup
3. Edit using your full Neovim configuration
4. Press `<leader>w` to save and paste the content back, or `<Esc><Esc>` to cancel

The edit popup uses your existing Neovim configuration, so all your plugins, keybindings, and settings are available.

## Vim Commands

See [docs/keybindings.md](docs/keybindings.md) for the full list of supported Vim keybindings.

## CLI Tool

ovim includes a CLI for controlling modes from scripts or tools like Karabiner-Elements:

```bash
ovim toggle   # Toggle between insert/normal mode
ovim normal   # Enter normal mode
ovim insert   # Enter insert mode
ovim mode     # Get current mode
```

See [docs/cli.md](docs/cli.md) for full CLI documentation and Karabiner integration examples.

## Screenshots

| Normal                                 | Insert                                 | Visual                                 |
| -------------------------------------- | -------------------------------------- | -------------------------------------- |
| ![Normal](docs/images/Component-2.png) | ![Insert](docs/images/Component-3.png) | ![Visual](docs/images/Component-4.png) |

![Indicator Position](docs/images/change-indicator-position.gif)

![Visual Mode](docs/images/visual-C-u-d.gif)

## License

MIT License - see [LICENSE](LICENSE) for details.
