import Cocoa
import ApplicationServices

// Get System Settings window position
let frontApp = NSWorkspace.shared.frontmostApplication!
let pid = frontApp.processIdentifier
print("App: \(frontApp.localizedName ?? "unknown") PID: \(pid)")

let appElement = AXUIElementCreateApplication(pid)
var focusedWindow: AnyObject?
if AXUIElementCopyAttributeValue(appElement, "AXFocusedWindow" as CFString, &focusedWindow) == .success,
   let window = focusedWindow {
    var position: AnyObject?
    var size: AnyObject?
    AXUIElementCopyAttributeValue(window as! AXUIElement, "AXPosition" as CFString, &position)
    AXUIElementCopyAttributeValue(window as! AXUIElement, "AXSize" as CFString, &size)

    var point = CGPoint.zero
    var sz = CGSize.zero
    AXValueGetValue(position as! AXValue, .cgPoint, &point)
    AXValueGetValue(size as! AXValue, .cgSize, &sz)

    print("\nWindow AX Position: (\(point.x), \(point.y))")
    print("Window AX Size: \(sz.width) x \(sz.height)")

    // Get a few clickable elements
    print("\n--- Sample Elements ---")
    func findElements(_ element: AXUIElement, depth: Int = 0) {
        if depth > 3 { return }

        var role: AnyObject?
        AXUIElementCopyAttributeValue(element, "AXRole" as CFString, &role)
        let roleStr = (role as? String) ?? ""

        if roleStr == "AXButton" || roleStr == "AXCheckBox" || roleStr == "AXStaticText" {
            var pos: AnyObject?
            var szVal: AnyObject?
            var title: AnyObject?
            AXUIElementCopyAttributeValue(element, "AXPosition" as CFString, &pos)
            AXUIElementCopyAttributeValue(element, "AXSize" as CFString, &szVal)
            AXUIElementCopyAttributeValue(element, "AXTitle" as CFString, &title)

            var p = CGPoint.zero
            var s = CGSize.zero
            if pos != nil { AXValueGetValue(pos as! AXValue, .cgPoint, &p) }
            if szVal != nil { AXValueGetValue(szVal as! AXValue, .cgSize, &s) }
            let t = (title as? String) ?? ""

            print("\(roleStr): '\(t.prefix(30))' at (\(p.x), \(p.y)) size \(s.width)x\(s.height)")
        }

        var children: AnyObject?
        if AXUIElementCopyAttributeValue(element, "AXChildren" as CFString, &children) == .success,
           let childArray = children as? [AXUIElement] {
            for child in childArray.prefix(10) {
                findElements(child, depth: depth + 1)
            }
        }
    }
    findElements(window as! AXUIElement)
}

// Screen info
print("\n--- Screen Info ---")
let mainHeight = NSScreen.main!.frame.height
print("Main screen height: \(mainHeight)")
for (i, screen) in NSScreen.screens.enumerated() {
    let f = screen.frame
    // What Rust calculates
    let rustY = mainHeight - f.origin.y - f.size.height
    print("Screen \(i): Cocoa(\(f.origin.x), \(f.origin.y)) -> Rust offset (\(f.origin.x), \(rustY))")
}
