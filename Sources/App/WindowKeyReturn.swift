#if os(macOS)
import AppKit

/// When a window that holds the keyboard leaves the screen, the keyboard goes to the window under it. AppKit does that by itself only in the
/// ACTIVE app, and not at once: in a copy that is not the active app (the hidden test copy never is) no window takes it, and a field focused in
/// the main window during that gap drops every typed letter (lead finding 109, main172 and main173: the composer's window hidden, the search
/// opened less than half a second later, three real letters: 0 arrive; after `makeKey()` on the main window: 3 arrive).
/// So the code that hides such a window calls `afterHide(of:)` on the same line of time: the front window that remains is made key at once,
/// with no gap. `makeKey()` brings nothing forward and activates nothing.
@MainActor
public enum WindowKeyReturn {
    /// Test door: how many times a window was given the keyboard back (`rows mac-keyevent.monitor` serves it as `key_returned`).
    public private(set) static var returned = 0

    /// Call right after `hidden` was taken off the screen (`orderOut`). Does nothing when AppKit already named another key window.
    public static func afterHide(of hidden: NSWindow?) {
        if let key = NSApp.keyWindow, key !== hidden { return }
        let next = NSApp.orderedWindows.first { $0 !== hidden && $0.isVisible && $0.canBecomeKey && WindowBridge.kind($0) == "window" } ?? WindowBridge.mainWindow()
        guard let next, next !== hidden else { return }
        next.makeKey()
        returned += 1
    }
}
#endif
