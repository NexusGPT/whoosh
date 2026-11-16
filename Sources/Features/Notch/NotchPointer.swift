#if os(macOS)
import AppKit

/// Where the pointer is, without Accessibility: AppKit's global and local mouse monitors. Over the shape the
/// panel takes clicks; anywhere else they pass through to the app beneath. The pointer on the shape, or
/// resting on the top edge above it (GotMail 0.9.5), grows the shape; leaving it shrinks it back.
@MainActor
final class NotchPointer {
    private var monitors: [Any] = []
    private weak var panel: NotchPanel?
    /// Only a crossing changes the stage: a pointer moving elsewhere never shrinks a mail that just dropped.
    private var over = false
    /// The beat behind the monitors (owner's fork, NotchPanel.swift "the poll behind the monitors", study #5):
    /// a PARKED pointer sends no events, and the shape grows and shrinks under it on every stage change, so
    /// without it the notch keeps a hover that stopped being true until somebody jogs the mouse. It calls the
    /// same `moved`, behind the same early-outs: one decision, not a second copy of it.
    private var poll: Timer?
    /// A pointer a drive can park (owner's fork, `pointerPark`): the hover rules read the machine's one cursor,
    /// and a check may not move the owner's mouse. Set, it stands in for the cursor read and nothing else: the
    /// same poll, the same monitors, the same rect tests. Cleared by the rig's reset.
    var park: CGPoint?
    var cursor: CGPoint { park ?? NSEvent.mouseLocation }
    private var lastCheck = Date.distantPast

    func start(panel: NotchPanel) {
        self.panel = panel
        let events: NSEvent.EventTypeMask = [.mouseMoved, .leftMouseDragged]
        if let g = NSEvent.addGlobalMonitorForEvents(matching: events, handler: { _ in
            MainActor.assumeIsolated { let p = NotchPanel.shared.pointer; p.moved(p.cursor) }
        }) { monitors.append(g) }
        if let l = NSEvent.addLocalMonitorForEvents(matching: events, handler: { e in
            MainActor.assumeIsolated { let p = NotchPanel.shared.pointer; p.moved(p.cursor) }
            return e
        }) { monitors.append(l) }
        let t = Timer(timeInterval: Double(WhooshConstants.NOTCH_POINTER_POLL_MS) / 1000, repeats: true) { _ in
            MainActor.assumeIsolated { let p = NotchPanel.shared.pointer; p.moved(p.cursor) }
        }
        RunLoop.main.add(t, forMode: .common)   // `.common`: a menu or a tracking loop does not stop it
        poll = t
    }

    /// The shape's rectangle on the screen, from its laid-out size: centred on the housing, hanging from the top.
    var shapeRect: CGRect {
        guard let screen = NotchGeometry.screen, let p = panel else { return .zero }
        let s = NotchModel.shared.shapeSize
        return CGRect(x: p.geometry.housing.midX - s.width / 2, y: screen.frame.maxY - s.height, width: s.width, height: s.height)
    }

    /// One pointer position. A top-edge rest counts within the card's width of the housing's centre.
    func moved(_ p: CGPoint, force: Bool = false) {
        let now = Date()
        guard force || now.timeIntervalSince(lastCheck) * 1000 >= Double(WhooshConstants.NOTCH_POINTER_MIN_MS) else { return }
        lastCheck = now
        // a drag owns the pointer: it is moving the notch, not hovering it
        guard !NotchDrag.shared.dragging, let screen = NotchGeometry.screen, let panel else { return }
        NotchTwitch.note("pointer", String(format: "%.0f,%.0f", p.x, p.y))
        let rect = shapeRect
        let inside = rect.insetBy(dx: -1, dy: -1).contains(p)
        let reach = max(rect.width, NotchMetrics.cardWidth) / 2
        let atEdge = p.y >= screen.frame.maxY - 2 && abs(p.x - panel.geometry.housing.midX) <= reach
        panel.window?.ignoresMouseEvents = !inside
        let hit = inside || atEdge
        guard hit != over else { return }
        over = hit
        if hit { NotchFocus.refresh() }
        NotchModel.shared.hover(hit)
    }

    /// Parks the pointer at the screen's left edge, far from the shape, and settles the hover state there. A drive's reset and
    /// leave use it: a cleared park would read the owner's real cursor, which sits wherever the owner left it, and a mail that
    /// dropped under it would grow into the list on its own (round 7: J6.13, J6.17, J6.37 read `list` for `drop`).
    func parkAway() {
        guard let s = NotchGeometry.screen else { park = nil; return }
        let at = CGPoint(x: s.frame.minX + WhooshSpacing.s4, y: s.frame.midY)
        park = at
        moved(at, force: true)
    }

    /// After a change: re-test the pointer where it is, so a grown or shrunk shape takes or drops it at once.
    func refresh() {
        panel?.window?.ignoresMouseEvents = !shapeRect.insetBy(dx: -1, dy: -1).contains(cursor)
    }
}
#endif
