#if os(macOS)
import AppKit

/// The chips on screen, so Quick Look can zoom from the right one and the socket can say where each chip is.
@MainActor
enum AttachmentChipFrames {
    private final class Weak { weak var view: NSView?; init(_ v: NSView) { view = v } }
    private static var views: [AttachmentRef: Weak] = [:]

    static func register(_ ref: AttachmentRef, _ view: NSView) { views[ref] = Weak(view) }
    static func unregister(_ ref: AttachmentRef, _ view: NSView) { if views[ref]?.view === view { views[ref] = nil } }
    static func view(_ ref: AttachmentRef) -> NSView? { views[ref]?.view }

    /// The chip's frame in screen points, or nil when it is not in a window.
    static func screenFrame(_ ref: AttachmentRef) -> NSRect? {
        guard let v = view(ref), let w = v.window else { return nil }
        return w.convertToScreen(v.convert(v.bounds, to: nil))
    }
}
#endif
