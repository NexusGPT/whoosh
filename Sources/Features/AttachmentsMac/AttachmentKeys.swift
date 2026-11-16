#if os(macOS)
import AppKit

/// The keys of a selected chip, on the app's one key path (ChordKit → AppModel.press → the registry), so the socket's
/// `key` and the real keyboard do the same thing. A chip holds the keys only while it is selected (a click selects
/// it, as in the Finder) or while Quick Look is open; otherwise every key passes on untouched.
@MainActor
enum AttachmentKeys {
    static var store: AttachmentStore { .shared }
    static var ql: QuickLookController { .shared }

    /// Selects a chip on the input frame; an open Quick Look follows it.
    static func select(_ ref: AttachmentRef) {
        store.selected = ref
        _ = store.phase(ref)
        ql.follow(ref)
    }

    static func holdsKeys(_ m: AppModel) -> Bool { ql.isOpen || store.liveSelection(m) != nil }

    /// Space: Quick Look opens over the selected chip, or closes.
    static func space(_ m: AppModel) -> Bool {
        if ql.isOpen { return ql.close() }
        if let s = store.liveSelection(m) { return ql.show(s) }
        // No chip selected: the files' header draws the Space key beside "Quick Look" for the whole set, so Space opens the open mail's first
        // file, as ⌘O does (lead, 13 Nov, real-key audit on main118: Space did nothing while its key was drawn on screen). A text that holds
        // the keyboard takes its own Space before this is ever asked.
        // The reader's page-down comes first (this binding is asked before the preset's): a mail that holds a file still scrolls by Space, and the first
        // file is previewed only where the thread cannot scroll further, its end or a mail that fits (lead, 13 Nov, par-reader on main147: from the top
        // of a thread with a file, Space opened the preview and the thread never moved).
        if ReaderKeys.scroll(1)(m) { return true }
        guard let first = store.readerRefs(m).first, ReaderState.open(m) != nil else { return false }
        select(first)
        return ql.show(first)
    }

    /// Return: opens the selected chip in its default app (Return in the Finder renames; in Mail it opens).
    static func enter(_ m: AppModel) -> Bool {
        guard let s = ql.current ?? store.liveSelection(m) else { return false }
        return AttachmentActs.open(s)
    }

    /// ⌘O, Superhuman's "Open links & attachments": the selected chip, else the open message's first attachment.
    static func openLinks(_ m: AppModel) -> Bool {
        if let s = ql.current ?? store.liveSelection(m) { return AttachmentActs.open(s) }
        guard let first = store.readerRefs(m).first, ReaderState.open(m) != nil else { return false }
        select(first)
        return AttachmentActs.open(first)
    }

    /// ← →: steps Quick Look, or moves the selection to the neighbouring chip.
    static func arrow(_ delta: Int, _ m: AppModel) -> Bool {
        if ql.isOpen { return ql.step(delta) }
        guard let s = store.liveSelection(m) else { return false }
        let all = store.siblings(m)
        guard let i = all.firstIndex(of: s), all.indices.contains(i + delta) else { return false }
        select(all[i + delta])
        return true
    }

    /// Escape: closes Quick Look first, then lets go of the selection.
    static func escape(_ m: AppModel) -> Bool {
        if ql.isOpen { return ql.close() }
        guard store.liveSelection(m) != nil else { return false }
        store.selected = nil
        return true
    }

    /// ⌘C on a selected chip copies the file, as in the Finder; with no chip selected the key passes on.
    static func copy(_ m: AppModel) -> Bool {
        guard let s = ql.current ?? store.liveSelection(m) else { return false }
        return AttachmentActs.copy(s)
    }

    static var bindings: [String: (AppModel) -> Bool] {
        ["Space": space, "Enter": enter, "Escape": escape, "Left": { arrow(-1, $0) }, "Right": { arrow(1, $0) },
         "⌘C": copy, "⌘V": AttachmentIntake.paste]
    }
}
#endif
