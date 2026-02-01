# Monaco Editor Live Sync

## Problem

Monaco editors on sites like neetcode.io, leetcode.com, and other coding platforms don't support live sync. The text is only updated when nvim exits (via clipboard paste).

## Root Cause

Monaco editors on these sites are bundled without exposing global APIs:
- No global `monaco` or `editor` objects
- No `monaco.editor.getEditors()` available
- Angular/React component internals are not accessible
- Webpack bundles Monaco in closures with no external access

## Why JavaScript-based solutions don't work

Monaco checks `event.isTrusted` on all input events. This property:
- Is `true` only for real user-initiated events (actual keyboard presses)
- Is `false` for all programmatically dispatched events
- Cannot be spoofed - it's a browser security feature

### Attempted approaches that failed:

1. **Direct Monaco API** - Not exposed globally
2. **React/Angular fiber traversal** - Editor instance not found
3. **Webpack chunk inspection** - Monaco hidden in closures
4. **EditContext API** - Updates internal state but Monaco doesn't sync from it
5. **KeyboardEvent dispatch** - Ignored due to `isTrusted: false`
6. **InputEvent dispatch** - Ignored due to `isTrusted: false`
7. **CompositionEvent** - Ignored due to `isTrusted: false`
8. **ClipboardEvent** - Ignored due to `isTrusted: false`
9. **document.execCommand** - Returns false, no effect
10. **Prototype hooks** - Editor created before hooks can be installed

### What does work:
- Real keyboard events (user actually pressing keys)
- Chrome DevTools Protocol `Input.dispatchKeyEvent` (sends trusted events)
- System-level keyboard injection (but requires window focus)

## Current Behavior

When Monaco is detected but API is inaccessible:
1. Live sync fails with error containing `monaco_dom` or `monaco_not_found`
2. Accessibility API fallback is skipped (Monaco ignores AXValue changes)
3. On nvim exit, text is restored via clipboard (Cmd+A, Cmd+V)

This works reliably but there's no live preview in the browser while editing.

## Potential Solutions for Live Sync

### Option 1: Chrome DevTools Protocol (CDP)

Connect to Chrome's debugging port and send trusted keyboard events.

**Pros:**
- Sends `isTrusted: true` events
- Works without focus switching
- No browser extension needed

**Cons:**
- Requires Chrome started with `--remote-debugging-port=9222`
- Additional setup for users
- Security implications of open debug port

**Implementation:**
```rust
// Connect to CDP websocket at ws://localhost:9222
// Send Input.dispatchKeyEvent for Cmd+A, then Cmd+V
// Or use Input.insertText for direct text insertion
```

### Option 2: Browser Extension

Create a Chrome extension that:
- Listens for messages from ovim (via native messaging or localhost server)
- Uses `chrome.debugger` API to send trusted events
- Or finds Monaco through content script with higher privileges

**Pros:**
- No command-line flags needed
- Could provide better Monaco detection
- Could expose Monaco API to ovim

**Cons:**
- Requires extension installation
- Extension review/publishing process
- Maintenance burden

### Option 3: Focus Switching (Not Recommended)

Quickly switch focus: browser -> Cmd+A, Cmd+V -> back to nvim

**Pros:**
- Works with current architecture

**Cons:**
- Disruptive UX (visible flickering)
- Slow (focus switching takes time)
- May interfere with user's workflow

### Option 4: Accept Limitation

Keep current behavior: live sync for supported editors, clipboard-on-exit for Monaco.

**Pros:**
- No additional complexity
- Works reliably
- No user setup required

**Cons:**
- No live preview for Monaco-based sites

## Recommendation

Implement **Option 1 (CDP)** as an optional feature:

1. Add config option `cdp_port` in settings
2. If set, connect to CDP on that port
3. For Monaco editors, use CDP `Input.dispatchKeyEvent` for live sync
4. Fall back to clipboard-on-exit if CDP not available

This gives power users live sync capability while keeping the default behavior simple.

## Sites Affected

- neetcode.io
- leetcode.com (some editors)
- codesandbox.io
- codepen.io (Monaco mode)
- Any site using bundled Monaco without global exposure

## Files Involved

- `src/nvim_edit/mod.rs` - Live sync handler, detects Monaco failure
- `src/nvim_edit/browser_scripting/js/set_element_text.js` - Monaco detection
- `src/nvim_edit/clipboard.rs` - Clipboard-based fallback
