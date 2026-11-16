#if os(macOS)
import AppKit

/// The sidebar's hide and show, with the list's width change inside the same motion (owner 2026-11-08 14:52, MOTION.md: one motion,
/// no second animation after it). AppKit animates a collapse by itself but cannot move the list's width with it (a pin set once makes
/// the list jump, and a divider set mid-slide reads the live geometry and lands wrong), so both panes are driven here, one display
/// frame at a time, by pinning their split items' thickness to the same eased progress. The reader takes what both leave. The state
/// already changed on the input frame; this only draws the travel. Interruptible: `start` again re-targets from the widths on screen.
/// Reduced Motion never reaches this (the shell jumps).
@MainActor
final class MacSlideTween: NSObject {
    struct Pane { let item: NSSplitViewItem; let view: NSView; let rest: (min: CGFloat, max: CGFloat) }
    private let sidebar: Pane, list: Pane
    private let duration = TimeInterval(WhooshConstants.TAB_MORPH_MS) / 1000
    private var link: CADisplayLink?
    private var began: CFTimeInterval = 0
    private var sbFrom: CGFloat = 0, sbTo: CGFloat = 0, lsFrom: CGFloat = 0, lsTo: CGFloat = 0
    private var hiding = false
    private let finished: () -> Void
    private(set) var running = false
    private var backstop: Task<Void, Never>?

    init(sidebar: Pane, list: Pane, finished: @escaping () -> Void) {
        self.sidebar = sidebar; self.list = list; self.finished = finished
    }

    /// The slide's curve (control points 0.2, 0.9, 0.3, 1), solved for y at the elapsed fraction.
    static func ease(_ x: Double) -> Double {
        let (x1, y1, x2, y2) = (0.2, 0.9, 0.3, 1.0)
        func bez(_ t: Double, _ a: Double, _ b: Double) -> Double { 3 * (1 - t) * (1 - t) * t * a + 3 * (1 - t) * t * t * b + t * t * t }
        var lo = 0.0, hi = 1.0
        for _ in 0..<24 { let mid = (lo + hi) / 2; if bez(mid, x1, x2) < x { lo = mid } else { hi = mid } }
        return bez((lo + hi) / 2, y1, y2)
    }

    /// `sidebarTo` is 0 when hiding, the sidebar's width when showing; `listTo` the list's width afterwards.
    func start(sidebarTo: CGFloat, listTo: CGFloat) {
        sbFrom = sidebar.item.isCollapsed ? 0 : sidebar.view.frame.width
        lsFrom = list.view.frame.width
        sbTo = sidebarTo; lsTo = listTo
        hiding = sidebarTo == 0
        pin(sidebar, sbFrom); pin(list, lsFrom)
        if sidebar.item.isCollapsed { sidebar.item.isCollapsed = false }   // showing: opens at the pinned zero width, nothing visible yet
        began = CACurrentMediaTime()
        running = true
        link?.invalidate()
        link = list.view.displayLink(target: self, selector: #selector(tick(_:)))
        link?.add(to: .main, forMode: .common)
        backstop?.cancel()
        backstop = Task { @MainActor [weak self] in
            try? await Task.sleep(nanoseconds: UInt64((self?.duration ?? 0.22) * 1_000_000_000) + 150_000_000)
            if !Task.isCancelled { self?.finish() }
        }
    }

    private func pin(_ p: Pane, _ w: CGFloat) { p.item.minimumThickness = w; p.item.maximumThickness = w }

    @objc private func tick(_ l: CADisplayLink) {
        let t = min(1, (CACurrentMediaTime() - began) / duration)
        let e = CGFloat(Self.ease(t))
        pin(sidebar, (sbFrom + (sbTo - sbFrom) * e).rounded())
        pin(list, (lsFrom + (lsTo - lsFrom) * e).rounded())
        guard t >= 1 else { return }
        finish()
    }

    /// The end state: reached by the last display frame, or by the backstop below when the window gets no display frames
    /// (an occluded or hidden window): the slide never stays half done.
    private func finish() {
        guard running else { return }
        link?.invalidate(); link = nil
        backstop?.cancel(); backstop = nil
        pin(sidebar, sbTo); pin(list, lsTo)
        if hiding { sidebar.item.isCollapsed = true }   // already zero wide: nothing moves
        sidebar.item.minimumThickness = sidebar.rest.min; sidebar.item.maximumThickness = sidebar.rest.max
        list.item.minimumThickness = list.rest.min; list.item.maximumThickness = list.rest.max
        running = false
        finished()
    }

    func cancel() {
        link?.invalidate(); link = nil; running = false; backstop?.cancel(); backstop = nil
        sidebar.item.minimumThickness = sidebar.rest.min; sidebar.item.maximumThickness = sidebar.rest.max
        list.item.minimumThickness = list.rest.min; list.item.maximumThickness = list.rest.max
    }
}
#endif
