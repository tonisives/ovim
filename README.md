# ovim

macOS system-wide Vim keybindings and modal editor.

**ovim has two independent editing modes:**

| In-Place Mode                                                                                                                                          | Edit Popup                                                                                                                                   |
| ------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Simulates Vim motions by intercepting keystrokes and injecting native macOS key events. Works instantly in any app. Supports a subset of Vim commands. | Opens your actual Neovim installation in a terminal window with your full config and plugins. Edit complex text, then paste back with `:wq`. |
| ![c-u-d](docs/images/visual-C-u-d.gif)                                                                                                                 | ![Edit Popup](docs/images/edit-popup.gif)                                                                                                    |

## Features

| Feature           | In-Place Mode                                                            | Edit Popup                               |
| ----------------- | ------------------------------------------------------------------------ | ---------------------------------------- |
| Vim support       | Basic motions, operators, text objects ([see list](docs/keybindings.md)) | Full Neovim with all your plugins        |
| User config       | In app configuration for widgets, ignore list                            | Uses your `~/.config/nvim`               |
| Speed             | Instant                                                                  | ~500ms (terminal startup)                |
| App compatibility | All apps (with Accessibility permission)                                 | Apps with Accessibility API or browsers  |
| Use case          | Quick edits, modal navigation                                            | Complex edits, regex, macros, multi-line |

## Installation

### Homebrew

```bash
brew install --cask tonisives/tap/ovim
```

### GitHub Releases

Download the latest `.dmg` from the [Releases](https://github.com/tonisives/ovim/releases) page.

### Build from Source

```bash
git clone https://github.com/tonisives/ovim.git
cd ovim
pnpm install
pnpm package
# Signed app installed in build/ovim.app
```

Requires [Rust](https://rustup.rs/), [Node.js](https://nodejs.org/) v18+, and [pnpm](https://pnpm.io/).

### Package after local merges

The repository includes a tracked `post-merge` hook that builds the macOS app
from the primary checkout and installs it at `~/workspace/_tools/ovim.app`:

```bash
pnpm hooks:install
```

The Git setting is shared with linked worktrees, but merges inside task
worktrees are skipped. Set `OVIM_OUTPUT_DIR` to choose another installation
directory, or set `OVIM_SKIP_POST_MERGE_PACKAGE=1` to skip a build for one
merge. The hook does not restart a running app.

### Keep permissions across local builds

`pnpm package` uses your Developer ID Application certificate and preserves the
signature during installation. If multiple signing identities are available,
set `APPLE_SIGNING_IDENTITY` to the certificate name or SHA-1 shown by
`security find-identity -v -p codesigning`. Certificates with the same name are
selected by SHA-1 to avoid ambiguous signing requests. Keep using the same
signing identity, bundle identifier (`com.tonis.ovim`), and installation path.
The post-merge hook uses this same packaging command.

After switching from an old ad-hoc build, quit Ovim, open the signed build at its
usual path, and grant Accessibility and Input Monitoring again if requested.
A manually revoked permission still needs to be granted again. General Settings
always provides buttons to open both permission panels, even when macOS reports
access as allowed. Relaunch after changing permissions if the running process
still reports the old state.

The installer rejects ad-hoc builds rather than replacing a signed installation
with an identity that changes on every rebuild. Tauri updater keys sign update
archives; they do not replace an Apple code-signing certificate.

Apple documents the identity mechanism in
[Code Signing Requirements](https://developer.apple.com/documentation/technotes/tn3127-inside-code-signing-requirements).

## Requirements

- macOS 10.15 (Catalina) or later
- **Accessibility permission** - Grant in System Settings > Privacy & Security > Accessibility
- Terminal app. `Alacritty` is recommended. We also include Kitty, Terminal.app and WezTerm limited support.

## Quick Start

1. Launch ovim - it appears in your menu bar
2. Grant Accessibility permission when prompted
3. Access Settings from the menu bar icon

| In-Place Mode                                                                                   | Edit Popup                                                                                                     |
| ----------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| Press **Caps Lock** to toggle between Normal/Insert modes. Use Vim motions directly in any app. | Assign a shortcut to "Toggle Edit Popup" in Settings. Press it to open Neovim, edit, then `:wq` to paste back. |

## In-Place Mode

Toggle between Normal and Insert modes with Caps Lock (configurable). In Normal mode, use Vim motions directly in any application.

**Supported commands:** `hjkl`, `w/b/e`, `0/$`, `gg/G`, `d/y/c` + motions, `dd/yy/cc`, `x`, `p/P`, `u/Ctrl+r`, Visual mode, counts, and more. See [docs/keybindings.md](docs/keybindings.md) for the full list.

![Indicator Position](docs/images/change-indicator-position.gif)

### Widgets

Display battery status, time, or selection info

## Edit Popup

Opens your actual Neovim installation in a terminal window. Your full config (`~/.config/nvim`) and all plugins are available.

**How it works:**

1. Assign a shortcut to "Toggle Edit Popup" in Settings
2. Select text in any application (optional - captures existing text)
3. Press your shortcut to open Neovim in a popup terminal
4. Edit with your full Neovim setup (plugins, keybindings, macros, etc.)
5. Type `:wq` to save and paste back, or close the window to cancel

**Supported terminals:** Alacritty, Kitty, WezTerm, iTerm2, Terminal.app

With AeroSpace running, Ovim automatically floats its own Alacritty popup before positioning it beside the text field. Regular Alacritty windows keep their layout. This also covers prewarmed editor windows and requires the `aerospace` CLI on PATH or in a standard Homebrew location; no AeroSpace configuration change is needed.

The optional [bmux edit adapter](docs/edit-adapters.md) keeps trusted keyboard/text input for controlled editors such as X. Temporary replacement selections are hidden only in the edited field, then collapsed and restored after the update, including on failure.

## More features

- Edit adapters - connect app-specific text capture and synchronization. [docs/edit-adapters.md](docs/edit-adapters.md)
- Custom launch script - use your own editor. [docs/launcher-script.md](docs/launcher-script.md)
- CLI - toggle modes via CLI. [docs/cli.md](docs/cli.md)

Check out [full docs](docs/README.md) and the [official website](https://vim.tonis.dev/)

## Issues

Please check logs at `/tmp/ovim-rust.log` and submit an [issue](https://github.com/tonisives/ovim/issues)

## Also by the author

[**ClawTab**](https://clawtab.cc) - Schedule and manage Claude Code agents, shell scripts, and AI jobs from your macOS menu bar. Control everything remotely from your phone.

[![ClawTab](https://clawtab.cc/assets/job-list.png)](https://clawtab.cc)

## License

MIT License - see [LICENSE](LICENSE) for details.
