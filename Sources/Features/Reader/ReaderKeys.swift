import SwiftUI

/// The reader's keys (round 5): the stub ids Commands.swift hands to this lane, provided through CommandProviding,
/// plus the live ids the reader answers when a thread is on screen (delete, and Enter on a link ring). Every handler
/// returns false when it has nothing to do in the current state, so the key passes on. Each runs on the key's frame.
@MainActor
enum ReaderKeys {
    /// The open thread without its body (the keys need counts and ids only); nil when the reader is not open or a
    /// quick-reply field holds the keys.
    static func thread(_ m: AppModel) -> ReaderThread? {
        guard isOpen(m) else { return nil }
        return ReaderFocus.shared.thread(m, resting: true)
    }

    /// The reader holds the keys: a thread is open and no quick-reply field has them. Reads no core state, so space,
    /// ⇧space and ⌘↑ ⌘↓ (held down, they repeat) build no thread.
    static func isOpen(_ m: AppModel) -> Bool {
        MessageBodyFeature.readerHoldsKeys(m) && !QuickReplyModel.shared.isOpen
    }

    /// Esc, first in the chain (the command `back`): keeps the pane focus in step with the state, then passes the key on, so the list's Esc still closes the mail.
    /// The confirm to unsubscribe takes it first. In READER (no reply field) the pane goes to the list; in REPLY (the field holds the keys) Esc ends in READER, so the pane is the reader's.
    /// Reads and writes `focusedPane` only; the bar, ⇧I and O read `MessageBodyFeature.readerHoldsKeys`, never the pane.
    static func back(_ m: AppModel) -> Bool {
        if ReaderListActions.cancelUnsubscribe(m) { return true }
        if backToResults(m) { return true }
        if MacShellState.shared.backToList(m) { return true }
        let shell = MacShellState.shared
        guard m.overlay == nil, m.surface == ReaderState.surface || m.surface == SurfaceSlot.macMain.id else { return false }
        if QuickReplyModel.shared.isOpen {
            if MessageBodyFeature.readerHoldsKeys(m), shell.focusedPane != .reader { shell.focusedPane = .reader }
        } else if shell.focusedPane == .reader, !shell.listCollapsed {
            shell.focusedPane = .list
        }
        return false
    }

    /// A mail the search opened (Enter on a result, A148) is on screen with the results kept in the list column.
    static func readsAResult(_ m: AppModel) -> Bool {
        m.overlay == nil && m.surface == ReaderState.surface && SearchMacModel.shared.showsResults && !QuickReplyModel.shared.isOpen
    }

    /// Esc from a mail the search opened: back to the results, the same result selected, the field as it was (its words, holding the keys).
    /// A second Esc closes the search as before (the search screen's own Esc). A pending unsubscribe confirm takes Esc first.
    static func backToResults(_ m: AppModel) -> Bool {
        guard readsAResult(m), SearchMacModel.shared.returnToSearch, ReaderState.shared.unsubscribe == nil else { return false }
        let shell = MacShellState.shared
        if !shell.listCollapsed { shell.focusedPane = .list }
        return SearchMacModel.shared.open(returning: true)
    }

    /// J, K and the arrows while the reader shows a mail the search opened: the selection moves through the RESULTS (the list the column
    /// shows), as the same keys move through the inbox when no search is open. The reader follows by the light-focus rule (ReaderFocus).
    static func stepResults(_ m: AppModel, _ delta: Int) -> Bool {
        readsAResult(m) && SearchMacModel.shared.move(delta)
    }

    /// The core's id of the message at `index` of the thread; nil where the core did not answer for it.
    static func messageId(_ t: ReaderThread, _ index: Int) -> String? {
        index == t.earlierCount ? t.messageId : (index < t.earlier.count ? t.earlier[index]?.id : nil)
    }

    private static func attachments(_ t: ReaderThread, _ index: Int) -> [String] {
        index == t.earlierCount ? t.attachmentNames : (index < t.earlier.count ? t.earlier[index]?.attachments ?? [] : [])
    }

    /// N and P: the mark moves one message; an earlier message unfolds the rule so the mark is seen. At the first or
    /// last message the key is taken and nothing moves: a refused P would fall through to the root's legacy P (pin).
    static func step(_ m: AppModel, _ delta: Int) -> Bool {
        guard let t = thread(m) else { return false }
        let state = ReaderState.shared
        let to = state.focused(t) + delta
        guard (0...t.earlierCount).contains(to) else { return true }
        state.focusedIndex = to
        if to < t.earlierCount { state.earlierOpenFor = t.threadId }
        ReaderLinks.shared.reset()
        ReaderScroll.shared.show(ReaderScroll.blockId(to))
        return true
    }

    /// The preset's `expand-message` key (O in Superhuman). On the open message it is the three dots of the mail view when the mail has a fold
    /// (A129f: the action follows the preset's key, never a literal chord); on a marked earlier message it shows whole or folds back to its first
    /// words. A mail without a fold, or an open message with nothing to expand, passes the key on.
    static func expand(_ m: AppModel) -> Bool {
        guard let t = thread(m) else { return false }
        let state = ReaderState.shared
        let i = state.focused(t)
        if i >= t.earlierCount { return MessageBodyProbe.shared.current?.toggleFold() ?? false }
        if state.expanded.contains(i) { state.expanded.remove(i) } else { state.expanded.insert(i); state.earlierOpenFor = t.threadId }
        state.focusedIndex = i
        return true
    }

    /// ⇧O (round 10, A70): every earlier message whole and every header open; when every header already is open, every
    /// message folds back and every header closes. A one-message thread has no earlier messages but its header still answers.
    static func expandAll(_ m: AppModel) -> Bool {
        guard let t = thread(m) else { return false }
        let state = ReaderState.shared
        let ids = (0...t.earlierCount).compactMap { messageId(t, $0) }
        guard t.earlierCount > 0 || !ids.isEmpty else { return false }
        let close = state.allHeadersOpen(ids)
        withAnimation(ReaderHeaderMotion.keyAnimation) {
            state.setHeaders(ids, open: !close)
            guard t.earlierCount > 0 else { return }
            if close { state.expanded = []; state.earlierOpenFor = nil } else { state.expanded = Set(0..<t.earlierCount); state.earlierOpenFor = t.threadId }
        }
        return true
    }

    /// Tab: the ring goes to the next link or date of the marked message. False when it has none (Tab then moves the split).
    static func cycleLinks(_ m: AppModel) -> Bool {
        guard let t = thread(m), let id = messageId(t, ReaderState.shared.focused(t)) else { return false }
        return ReaderLinks.shared.cycle(m, messageId: id)
    }

    /// ⇧Tab: the ring goes back one link or date of the marked message. The preset's ⇧Tab is "previous split" (a fixed key),
    /// so the reader answers that command while a thread with links is open; with none it passes on and the split moves.
    static func cycleLinksBack(_ m: AppModel) -> Bool {
        guard let t = thread(m), let id = messageId(t, ReaderState.shared.focused(t)) else { return false }
        return ReaderLinks.shared.cycleBack(m, messageId: id)
    }

    /// ⌘O: the marked message's first link in the browser, its attachments to Quick Look.
    static func openLinks(_ m: AppModel) -> Bool {
        guard let t = thread(m) else { return false }
        let i = ReaderState.shared.focused(t)
        guard let id = messageId(t, i) else { return false }
        let names = attachments(t, i)
        let opened = ReaderLinks.shared.openAll(m, messageId: id, attachments: names)
        #if os(macOS)
        // The reader answers first (focus order), so it hands the open message's files to the chips' own opener:
        // with no chip selected that selects and opens the first attachment.
        if handsToAttachments(names: names, index: i, openIndex: t.earlierCount), AttachmentKeys.openLinks(m) { return true }
        #endif
        return opened
    }

    /// ⌘O reaches the attachment chips only for the open message (the chips belong to it) and only when it has files.
    static func handsToAttachments(names: [String], index: Int, openIndex: Int) -> Bool { !names.isEmpty && index == openIndex }

    /// Enter reaches the engine's "reply-all" when a thread is open. With the ring on a link, Enter opens that link
    /// instead; a click on the Reply all button is not Enter and always replies.
    static func enterOnLink(_ m: AppModel) -> Bool {
        guard ShortcutEngine.current.resolved == "reply-all", ReaderLinks.shared.cursor != nil else { return false }
        return ReaderLinks.shared.openCursor(m)
    }

    static func scroll(_ direction: Double) -> (AppModel) -> Bool {
        { m in isOpen(m) && ReaderScroll.shared.page(direction) }
    }

    static func jump(top: Bool) -> (AppModel) -> Bool {
        { m in isOpen(m) && ReaderScroll.shared.scroll(to: top ? 0 : ReaderScroll.shared.maxOffset) }
    }
}

/// The ids Commands.swift lists as live reader commands, plus ⇧Tab (answers "previous split" while a thread with links is open), delete, the Enter-on-a-link door, and round 6's ⇧H
/// (toggle-message-header) and ⌘U (unsubscribe, through a confirm). ⇧E is the inbox list's (one owner per command).
extension ReaderFeature: CommandProviding {
    public static func commands() -> [String: (AppModel) -> Bool] {
        [
            "next-message": { ReaderKeys.step($0, 1) },
            "previous-message": { ReaderKeys.step($0, -1) },
            "expand-message": ReaderKeys.expand,
            "expand-all-messages": ReaderKeys.expandAll,
            "open-links": ReaderKeys.openLinks,
            "cycle-links": ReaderKeys.cycleLinks,
            "cycle-links-back": ReaderKeys.cycleLinksBack,   // its own id (keys-r5: it sat under previous-split, the tab switch, so Shift+Tab in the reader changed tabs)
            "scroll-down": ReaderKeys.scroll(1),
            "scroll-up": ReaderKeys.scroll(-1),
            "jump-top": ReaderKeys.jump(top: true),
            "jump-bottom": ReaderKeys.jump(top: false),
            "reply-all": { ReaderListActions.confirmUnsubscribe($0) || ReaderKeys.enterOnLink($0) },
            "back": ReaderKeys.back,
            // J, K and the arrows on a mail the search opened walk the results (A148); false otherwise, so the list's own move answers
            "next": { ReaderKeys.stepResults($0, 1) },
            "previous": { ReaderKeys.stepResults($0, -1) },
            ReaderWords.headerCommand: ReaderListActions.toggleHeader,
            "unsubscribe": ReaderListActions.unsubscribe,
            "delete": ReaderActions.deleteFocused,
        ]
    }
}
