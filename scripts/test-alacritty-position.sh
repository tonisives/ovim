#!/bin/bash
# Test script for Alacritty window positioning
# Usage: ./scripts/test-alacritty-position.sh [x] [y] [width] [height] [scale]
#
# Note: Alacritty uses physical pixels, macOS uses points.
# On Retina displays, multiply by scale factor (default 2).

X=${1:-500}
Y=${2:-300}
WIDTH=${3:-80}
HEIGHT=${4:-24}
SCALE=${5:-2}

# Apply scale factor for Retina
PHYS_X=$((X * SCALE))
PHYS_Y=$((Y * SCALE))

echo "Testing Alacritty spawn"
echo "  Points (macOS): ($X, $Y)"
echo "  Pixels (Alacritty): ($PHYS_X, $PHYS_Y)"
echo "  Size: ${WIDTH}x${HEIGHT} cells"

# Spawn with position (using physical pixels)
alacritty \
    -o "window.title=\"ovim-test-position\"" \
    -o "window.dynamic_title=false" \
    -o "window.startup_mode=\"Windowed\"" \
    -o "window.dimensions.columns=$WIDTH" \
    -o "window.dimensions.lines=$HEIGHT" \
    -o "window.position={x=$PHYS_X,y=$PHYS_Y}" \
    -e bash -c "echo 'Position test'; echo 'Points: x=$X y=$Y'; echo 'Pixels: x=$PHYS_X y=$PHYS_Y'; echo 'Press enter to close'; read" &

sleep 1

# Get position of the front window (should be our new one)
ACTUAL=$(osascript -e 'tell application "System Events" to tell process "alacritty" to get position of front window as text')

echo "Requested: ($X, $Y)"
echo "Actual:    $ACTUAL"

# Show difference
REQ_X=$X
REQ_Y=$Y
ACT_X=$(echo "$ACTUAL" | cut -d',' -f1 | tr -d ' ')
ACT_Y=$(echo "$ACTUAL" | cut -d',' -f2 | tr -d ' ')

if [ -n "$ACT_X" ] && [ -n "$ACT_Y" ]; then
    DIFF_X=$((ACT_X - REQ_X))
    DIFF_Y=$((ACT_Y - REQ_Y))
    echo "Difference: x=$DIFF_X, y=$DIFF_Y"
fi
