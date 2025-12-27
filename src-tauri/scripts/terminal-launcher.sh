#!/bin/bash
# ovim terminal launcher script
#
# This script runs before the terminal/editor spawns. You can use it to:
# 1. Set up environment variables (PATH, etc.) for the editor
# 2. Optionally spawn the editor yourself (for custom terminals like tmux)
#
# Exit codes:
#   exit 0  - If no editor spawned: fall through to normal terminal flow
#           - If editor spawned: ovim will wait for that editor process
#   exit 1+ - Error, abort the edit popup
#
# Available environment variables:
#   OVIM_FILE     - temp file path to edit
#   OVIM_EDITOR   - configured editor executable
#   OVIM_SOCKET   - RPC socket path (for live sync)
#   OVIM_TERMINAL - selected terminal type
#   OVIM_WIDTH    - popup width in pixels
#   OVIM_HEIGHT   - popup height in pixels
#   OVIM_X        - popup x position
#   OVIM_Y        - popup y position

# Example: Add homebrew and local bins to PATH
# export PATH="/opt/homebrew/bin:$HOME/.local/bin:$PATH"

# Example: Spawn in tmux popup (blocks until editor closes)
# tmux popup -E -w 80% -h 80% "$OVIM_EDITOR --listen $OVIM_SOCKET $OVIM_FILE"
# exit 0

# Example: Focus existing tmux window and spawn there
# tmux send-keys -t :editor "$OVIM_EDITOR --listen $OVIM_SOCKET $OVIM_FILE" Enter
# exit 0

# Default: exit 0 to continue with normal terminal flow
exit 0
