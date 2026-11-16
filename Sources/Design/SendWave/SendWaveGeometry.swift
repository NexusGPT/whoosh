#if os(macOS)
import AppKit

/// Where the one wave starts, in SCREEN points (AppKit: origin bottom-left), and how each window sees it.
@MainActor
enum SendWaveGeometry {
    /// The round end of the send button (its trailing end, where the arrow or the label sits) and the button's
    /// frame, both on screen. The effect leaves the button out, so it always draws LIVE (its sent state on the
    /// key's frame, never a copy). A probe frame is taken only when it lies at the sending pane (two views can
    /// share one id, e.g. the notch's own quick reply); otherwise the origin is the pane's bottom-trailing corner,
    /// where both panes keep their send button, and nothing is left out. `pane` and the probe are in the window's
    /// content points, origin top-left (SwiftUI's global space on the Mac).
    static func origin(button id: String, pane: CGRect, in window: NSWindow) -> (CGPoint, CGRect) {
        let near = pane.insetBy(dx: -WhooshMetrics.actionButton * 2, dy: -WhooshMetrics.actionButton * 2)
        guard let b = FrameProbe.frame(id), near.contains(CGPoint(x: b.midX, y: b.midY)) else {
            let inset = WhooshMetrics.actionButton
            let corner = toScreen(CGRect(x: pane.maxX - inset, y: pane.maxY - inset, width: 0, height: 0), in: window)
            return (corner.origin, .zero)
        }
        let r = toScreen(b, in: window)
        let o = CGPoint(x: r.maxX - r.height / 2, y: r.midY)
        return (o, SendWaveCenter.mutant == "freeze-button" ? .zero : r)
    }

    /// Content points (top-left origin) to screen points.
    static func toScreen(_ rect: CGRect, in window: NSWindow) -> CGRect {
        guard let view = window.contentView else { return rect }
        let inView = view.isFlipped ? rect : CGRect(x: rect.minX, y: view.bounds.height - rect.maxY, width: rect.width, height: rect.height)
        return window.convertToScreen(view.convert(inView, to: nil))
    }

    /// A screen point in a window's own drawing points (origin at the window's top-left corner).
    static func local(_ p: CGPoint, in frame: CGRect) -> CGPoint { CGPoint(x: p.x - frame.minX, y: frame.maxY - p.y) }

    static func local(_ r: CGRect, in frame: CGRect) -> CGRect {
        r.isEmpty ? .zero : CGRect(x: r.minX - frame.minX, y: frame.maxY - r.maxY, width: r.width, height: r.height)
    }

    /// The wave's reach: from the origin to the farthest corner of every window it crosses.
    static func reach(from o: CGPoint, over frames: [CGRect]) -> CGFloat {
        frames.flatMap { f in [CGPoint(x: f.minX, y: f.minY), CGPoint(x: f.maxX, y: f.minY), CGPoint(x: f.minX, y: f.maxY), CGPoint(x: f.maxX, y: f.maxY)] }
            .map { hypot($0.x - o.x, $0.y - o.y) }.max() ?? 1
    }
}
#endif
