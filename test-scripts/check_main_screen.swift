import Cocoa

print("NSScreen.main: \(NSScreen.main?.frame ?? .zero)")
print("NSScreen.screens[0]: \(NSScreen.screens[0].frame)")

for (i, screen) in NSScreen.screens.enumerated() {
    print("Screen \(i): \(screen.frame)")
}
