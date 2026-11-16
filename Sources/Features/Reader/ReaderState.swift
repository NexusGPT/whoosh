import Foundation
import Observation

/// The reader's own UI state. Which thread is open comes from the app model (surface "mac-reader") and which message
/// the pane shows from `ReaderFocus`; this holds only what the reader decides: whether the earlier messages are
/// unfolded, which message the N and P keys stand on, and which earlier messages are expanded (O and ⇧O).
@MainActor @Observable
final class ReaderState {
    static let shared = ReaderState()
    static let surface = "mac-reader"

    /// thread id whose earlier messages the person unfolded; another thread opens folded again
    var earlierOpenFor: String?

    /// message id whose sign-in code was copied: the chip says "Copied" for that message only
    var copiedFor: String?
    /// What the empty reader says after a delete
    var deletedNote: String?
    /// The pointer is on the open message's header: the hover strip shows (it also shows while the pane holds the keys).
    var headerHover = false
    /// The message N and P stand on, as an index in the thread; nil until a key moved it (then the open message is it).
    var focusedIndex: Int?
    /// Earlier messages shown whole (O, ⇧O); the rest show their first words.
    var expanded: Set<Int> = []

    /// SPEC A70: the messages whose header is expanded, for the thread `headerThread`; another thread starts collapsed.
    var headerThread: String?
    var headerOpen: Set<String> = []
    /// ⌘U waits for a confirm: the message and the method the core named (in-pane, never a system alert).
    var unsubscribe: (messageId: String, method: UnsubscribeMethod)?
    /// What the pane says after ⌘U or ⇧E ran.
    var actionNote: String?

    func headerIsOpen(_ messageId: String) -> Bool {
        headerOpen.contains(messageId) && headerThread == shownThread
    }

    /// The thread of the message the pane shows (the fixture keys some rows' thread by message id).
    private var shownThread: String? {
        guard let id = ReaderFocus.shared.shownId else { return nil }
        return AppModel.shared.knownRow(id)?.threadId ?? id   // a search result outside the inbox rows has its own thread (A148)
    }

    func toggleHeader(_ messageId: String) {
        let thread = shownThread
        if headerThread != thread { headerThread = thread; headerOpen = [] }
        if headerOpen.contains(messageId) { headerOpen.remove(messageId) } else { headerOpen.insert(messageId) }
    }

    /// ⇧O: every header of the thread opens, or (when they all are open) closes. Ids the core did not answer for are left out.
    func setHeaders(_ ids: [String], open: Bool) {
        let thread = shownThread
        if headerThread != thread { headerThread = thread; headerOpen = [] }
        if open { headerOpen.formUnion(ids) } else { headerOpen.subtract(ids) }
    }

    func allHeadersOpen(_ ids: [String]) -> Bool { !ids.isEmpty && ids.allSatisfy { headerIsOpen($0) } }

    func earlierOpen(_ t: ReaderThread) -> Bool { earlierOpenFor == t.threadId }

    /// What the pane draws: the earlier messages are shown when the person unfolded them OR an earlier message holds the selected result's passage (A152: the
    /// reader opens at the mail that holds it). The others keep their own state.
    func earlierShown(_ t: ReaderThread) -> Bool { earlierOpen(t) || ReaderFocus.shared.passage?.earlierIndex(in: t) != nil }

    /// The earlier messages drawn whole: the ones O or ⇧O opened, and the one that holds the passage.
    func expandedShown(_ t: ReaderThread) -> Set<Int> {
        guard let i = ReaderFocus.shared.passage?.earlierIndex(in: t) else { return expanded }
        return expanded.union([i])
    }

    func toggleEarlier(_ t: ReaderThread) {
        earlierOpenFor = earlierOpen(t) ? nil : t.threadId
    }

    /// The index N and P act from: the marked message, else the open one (the last).
    func focused(_ t: ReaderThread) -> Int { min(focusedIndex ?? t.earlierCount, t.earlierCount) }

    /// A new message on screen starts without marks, expansions or a link ring.
    /// Written only when it changes (perf-archive r3): an equal write still marks the pane changed, a second draw after an open.
    func resetNavigation() {
        if focusedIndex != nil { focusedIndex = nil }
        if !expanded.isEmpty { expanded = [] }
    }

    /// The open thread, or nil when the reader shows its empty state.
    static func open(_ m: AppModel) -> ReaderThread? {
        m.surface == surface ? ReaderFocus.shared.thread(m, resting: false) : nil
    }
}
