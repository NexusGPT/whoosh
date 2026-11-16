import Foundation
import Observation
#if canImport(UIKit)
import UIKit
#elseif canImport(AppKit)
import AppKit
#endif

/// The thread screen's one state object. Mail data stays in the core and `AppModel`; this holds what the screen owns:
/// which message it is open on, which earlier messages are unfolded, the code pill's "Copied", the note sheet.
@MainActor @Observable
public final class ThreadPhoneState {
    public static let shared = ThreadPhoneState()
    /// `AppModel.overlay` while the thread is up (the inbox sets it on a row tap).
    public static let overlayId = "iphone-thread"
    static let inboxSurface = "iphone-inbox"

    /// The message the thread was opened on, frozen at open: a later refresh may re-order the inbox rows.
    public private(set) var messageId: String?
    /// The message whose sign-in code was copied: the pill says "Copied" for that message only.
    public internal(set) var copiedFor: String?
    /// Earlier messages the person unfolded, as their index in the thread.
    public var expanded: Set<Int> = []
    public var noteOpen = false
    /// The More panel above the bar.
    public var moreOpen = false
    public var noteDraft = ""
    /// A refused leave (archive or snooze) in words; the thread stays.
    public internal(set) var failure: String?
    /// The thread already marked read for this open (opening twice counts once).
    @ObservationIgnored private var openedFor: String?
    /// The earlier message that holds the passage a search result showed: `begin` unfolds it once, when the thread is on screen.
    @ObservationIgnored private(set) var pendingUnfold: String?

    private init() {}

    /// The message the screen shows now: the frozen one, else the inbox's focused row (a tap or a notification set it).
    public func target(_ m: AppModel) -> String? { messageId ?? m.focusedMessageId }

    func thread(_ m: AppModel) -> ReaderThread? {
        ReaderThread.make(m, messageId: target(m), withBody: false)
    }

    // MARK: open and close

    /// Opens the thread of the inbox row at `index` (what a tap on the row does).
    @discardableResult
    public func open(row index: Int, in m: AppModel) -> Bool {
        let rows = InboxPhoneState.shared.rows(m)
        guard rows.indices.contains(index) else { return false }
        reset()
        m.focus(messageId: rows[index].messageId)
        m.overlay = Self.overlayId
        m.surface = Self.overlayId
        begin(m)   // the open itself marks the thread read (J7.2): the screen's onAppear runs `begin` too, which does nothing twice (openedFor); a host with no screen drawn (the hidden test copy) still gets the open
        return true
    }

    /// Opens the thread of a mail by its id (a search result, which may be in no loaded inbox row), with the earlier message `hit` that holds the passage
    /// unfolded when the screen appears (item 9f990234, the Mac reader's rule: the thread opens at the mail that holds the passage). `hit` nil or the
    /// opened mail itself unfolds nothing. False, and nothing changes, when the window knows no row with that id.
    @discardableResult
    public func open(messageId id: String, unfolding hit: String?, in m: AppModel) -> Bool {
        guard m.knownRow(id) != nil else { return false }
        reset()
        messageId = id
        pendingUnfold = (hit == nil || hit == id) ? nil : hit
        m.overlay = Self.overlayId
        m.surface = Self.overlayId
        begin(m)   // as `open(row:)`: marks read and unfolds the passage's message at the open
        return true
    }

    /// The index, among the thread's earlier messages, of the message with this id; nil when it is not an earlier one.
    func earlierIndex(of id: String, in m: AppModel) -> Int? {
        guard let t = thread(m) else { return nil }
        return t.entries(earlierOpen: false).first { $0.index < t.earlierCount && $0.fact?.id == id }?.index
    }

    /// Called once when the screen appears: freezes the message and marks it read (the core's open-thread command).
    public func begin(_ m: AppModel) {
        if m.surface != Self.overlayId { m.surface = Self.overlayId }
        guard let id = target(m) else { return }
        if messageId != id { reset(); messageId = id }
        if let hit = pendingUnfold {
            pendingUnfold = nil
            if let i = earlierIndex(of: hit, in: m) { expanded.insert(i) }
        }
        guard openedFor != id, let t = thread(m) else { return }
        openedFor = id
        m.perform(.openThread(threadId: t.threadId))
    }

    /// Back to the list: the overlay and the frozen message go, the inbox surface returns.
    @discardableResult
    public func close(_ m: AppModel) -> Bool {
        reset()
        m.overlay = nil
        m.surface = Self.inboxSurface
        return true
    }

    /// The overlay went away by some other road (Reply opened the composer): the next open starts clean.
    public func dismissed() { reset() }

    func reset() {
        messageId = nil; copiedFor = nil; expanded = []; noteOpen = false; moreOpen = false; failure = nil; openedFor = nil; pendingUnfold = nil
    }

    // MARK: earlier messages

    public func toggleExpanded(_ index: Int) {
        if expanded.contains(index) { expanded.remove(index) } else { expanded.insert(index) }
    }
}
