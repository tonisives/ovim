#!/bin/bash
# ovim sample script: PATH override
#
# This script customizes the environment for the edit popup.
# Use this when you need to add custom paths (homebrew, local bins, etc.)
# but still want ovim to handle terminal spawning.
#
# To use: Copy this file to ~/Library/Application Support/ovim/terminal-launcher.sh

# Add homebrew and local bins to PATH
export PATH="/opt/homebrew/bin:$HOME/.local/bin:$PATH"

# Add any other environment customizations here
# export MY_VAR="value"

# Exit 0 tells ovim to continue with built-in terminal spawning
exit 0
