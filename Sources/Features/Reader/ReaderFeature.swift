import SwiftUI

/// The reader's registration (folder Sources/Features/Reader/): its rows, clicks, keys and the pane it claims in
/// the shell's reader slot. The reader opens while the surface is "mac-reader", on the focused row.
/// Enter opens it by focus: while the reader pane holds the focus (`hasFocus`) the reader's Enter is asked first.
/// From the list, Enter goes through the command table ("open"), whose owner decides (shell round 5).
/// The engine commands the reader provides (next-message and the rest) are in ReaderKeys.swift.
@MainActor
public enum ReaderFeature: FeatureModule {
    public static let id = "reader"
    public static let surfaces: [String] = ["mac-main"]

    public static func view(for slot: SurfaceSlot) -> AnyView? {
        slot == .macReader ? AnyView(ReaderPane()) : nil
    }

    /// The reader pane holds the focus, unless the composer holds the keyboard (its pop-out window is key, or its surface is up): the composer's
    /// Enter (the discard confirmation) and Escape then come first, whatever pane the last click left focused (main80: Enter after a discard opened a reply).
    /// Nor while the reply field under the thread holds the keys (open and focused): the field's Escape (close the reply) and Return come first, so Escape
    /// does not leave for the search results with the typed reply still open behind them. Features are asked in registry order and the reader comes first.
    /// Nor while a file of the mail holds the keys (a selected chip, or Quick Look open): its Escape (close the preview, then let go of the file) and
    /// its Return (open the file) come first. Lead, 13 Nov: with the pane focused the reader took both, so Escape left the mail with the preview
    /// still open, and Return opened a reply over a selected file.
    public static func hasFocus(_ m: AppModel) -> Bool {
        // a file of the mail holding the keys is a Mac thing (AttachmentsMac): the iPhone build has no such type (it did not compile here, 15 Nov)
        #if os(macOS)
        let fileHoldsKeys = AttachmentKeys.holdsKeys(m)
        #else
        let fileHoldsKeys = false
        #endif
        return MacShellState.shared.focusedPane == .reader && !ComposerMacFeature.hasFocus(m)
            && !(QuickReplyModel.shared.isOpen && QuickReplyModel.shared.fieldFocused)
            && !fileHoldsKeys
    }

    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        // The real ⌫ key's chord is "Backspace" (ChordKit names key 51; before 13 Nov it was the character "\u{7F}", kept here for a layout that sends it).
        // Escape: with the reader pane focused on a mail the search opened, back to the results (A148) in every preset, also those with no
        // Escape → back binding (Gmail, Apple Mail); false otherwise, so the key goes on as before
        // A pending unsubscribe confirm takes Escape first (its Cancel shows Esc): without this the pane left for the list with the confirm still armed.
        ["Enter": openAndReply, "Escape": { m in
            if ReaderListActions.cancelUnsubscribe(m) { return true }
            QuickReplyModel.shared.foldIfIdle(m)
            return ReaderKeys.backToResults(m) || MacShellState.shared.backToList(m)
        }]
    }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        ReaderProbe.rowSources().merging(ReaderHeaderProbe.rowSources()) { a, _ in a }
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        var out: [String: (AppModel) -> Bool] = [
            "mac-main.reader.perf.start": { _ in ReaderPerf.shared.start(); return true },
            "mac-main.reader.perf.stop": { _ in ReaderPerf.shared.stop(); return true },
            "mac-main.reader.open": openFocused,
            "mac-main.reader.earlier": { m in
                guard let t = ReaderState.open(m), t.earlierCount > 0 else { return false }
                ReaderState.shared.toggleEarlier(t)
                return true
            },
            "mac-main.reader.sender": ReaderActions.openSender,
            "mac-main.reader.codepill": ReaderActions.copyCode,
            "mac-main.reader.delete": ReaderActions.deleteOpen,
            "mac-main.reader.reply": { m in m.surface == ReaderState.surface && ReaderActions.run("reply", m) },
        ]
        // the strip on the message header and the icons in the header band: each runs its engine command
        for item in ReaderStripItem.all {
            out["mac-main.reader.strip.\(item.id)"] = { m in
                item.enabled(quickOpen: QuickReplyModel.shared.isOpen) && ReaderActions.run(item.command, m)
            }
        }
        for a in ReaderAction.all { out["mac-main.reader.key.\(a.id)"] = { m in ReaderActions.run(a.command, m) } }
        // SPEC A70: each message's header chevron (index in the thread), the marked one, and round 6's doors
        // the chevron (header.<i>) and the line itself (header.line.<i>, the .onTapGesture on the header row) run the same toggle
        for i in 0..<16 {
            let toggle: (AppModel) -> Bool = { m in
                guard let t = ReaderKeys.thread(m), let id = ReaderKeys.messageId(t, i) else { return false }
                ReaderState.shared.toggleHeader(id)
                return true
            }
            out["mac-main.reader.header.\(i)"] = toggle
            out["mac-main.reader.header.line.\(i)"] = toggle
        }
        out["mac-main.reader.header"] = ReaderListActions.toggleHeader
        out["mac-main.reader.unsubscribe.ask"] = ReaderListActions.unsubscribe
        out["mac-main.reader.unsubscribe.confirm"] = ReaderListActions.confirmUnsubscribe
        out["mac-main.reader.unsubscribe.cancel"] = ReaderListActions.cancelUnsubscribe
        // the ring row's chips (the first eight; a longer list is reached with Tab)
        for i in 0..<8 { out["mac-main.reader.link.\(i)"] = { m in ReaderLinks.shared.openCursor(m, at: i) } }
        return out
    }

    public static func paletteCommands() -> [(title: String, key: String)] {
        [(Words.text("Show earlier messages"), "")]
    }

    /// Enter while the reader pane holds the focus on a row whose thread is not the one shown: the same act as R (A102), the
    /// thread opens and the quick reply under it takes the cursor. False when that thread is on screen already (the engine's
    /// Enter then replies all, Superhuman's meaning).
    static func openAndReply(_ m: AppModel) -> Bool {
        guard !ComposerMacFeature.hasFocus(m), let row = m.shownRow else { return false }
        if m.surface == ReaderState.surface && ReaderFocus.shared.target(m) == row.messageId { return false }
        return QuickReplyModel.shared.open(m)
    }

    /// Opens the focused row's thread in the reader, and the pane follows at once (an open is not a passing focus).
    /// False (the key passes on) when no row is focused or the reader already shows that thread.
    static func openFocused(_ m: AppModel) -> Bool {
        guard let row = m.shownRow else { return false }
        if m.surface == ReaderState.surface && ReaderFocus.shared.target(m) == row.messageId { return false }
        guard m.perform(.openThread(threadId: row.threadId)) else { return false }
        ReaderState.shared.deletedNote = nil
        ReaderFocus.shared.settleForOpen(m)
        m.surface = ReaderState.surface
        MacShellState.shared.focusedPane = .reader   // the door enters READER whole: the pane focus follows the state
        return true
    }
}
