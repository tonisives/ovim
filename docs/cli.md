# CLI Tool

ovim includes a command-line tool for controlling modes from scripts or other applications like Karabiner-Elements.

## Commands

```bash
ovim mode          # Get current mode
ovim toggle        # Toggle between insert and normal mode
ovim insert        # Switch to insert mode (alias: i)
ovim normal        # Switch to normal mode (alias: n)
ovim visual        # Switch to visual mode (alias: v)
ovim set <mode>    # Set mode to insert/normal/visual
ovim edit          # Activate Edit Popup (alias: e)
ovim click         # Activate Click Mode (alias: c)
```

## Installation

The CLI is bundled with the ovim.app:

```bash
# Use directly from the app bundle
/Applications/ovim.app/Contents/MacOS/ovim toggle

# Or create a symlink for convenience
sudo ln -s /Applications/ovim.app/Contents/MacOS/ovim /usr/local/bin/ovim
```

After creating the symlink, you can use `ovim` directly from anywhere.

## How It Works

The CLI communicates with the running ovim app via a Unix socket at `~/Library/Caches/ovim.sock` (or `/tmp/ovim.sock` as fallback). The main ovim app must be running for CLI commands to work.

## Karabiner-Elements Integration

[Karabiner-Elements](https://karabiner-elements.pqrs.org/) can execute shell commands via `shell_command`, making it easy to trigger ovim mode changes from custom key mappings.

Note: The examples below use the full app bundle path. If you created a symlink to `/usr/local/bin/ovim`, you can use just `ovim` instead.

### Example: Caps Lock Toggle

This example uses Caps Lock to toggle between normal and insert modes:

```json
{
    "description": "Caps Lock toggles ovim mode",
    "manipulators": [
        {
            "type": "basic",
            "from": { "key_code": "caps_lock" },
            "to": [
                { "shell_command": "/Applications/ovim.app/Contents/MacOS/ovim toggle" }
            ]
        }
    ]
}
```

### Example: Escape Enters Normal Mode

Enter normal mode when pressing Escape (excluding terminal apps):

```json
{
    "description": "Escape enters ovim normal mode",
    "manipulators": [
        {
            "conditions": [
                {
                    "bundle_identifiers": [
                        "^com\\.apple\\.Terminal$",
                        "^com\\.googlecode\\.iterm2$",
                        "^net\\.kovidgoyal\\.kitty$"
                    ],
                    "type": "frontmost_application_unless"
                }
            ],
            "type": "basic",
            "from": { "key_code": "escape" },
            "to": [
                { "shell_command": "/Applications/ovim.app/Contents/MacOS/ovim normal" }
            ]
        }
    ]
}
```

### Example: Mouse Click Enters Insert Mode

Automatically enter insert mode when clicking (useful for text editing):

```json
{
    "description": "Mouse click enters ovim insert mode",
    "manipulators": [
        {
            "conditions": [
                {
                    "bundle_identifiers": [
                        "^com\\.apple\\.Terminal$",
                        "^com\\.googlecode\\.iterm2$"
                    ],
                    "type": "frontmost_application_unless"
                }
            ],
            "type": "basic",
            "from": { "any": "pointing_button" },
            "to": [
                { "shell_command": "/Applications/ovim.app/Contents/MacOS/ovim insert" },
                { "pointing_button": "button1" }
            ]
        }
    ]
}
```

### Full Karabiner Complex Modification

Here's a complete complex modification you can add to your `~/.config/karabiner/karabiner.json`:

```json
{
    "description": "ovim mode control",
    "manipulators": [
        {
            "type": "basic",
            "from": { "key_code": "caps_lock" },
            "to": [
                { "shell_command": "/Applications/ovim.app/Contents/MacOS/ovim toggle" }
            ]
        },
        {
            "conditions": [
                {
                    "bundle_identifiers": [
                        "^com\\.apple\\.Terminal$",
                        "^com\\.googlecode\\.iterm2$",
                        "^net\\.kovidgoyal\\.kitty$",
                        "^io\\.alacritty$"
                    ],
                    "type": "frontmost_application_unless"
                }
            ],
            "type": "basic",
            "from": { "key_code": "escape" },
            "to": [
                { "shell_command": "/Applications/ovim.app/Contents/MacOS/ovim normal" }
            ]
        }
    ]
}
```

## Tips

- The CLI returns immediately after sending the command; it doesn't wait for mode change confirmation
- If ovim is not running, the CLI will print an error and exit with code 1
- You can check the current mode with `ovim mode` in scripts
