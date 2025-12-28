#!/bin/bash
# ovim terminal launcher script
#
# This script runs before the terminal/editor spawns. You can use it to:
# 1. Run custom code before ovim start the popup
# 2. Optionally spawn the editor yourself (for custom terminals like tmux)
#
# IMPORTANT: Signal your intent via IPC:
#   ovim launcher-handled --session "$OVIM_SESSION_ID" [--pid <editor_pid>]
#   ovim launcher-fallthrough --session "$OVIM_SESSION_ID"
#
# Available environment variables:
#   OVIM_SESSION_ID - unique session ID (required for IPC callbacks)
#   OVIM_FILE       - temp file path to edit
#   OVIM_EDITOR     - configured editor executable
#   OVIM_SOCKET     - RPC socket path (for live sync)
#   OVIM_TERMINAL   - selected terminal type
#   OVIM_WIDTH      - popup width in pixels
#   OVIM_HEIGHT     - popup height in pixels
#   OVIM_X          - popup x position
#   OVIM_Y          - popup y position

# Spawn in tmux popup with live sync
# if command -v tmux &>/dev/null && tmux list-sessions &>/dev/null 2>&1; then
#     # Find the terminal window running tmux and focus it
#     CLIENT_TTY=$(tmux list-clients -F '#{client_activity} #{client_tty}' | sort -rn | head -1 | awk '{print $2}')
#
#     if [ -n "$CLIENT_TTY" ]; then
#         osascript <<EOF
# tell application "System Events"
#     set alacrittyProcess to first process whose name is "alacritty"
#     set frontmost of alacrittyProcess to true
# end tell
# tell application "Alacritty" to activate
# EOF
#     fi
#
#     # Signal we're handling it (before blocking command)
#     "$OVIM_CLI" launcher-handled --session "$OVIM_SESSION_ID"
#
#     # Spawn editor in a tmux popup window
#     # -E: close popup when command exits
#     # -w 80% -h 80%: size as percentage of terminal
#     tmux popup -E -w 80% -h 80% "$OVIM_EDITOR --listen $OVIM_SOCKET $OVIM_FILE"
# fi

# Fallthrough to normal terminal flow
"$OVIM_CLI" launcher-fallthrough --session "$OVIM_SESSION_ID"
