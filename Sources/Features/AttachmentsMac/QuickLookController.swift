#if os(macOS)
import AppKit
import Quartz

/// The system Quick Look panel over the attachments of the selected chip's message: it zooms from the chip (the
/// delegate returns the chip's frame on screen and its thumbnail), ← → step through the message's attachments, space
/// or Esc close it, and it follows the selection to another chip. A file still downloading shows its title with the
/// progress until its bytes land, then the panel refreshes in place.
@MainActor
final class QuickLookController: NSResponder {
    static let shared = QuickLookController()
    private(set) var items: [AttachmentRef] = []
    private var installedIn: NSWindow?

    override init() { super.init() }
    required init?(coder: NSCoder) { nil }

    /// The system panel; never created on a hidden rig copy (QuickLookController+Hidden.swift keeps its state instead).
    var panel: QLPreviewPanel? { !HiddenRig.isOn && QLPreviewPanel.sharedPreviewPanelExists() ? QLPreviewPanel.shared() : nil }
    /// Hidden rig: the item the panel would show, nil when it would be closed.
    var hiddenIndex: Int?
    /// Open as far as the keys are concerned: from the moment this controller shows the panel until it closes it or
    /// the panel's own window closes (its close button). Neither the zoom-in nor the zoom-out makes a key wait or slip.
    var shown = false
    /// Closes this controller asked for: the panel's close notice for each arrives late (after its zoom-out) and
    /// must not undo a reopen that happened meanwhile.
    var ownCloses = 0
    var isOpen: Bool { HiddenRig.isOn ? hiddenIndex != nil : shown && panel != nil }
    var currentIndex: Int? { HiddenRig.isOn ? hiddenIndex : (isOpen ? panel?.currentPreviewItemIndex : nil) }
    var current: AttachmentRef? {
        guard let i = currentIndex, items.indices.contains(i) else { return nil }
        return items[i]
    }

    /// Opens the panel on `ref` (or moves it there when open). Waits for a file still downloading.
    func show(_ ref: AttachmentRef) -> Bool {
        if AttachmentRig.mutant("no-panel") { return true }
        let store = AttachmentStore.shared
        if store.peek(ref).url == nil && !AttachmentRig.mutant("no-wait") { return store.whenReady(ref, .quickLook) }
        items = store.siblings()
        if !items.contains(ref) { items = [ref] }
        store.selected = ref
        if HiddenRig.isOn { return showHidden(ref) }
        guard let p = QLPreviewPanel.shared() else { return false }
        install()
        p.dataSource = self
        p.delegate = self
        p.reloadData()
        p.currentPreviewItemIndex = items.firstIndex(of: ref) ?? 0
        // a panel still zooming out from a close reads visible: bring it forward whenever this controller had closed it
        if !shown || !p.isVisible { p.makeKeyAndOrderFront(nil) }
        shown = true
        settleFront(p, tries: Int(WhooshSpacing.s4))
        return true
    }

    func close() -> Bool {
        if HiddenRig.isOn { return closeHidden() }
        guard isOpen, let p = panel else { return false }
        shown = false
        ownCloses += 1
        p.orderOut(nil)
        installedIn?.makeKey()                 // the keys go back to the window the panel was opened from
        return true
    }

    /// Space: open on the selected chip, or close.
    func toggle(_ ref: AttachmentRef) -> Bool { isOpen ? close() : show(ref) }

    /// ← → inside the panel: the previous or next attachment of the same message (stops at the ends, as the Finder).
    func step(_ delta: Int) -> Bool {
        if AttachmentRig.mutant("no-step") || (delta < 0 && AttachmentRig.mutant("no-step-back")) { return true }
        if HiddenRig.isOn { return stepHidden(delta) }
        guard isOpen, let p = panel else { return false }
        let n = p.currentPreviewItemIndex + delta
        guard items.indices.contains(n) else { return false }
        p.currentPreviewItemIndex = n
        AttachmentStore.shared.selected = items[n]
        _ = AttachmentStore.shared.phase(items[n])
        return true
    }

    /// The selection moved to another chip while the panel is open: the panel follows it.
    func follow(_ ref: AttachmentRef) {
        guard isOpen, !AttachmentRig.mutant("no-follow") else { return }
        _ = show(ref)
    }

    /// A file's bytes landed: redraw it when the panel shows it.
    func refresh(_ ref: AttachmentRef) {
        guard isOpen, let p = panel, items.contains(ref) else { return }
        let i = p.currentPreviewItemIndex
        p.reloadData()
        p.currentPreviewItemIndex = i
        p.refreshCurrentPreviewItem()
    }

    /// The panel looks for its controller in the responder chain of the key window: this object joins it once.
    private func install() {
        guard let w = (NSApp as NSApplication?)?.mainWindow ?? (NSApp as NSApplication?)?.keyWindow, installedIn !== w else { return }
        nextResponder = w.nextResponder
        w.nextResponder = self
        installedIn = w
    }

    nonisolated override func acceptsPreviewPanelControl(_ panel: QLPreviewPanel!) -> Bool { true }
    nonisolated override func beginPreviewPanelControl(_ panel: QLPreviewPanel!) {
        MainActor.assumeIsolated { panel.dataSource = self; panel.delegate = self }
    }
    nonisolated override func endPreviewPanelControl(_ panel: QLPreviewPanel!) {}
}
#endif
