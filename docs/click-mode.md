# Click Mode

Click Mode allows you to click on any UI element using only your keyboard, similar to Vimium for browsers or Homerow for macOS.

## How It Works

```
+----------------------------------+
|  Press Cmd+Shift+F               |
|         |                        |
|         v                        |
|  +-------------+                 |
|  | Scan UI     |                 |
|  | Elements    |                 |
|  +-------------+                 |
|         |                        |
|         v                        |
|  +-------------+                 |
|  | Show Hint   |                 |
|  | Labels      |                 |
|  +-------------+                 |
|         |                        |
|         v                        |
|  Type hint letters (e.g., "AS")  |
|         |                        |
|         v                        |
|  Element is clicked!             |
+----------------------------------+
```

## Activation

**Default shortcut:** `Cmd + Shift + F`

When activated, hint labels appear on all clickable elements in the frontmost application:

```
+------------------------------------------+
|  Browser Window                     [_]  |
+------------------------------------------+
|                                          |
|   [AS] Home    [SD] About    [DF] Blog   |
|                                          |
|   +----------------------------------+   |
|   |                                  |   |
|   |   Welcome to Our Site            |   |
|   |                                  |   |
|   |   [FG] Learn More                |   |
|   |                                  |   |
|   |   [GH] Contact Us                |   |
|   |                                  |   |
|   +----------------------------------+   |
|                                          |
+------------------------------------------+

  Hint labels: [AS] [SD] [DF] [FG] [GH]
```

## Typing Hints

Type the hint characters to click the corresponding element:

```
Input: "F"

+------------------------------------------+
|   [AS] Home    [SD] About    [DF] Blog   |
|                                          |
|   [FG] Learn More    [--] (filtered out) |
|   [GH] Contact Us                        |
+------------------------------------------+

  Remaining hints: [FG] [GH]
  (hints not starting with "F" are hidden)


Input: "FG"

  -> Clicks "Learn More" button
  -> Click mode deactivates
```

## Keyboard Controls

| Key | Action |
|-----|--------|
| `a-z` (except r,c,d,n) | Type hint characters |
| `r` | Switch to right-click mode |
| `c` | Switch to cmd-click mode |
| `d` | Switch to double-click mode |
| `n` | Switch to normal click mode |
| `Backspace` | Delete last character |
| `Escape` | Cancel click mode |

## Click Actions

Press action keys to change what happens when you select a hint:

```
Example: Right-click on element with hint "AS"

1. Press Cmd+Shift+F (activate click mode)
2. Press "r" (switch to right-click mode)
3. Type "AS" (the hint)

-> Context menu appears on the element
```

## Settings

Access settings via the tray menu -> Settings -> Click Mode tab.

### Configurable Options

| Setting | Description | Default |
|---------|-------------|---------|
| Enable Click Mode | Toggle feature on/off | On |
| Activation Shortcut | Key combination to activate | Cmd+Shift+F |
| Hint Characters | Characters used for hints | asdfghjkl... |
| Font Size | Hint label text size | 11px |
| Opacity | Hint label transparency | 100% |
| Background Color | Hint label background | #FFCC00 (yellow) |
| Text Color | Hint label text color | #000000 (black) |
| Show Search Bar | Display typed input at top | On |

### Hint Character Order

Hints are generated using characters in order of preference:

```
Priority: Home row first for fastest typing

  a s d f g h j k l   <- Used first (home row)
  q w e r t y u i o p <- Used next (top row)
  z x c v b n m       <- Used last (bottom row)

Single-char hints: a, s, d, f, g, h, j, k, l
Two-char hints:    aa, as, ad, af, ag, ah, aj, ak, al, sa, ss, ...
```

## Supported Elements

Click Mode detects these accessibility roles:

- Buttons (`AXButton`)
- Links (`AXLink`)
- Menu items (`AXMenuItem`, `AXMenuBarItem`)
- Checkboxes (`AXCheckBox`)
- Radio buttons (`AXRadioButton`)
- Dropdowns (`AXPopUpButton`, `AXComboBox`)
- Text fields (`AXTextField`, `AXTextArea`)
- Tabs (`AXTab`)
- Table rows/cells (`AXRow`, `AXCell`)
- Disclosure triangles
- Sliders
- Images with actions

## Troubleshooting

### No elements appear

1. **Check accessibility permissions**
   - System Preferences -> Security & Privacy -> Privacy -> Accessibility
   - Ensure ovim is checked

2. **Application compatibility**
   - Some apps (especially Electron-based) may have limited accessibility support
   - Native macOS apps work best

3. **Window focus**
   - Ensure the target window is focused before activating

### Elements in wrong positions

This can happen with:
- Multiple monitors with different scaling
- Apps with custom window chrome
- Scrolled content (elements may be off-screen)

### Hints not responding

- Ensure the overlay window has focus
- Try pressing Escape and reactivating
- Check that Click Mode is enabled in settings

## Architecture

```
+---------------------+     +----------------------+
|   Keyboard Handler  |     |   Click Overlay      |
|   (Rust)            |     |   (React/TypeScript) |
+---------------------+     +----------------------+
         |                            ^
         | CGEventTap                 | Tauri Events
         v                            |
+---------------------+     +----------------------+
|   Click Mode        |---->|   Frontend           |
|   Manager           |     |   Hint Labels        |
+---------------------+     +----------------------+
         |
         | Accessibility API
         v
+---------------------+
|   macOS AX API      |
|   (AXUIElement)     |
+---------------------+
```

## Comparison with Similar Tools

| Feature | ovim Click Mode | Vimium | Homerow | Shortcat |
|---------|-----------------|--------|---------|----------|
| Platform | macOS | Browser | macOS | macOS |
| Scope | System-wide | Browser only | System-wide | System-wide |
| Hint style | Vimium-like | Letters | Letters | Search-based |
| Right-click | r + hint | - | Shift+hint | Yes |
| Double-click | d + hint | - | - | - |
| Cmd-click | c + hint | - | - | - |
| Search mode | Planned | Yes | Yes | Yes |
| Price | Free | Free | Paid | Paid |

## Future Improvements

- [ ] Search mode (type to filter by element text)
- [ ] Scroll mode (navigate scrollable areas)
- [ ] Drag and drop support
- [ ] Custom hint positioning
- [ ] Per-app settings
