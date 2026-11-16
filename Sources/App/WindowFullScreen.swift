import Foundation
#if os(macOS)
import AppKit
#endif

@MainActor
public extension WindowControl {
    /// `window fullscreen on|off`: the main window enters or leaves full screen (the green button's path) and the reply
    /// waits until AppKit says it is there. Full screen moves the screen to a Space of its own, so the hidden rig refuses it.
    static func fullScreen(_ on: Bool) throws -> Bool {
        #if os(macOS)
        guard !HiddenRig.isOn else { throw ControlFailure("not judged by design: full screen moves the screen to a Space of its own, and a test copy never shows") }
        guard let w = WindowBridge.mainWindow() else { throw ControlFailure("no main window") }
        if w.styleMask.contains(.fullScreen) != on { w.toggleFullScreen(nil) }
        SurfaceSettle.wait { w.styleMask.contains(.fullScreen) == on }
        guard w.styleMask.contains(.fullScreen) == on else { throw ControlFailure("the main window did not \(on ? "enter" : "leave") full screen in time") }
        return on
        #else
        throw ControlFailure("window fullscreen is served on the Mac only")
        #endif
    }
}
