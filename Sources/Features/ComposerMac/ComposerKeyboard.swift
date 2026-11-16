import Foundation

#if os(macOS)
import AppKit

/// Two facts of the composer window that AppKit and the shell need from the composer: the words of its title bar and the header field its ring is on.
/// Lead, 13 Nov 2026 (pictures and a real-key probe of a forward on main155): the title bar read "New message" over the heading "Forward", and a
/// letter typed right after F joined the END of the forwarded text while the ring was on To.
@MainActor
enum ComposerKeyboard {
    private static var installed = false

    static func installOnce() {
        guard !installed else { return }
        installed = true
        WindowBridge.liveTitles[ComposerIds.surface] = { ComposerWords.title(ComposerModel.shared.kind) }
        ChordKit.composerField = { w in field(in: w) }
        WindowBridge.openingHeights[ComposerIds.surface] = ComposerOpenMotion.idealHeight
    }

    /// The header field under the ring (To, Cc, Bcc, Subject) as AppKit has it: the window's editable fields, from the top down, are the header's
    /// visible fields in their order. Nil when the ring is on the message or on From, and when the window holds any other field (an open panel):
    /// the message text's own rule then stands.
    static func field(in w: NSWindow) -> NSTextField? {
        let m = ComposerModel.shared
        let rows: [ComposerField] = [.to] + (m.showCc ? [.cc] : []) + (m.showBcc ? [.bcc] : []) + [.subject]
        guard m.isOpen, let ring = m.ringField, let at = rows.firstIndex(of: ring) else { return nil }
        var found: [NSTextField] = []
        func walk(_ v: NSView) {
            if let t = v as? NSTextField, t.isEditable, !t.isHiddenOrHasHiddenAncestor { found.append(t) }
            for s in v.subviews { walk(s) }
        }
        if let root = w.contentView { walk(root) }
        guard found.count == rows.count else { return nil }
        found.sort { $0.convert($0.bounds, to: nil).midY > $1.convert($1.bounds, to: nil).midY }
        return found[at]
    }
}
#endif
