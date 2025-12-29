import Cocoa
import ApplicationServices

// Get main screen info
let mainScreen = NSScreen.main!
let mainHeight = mainScreen.frame.height
print("Main screen height: \(mainHeight)")
print("Main screen frame (Cocoa): \(mainScreen.frame)")

// Get frontmost app's window position in AX coords
let frontApp = NSWorkspace.shared.frontmostApplication!
let pid = frontApp.processIdentifier
print("\nFrontmost app: \(frontApp.localizedName ?? "unknown")")

let appElement = AXUIElementCreateApplication(pid)
var focusedWindow: AnyObject?
if AXUIElementCopyAttributeValue(appElement, "AXFocusedWindow" as CFString, &focusedWindow) == .success,
   let window = focusedWindow {
    var position: AnyObject?
    var size: AnyObject?
    AXUIElementCopyAttributeValue(window as! AXUIElement, "AXPosition" as CFString, &position)
    AXUIElementCopyAttributeValue(window as! AXUIElement, "AXSize" as CFString, &size)

    var axPoint = CGPoint.zero
    var axSize = CGSize.zero
    AXValueGetValue(position as! AXValue, .cgPoint, &axPoint)
    AXValueGetValue(size as! AXValue, .cgSize, &axSize)

    print("\nWindow AX position: (\(axPoint.x), \(axPoint.y))")
    print("Window AX size: \(axSize.width) x \(axSize.height)")

    // Calculate Cocoa position for the window's top-left corner
    // AX y is from top of main screen, Cocoa y is from bottom of main screen
    let cocoaY = mainHeight - axPoint.y - axSize.height
    print("\nCalculated Cocoa position for window: (\(axPoint.x), \(cocoaY))")

    // For a hint at the top-left of the window:
    // hint_y in AX = window AX y (top of window)
    // hint_y in Cocoa = mainHeight - hint_ax_y - hint_height
    let hintHeight = 15.0
    let hintCocoaY = mainHeight - axPoint.y - hintHeight
    print("Hint at window top-left in Cocoa: (\(axPoint.x), \(hintCocoaY))")

    // Verify by creating a test window at that position
    print("\n--- Creating test window ---")
    let testWindow = NSWindow(
        contentRect: NSRect(x: axPoint.x, y: hintCocoaY, width: 100, height: 30),
        styleMask: [.borderless],
        backing: .buffered,
        defer: false
    )
    testWindow.backgroundColor = .red
    testWindow.level = .floating
    testWindow.orderFrontRegardless()

    print("Test window frame: \(testWindow.frame)")
    print("Press Enter to close...")
    _ = readLine()
}
