#!/bin/bash
# ovim sample script: tmux popup
#
# Opens the editor in a tmux popup window within your existing tmux session.
# Requires: tmux running in your terminal
#
# To use: Copy this file to ~/Library/Application Support/ovim/terminal-launcher.sh

# Add homebrew and local bins to PATH
export PATH="/opt/homebrew/bin:$HOME/.local/bin:$PATH"

# Find the terminal window running tmux and focus it
# Get the tty of the most recently active tmux client
CLIENT_TTY=$(tmux list-clients -F '#{client_activity} #{client_tty}' | sort -rn | head -1 | awk '{print $2}')

if [ -n "$CLIENT_TTY" ]; then
    # Find the PID of the process owning this tty
    TTY_PID=$(ps -t "$CLIENT_TTY" -o pid= | head -1 | tr -d ' ')

    if [ -n "$TTY_PID" ]; then
        # Focus Alacritty window (adjust for your terminal)
        osascript <<EOF 2>/dev/null
tell application "System Events"
    set alacrittyProcess to first process whose name is "alacritty"
    set alacrittyWindows to windows of alacrittyProcess
    repeat with w in alacrittyWindows
        try
            set frontmost of alacrittyProcess to true
            perform action "AXRaise" of w
            exit repeat
        end try
    end repeat
end tell
tell application "Alacritty" to activate
EOF
    fi
fi

# Spawn editor in a tmux popup window
# -E: close popup when command exits
# -w 80% -h 80%: size as percentage of terminal
tmux popup -E -w 80% -h 80% "$OVIM_EDITOR --listen $OVIM_SOCKET $OVIM_FILE"
