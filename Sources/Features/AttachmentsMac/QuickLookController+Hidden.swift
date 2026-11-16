#if os(macOS)
import AppKit
import Quartz

/// The hidden rig's Quick Look (owner 16:30: "stop the preview opening EVER"): the system panel is never created or
/// shown; the controller keeps what it would show (the items, the index) and the socket reads it from
/// `mac-main.attachments.panel`. Keys, the selection, steps and the wait for a download behave as on a visible copy.
extension QuickLookController {
    func showHidden(_ ref: AttachmentRef) -> Bool {
        hiddenIndex = items.firstIndex(of: ref) ?? 0
        return true
    }

    func closeHidden() -> Bool {
        guard hiddenIndex != nil else { return false }
        hiddenIndex = nil
        return true
    }

    func stepHidden(_ delta: Int) -> Bool {
        guard let i = hiddenIndex, items.indices.contains(i + delta) else { return false }
        hiddenIndex = i + delta
        AttachmentStore.shared.selected = items[i + delta]
        _ = AttachmentStore.shared.phase(items[i + delta])
        return true
    }

    /// A reopen during the panel's own zoom-out can be swallowed by it: look again a few frames later and bring the
    /// panel forward if it is gone while it should be shown (the state already changed on the input frame).
    func settleFront(_ p: QLPreviewPanel, tries: Int) {
        guard tries > 0 else { return }
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(WhooshConstants.POPOVER_MS))
            guard shown else { return }
            if !p.isVisible { p.makeKeyAndOrderFront(nil) }
            settleFront(p, tries: tries - 1)
        }
    }
}
#endif
