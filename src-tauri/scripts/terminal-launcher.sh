#!/bin/bash
# ovim terminal launcher script
#
# This script runs before the terminal/editor spawns. You can use it to:
# 1. Set up environment variables (PATH, etc.) for the editor
# 2. Optionally spawn the editor yourself (for custom terminals like tmux)
#
# IMPORTANT: Signal your intent via IPC:
#   ovim launcher-handled --session "$OVIM_SESSION_ID" [--pid <editor_pid>]
#   ovim launcher-fallthrough --session "$OVIM_SESSION_ID"
#
# Available environment variables:
#   OVIM_CLI        - path to ovim CLI binary (use this instead of 'ovim')
#   OVIM_SESSION_ID - unique session ID (required for IPC callbacks)
#   OVIM_FILE       - temp file path to edit
#   OVIM_EDITOR     - configured editor executable
#   OVIM_SOCKET     - RPC socket path (for live sync)
#   OVIM_TERMINAL   - selected terminal type
#   OVIM_WIDTH      - popup width in pixels
#   OVIM_HEIGHT     - popup height in pixels
#   OVIM_X          - popup x position
#   OVIM_Y          - popup y position

# Example: Spawn in tmux popup with live sync
# if command -v tmux &>/dev/null && tmux list-sessions &>/dev/null 2>&1; then
#     # Focus the terminal running tmux
#     osascript -e 'tell application "Alacritty" to activate'
#
#     # Signal we're handling it (before blocking command)
#     "$OVIM_CLI" launcher-handled --session "$OVIM_SESSION_ID"
#
#     # Spawn in tmux popup (blocks until editor closes)
#     tmux popup -E -w 80% -h 80% "$OVIM_EDITOR --listen $OVIM_SOCKET $OVIM_FILE"
#     exit 0
# fi

# Default: fallthrough to normal terminal flow
"$OVIM_CLI" launcher-fallthrough --session "$OVIM_SESSION_ID"
exit 0
