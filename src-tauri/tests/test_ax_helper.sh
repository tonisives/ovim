#!/bin/bash
# Test script for ovim-ax-helper
# Tests against apps that are known to cause issues

set -e

HELPER="/Users/tonis/workspace/tgs/ovim/src-tauri/target/release/ovim-ax-helper"

echo "Testing ovim-ax-helper..."
echo "========================="
echo ""

# Test 1: Helper without arguments (uses frontmost app)
echo "[Test 1] Query frontmost app..."
if output=$("$HELPER" 2>&1); then
    elem_count=$(echo "$output" | jq '. | length' 2>/dev/null || echo "0")
    echo "  PASS: Found $elem_count elements"
else
    echo "  FAIL: Helper crashed or returned error"
    echo "  Error: $output"
fi
echo ""

# Test 2: Query Finder (commonly open)
echo "[Test 2] Query Finder..."
finder_pid=$(pgrep -x Finder || echo "")
if [ -n "$finder_pid" ]; then
    if output=$("$HELPER" "$finder_pid" 2>&1); then
        elem_count=$(echo "$output" | jq '. | length' 2>/dev/null || echo "0")
        echo "  PASS: Found $elem_count elements for Finder (PID $finder_pid)"
    else
        echo "  FAIL: Helper crashed for Finder"
        echo "  Error: $output"
    fi
else
    echo "  SKIP: Finder not running"
fi
echo ""

# Test 3: Query System Preferences/Settings (known problematic)
echo "[Test 3] Query System Settings..."
settings_pid=$(pgrep -x "System Settings" || pgrep -x "System Preferences" || echo "")
if [ -n "$settings_pid" ]; then
    if output=$("$HELPER" "$settings_pid" 2>&1); then
        elem_count=$(echo "$output" | jq '. | length' 2>/dev/null || echo "0")
        echo "  PASS: Found $elem_count elements for System Settings (PID $settings_pid)"
    else
        echo "  FAIL: Helper crashed for System Settings"
        echo "  Error: $output"
    fi
else
    echo "  SKIP: System Settings not running"
fi
echo ""

# Test 4: Multiple rapid queries (stress test)
echo "[Test 4] Rapid query stress test (5 iterations)..."
failures=0
if [ -n "$finder_pid" ]; then
    for i in 1 2 3 4 5; do
        if ! "$HELPER" "$finder_pid" >/dev/null 2>&1; then
            failures=$((failures + 1))
        fi
    done
    if [ $failures -eq 0 ]; then
        echo "  PASS: All 5 queries succeeded"
    else
        echo "  WARN: $failures/5 queries failed"
    fi
else
    echo "  SKIP: Finder not running"
fi
echo ""

# Test 5: Invalid PID handling (returns empty array, doesn't crash)
echo "[Test 5] Invalid PID handling..."
output=$("$HELPER" 99999 2>&1)
if [ "$output" = "[]" ]; then
    echo "  PASS: Helper returns empty array for invalid PID"
else
    echo "  INFO: Output for invalid PID: $output"
fi
echo ""

echo "========================="
echo "Tests complete."
