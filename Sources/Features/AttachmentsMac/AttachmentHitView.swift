#if os(macOS)
import AppKit
import SwiftUI
import UniformTypeIdentifiers

/// The chip's input layer, in AppKit so it behaves as a Finder file: a click selects on the press (no wait for a
/// possible double-click), a double-click opens, a drag carries the file out (the file URL, or a file promise when
/// the bytes are not here yet) with the system's drag image, and a right-click shows the attachment menu.
struct AttachmentHitLayer: NSViewRepresentable {
    let ref: AttachmentRef
    func makeNSView(context: Context) -> AttachmentHitView { AttachmentHitView(ref: ref) }
    func updateNSView(_ v: AttachmentHitView, context: Context) { v.update(ref) }
    static func dismantleNSView(_ v: AttachmentHitView, coordinator: ()) { AttachmentChipFrames.unregister(v.ref, v) }
}

final class AttachmentHitView: NSView, NSDraggingSource {
    private(set) var ref: AttachmentRef
    private var down: NSEvent?
    private var tracking: NSTrackingArea?
    private var store: AttachmentStore { .shared }
    /// Pointer travel before a press becomes a drag (docs/CONSTANTS.md).
    static let dragStart = CGFloat(WhooshConstants.ATTACHMENT_DRAG_START_PT)

    init(ref: AttachmentRef) {
        self.ref = ref
        super.init(frame: .zero)
        setAccessibilityElement(true)
        setAccessibilityRole(.button)
        update(ref)
    }
    required init?(coder: NSCoder) { nil }

    func update(_ r: AttachmentRef) {
        if r != ref { AttachmentChipFrames.unregister(ref, self) }
        ref = r
        AttachmentChipFrames.register(r, self)
        toolTip = AttachmentRig.mutant("no-keys") ? r.name : AttachmentWords.tooltip(r.name)
        setAccessibilityLabel(AttachmentWords.accessibility(r.name, r.kind.rawValue))
    }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let t = tracking { removeTrackingArea(t) }
        let t = NSTrackingArea(rect: bounds, options: [.mouseEnteredAndExited, .activeInKeyWindow, .inVisibleRect], owner: self)
        addTrackingArea(t)
        tracking = t
    }
    override func mouseEntered(with event: NSEvent) { store.hovered = ref }
    override func mouseExited(with event: NSEvent) { if store.hovered == ref { store.hovered = nil } }

    override func mouseDown(with event: NSEvent) {
        down = event
        store.pressed = ref
        AttachmentKeys.select(ref)
        if event.clickCount == 2 { _ = AttachmentActs.open(ref) }
    }
    override func mouseUp(with event: NSEvent) { store.pressed = nil; down = nil }

    override func mouseDragged(with event: NSEvent) {
        guard let d = down, hypot(event.locationInWindow.x - d.locationInWindow.x, event.locationInWindow.y - d.locationInWindow.y) >= Self.dragStart else { return }
        down = nil
        store.pressed = nil
        guard let item = AttachmentDrag.item(ref, frame: bounds) else { return }
        beginDraggingSession(with: [item], event: d, source: self)
    }

    func draggingSession(_ session: NSDraggingSession, sourceOperationMaskFor context: NSDraggingContext) -> NSDragOperation { .copy }

    override func menu(for event: NSEvent) -> NSMenu? {
        AttachmentKeys.select(ref)
        return AttachmentMenu.make(ref)
    }
}
#endif
