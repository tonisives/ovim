#!/bin/bash
# ovim terminal launcher script
#
# This script is used to customize the environment for the edit popup.
# Any environment variables you export here (like PATH) will be inherited
# by the terminal and editor.

# Add homebrew and local bins to PATH
export PATH="/opt/homebrew/bin:$HOME/.local/bin:$PATH"

# For custom terminal spawning, implement your logic here.
# See sample scripts in ~/Library/Application Support/ovim/samples/
# Example: tmux popup
# tmux popup -E -w 80% -h 80% "$OVIM_EDITOR --listen $OVIM_SOCKET $OVIM_FILE"

# Exit 0 tells ovim to continue with built-in terminal spawning
exit 0
