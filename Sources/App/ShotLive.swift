#if os(macOS)
import AppKit
import ScreenCaptureKit

/// Captures a window's LIVE pixels (what the window server composites), so a WKWebView body, which
/// `cacheDisplay` leaves blank, appears. `WHOOSH_SHOT_RENDER=1` restores the old render path (the mutant).
@MainActor
enum ShotLive {
    static let timeoutS: Double = 5 // SHOT_LIVE_TIMEOUT_S: asked of the contracts lane for docs/CONSTANTS.md (a row from this lane breaks the gen-constants drift check)

    static var disabled: Bool { ProcessInfo.processInfo.environment["WHOOSH_SHOT_RENDER"] == "1" }

    private typealias ListImage = @convention(c) (CGRect, UInt32, UInt32, UInt32) -> Unmanaged<CGImage>?

    /// The window server's image of one of THIS process's windows. Own windows need no Screen Recording
    /// permission (a rig copy has none: ScreenCaptureKit is refused with a TCC error). The SDK marks
    /// CGWindowListCreateImage unavailable, so it is looked up by name.
    private static func windowListImage(_ id: CGWindowID) -> CGImage? {
        guard let h = dlopen(nil, RTLD_NOW), let sym = dlsym(h, "CGWindowListCreateImage") else { return nil }
        let f = unsafeBitCast(sym, to: ListImage.self)
        // optionIncludingWindow (1 << 3), boundsIgnoreFraming (1 << 0) | bestResolution (1 << 3)
        return f(CGRect.null, 1 << 3, id, (1 << 0) | (1 << 3))?.takeRetainedValue()
    }

    /// PNG bytes of `w` as drawn on screen, or nil with the reason in `why`.
    static func png(of w: NSWindow, why: inout String) -> Data? {
        let id = CGWindowID(w.windowNumber)
        guard id != 0 else { why = "the window has no window id"; return nil }
        // A window opened a moment ago is not on screen yet: ask again for up to a second.
        for _ in 0..<10 {
            if let img = windowListImage(id), img.width > 1, img.height > 1 {
                why = ""
                return NSBitmapImageRep(cgImage: img).representation(using: .png, properties: [:])
            }
            RunLoop.main.run(until: Date().addingTimeInterval(0.1))
        }
        let box = Box()
        let scale = w.backingScaleFactor
        let sem = DispatchSemaphore(value: 0)
        SCShareableContent.getExcludingDesktopWindows(false, onScreenWindowsOnly: true) { content, err in
            guard let win = content?.windows.first(where: { $0.windowID == id }) else {
                box.why = err.map { "\($0.localizedDescription)" } ?? "the window server does not list window \(id) on screen"
                sem.signal(); return
            }
            let cfg = SCStreamConfiguration()
            cfg.width = Int(win.frame.width * scale)
            cfg.height = Int(win.frame.height * scale)
            cfg.showsCursor = false
            cfg.ignoreShadowsSingleWindow = true
            SCScreenshotManager.captureImage(contentFilter: SCContentFilter(desktopIndependentWindow: win), configuration: cfg) { img, err in
                if let img { box.image = img } else { box.why = err.map { "\($0.localizedDescription)" } ?? "no image" }
                sem.signal()
            }
        }
        guard sem.wait(timeout: .now() + timeoutS) == .success else { why = "the capture timed out after \(Int(timeoutS)) s"; return nil }
        guard let img = box.image else { why = box.why; return nil }
        why = ""
        return NSBitmapImageRep(cgImage: img).representation(using: .png, properties: [:])
    }

    private final class Box: @unchecked Sendable {
        var image: CGImage?
        var why = ""
    }
}
#endif
