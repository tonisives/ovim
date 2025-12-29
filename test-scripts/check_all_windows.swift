import Cocoa
import ApplicationServices

// Find windows that match click-overlay or ovim
let apps = NSWorkspace.shared.runningApplications
for app in apps {
    let name = app.localizedName ?? ""
    // Check all apps for interesting windows
    let appElement = AXUIElementCreateApplication(app.processIdentifier)
    var windows: AnyObject?
    if AXUIElementCopyAttributeValue(appElement, "AXWindows" as CFString, &windows) == .success,
       let windowArray = windows as? [AXUIElement] {
        for window in windowArray {
            var title: AnyObject?
            AXUIElementCopyAttributeValue(window, "AXTitle" as CFString, &title)
            let titleStr = (title as? String) ?? ""

            // Look for click-overlay or ovim windows
            if titleStr.lowercased().contains("click") ||
               titleStr.lowercased().contains("overlay") ||
               titleStr.lowercased().contains("ovim") ||
               name.lowercased().contains("ovim") {
                var position: AnyObject?
                var size: AnyObject?
                AXUIElementCopyAttributeValue(window, "AXPosition" as CFString, &position)
                AXUIElementCopyAttributeValue(window, "AXSize" as CFString, &size)

                var point = CGPoint.zero
                var sz = CGSize.zero
                if let pos = position as? AXValue {
                    AXValueGetValue(pos, .cgPoint, &point)
                }
                if let s = size as? AXValue {
                    AXValueGetValue(s, .cgSize, &sz)
                }

                print("App: \(name) Window: '\(titleStr)'")
                print("  Position: (\(point.x), \(point.y))")
                print("  Size: \(sz.width) x \(sz.height)")
            }
        }
    }
}
