import Cocoa

let screens = NSScreen.screens
for (i, screen) in screens.enumerated() {
    print("Screen \(i):")
    print("  Backing scale factor: \(screen.backingScaleFactor)")
    print("  Frame: \(screen.frame)")
}
