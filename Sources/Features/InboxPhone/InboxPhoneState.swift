import Foundation
import Observation

/// The iPhone inbox's one state object: the undo capsule, the refusal line, the swipe picks, the Mailboxes
/// selection and the multi-select. Mail data stays in the core and AppModel; this holds only what the screen owns.
@MainActor @Observable
public final class InboxPhoneState {
    public static let shared = InboxPhoneState()

    /// The undo capsule: what the last swipe did, until its window closes (UNDO_SEND_DEFAULT_S) or Undo is used.
    public struct UndoNote: Equatable { public let verb: String; public let sender: String; public let at: Date }
    public private(set) var undo: UndoNote?
    /// A refused swipe in words, naming the account; the row stays where it was.
    public private(set) var failure: String?
    public var swipes = SwipeSettings.load() { didSet { swipes.save() } }
    /// The Mailboxes ticks: empty = All Inboxes.
    public var picked: Set<String> = []
    public var selecting = false
    public var selected: Set<String> = []
    public private(set) var refreshedAt = Date()
    public var searching = false

    private init() {}

    public var undoWindow: UndoCountdown? { undo.map { UndoCountdown(startedAt: $0.at) } }

    public func undoVisible(now: Date = Date()) -> Bool {
        guard let w = undoWindow else { return false }
        return !w.isExpired(at: now)
    }

    /// The rows the list shows: the core's rows for the tab, narrowed to the ticked accounts.
    public func rows(_ m: AppModel) -> [InboxRow] {
        picked.count > 1 ? m.rows.filter { picked.contains($0.accountId) } : m.rows
    }

    public func title(_ m: AppModel) -> String {
        switch picked.count {
        case 0: "All Inboxes"
        case 1: PhoneAccounts.of(picked.first!, in: m).name
        default: "\(picked.count) accounts"
        }
    }

    /// Applies the Mailboxes ticks: one account filters in the core, several narrow the list here.
    public func apply(_ m: AppModel) {
        m.accountFilter = picked.count == 1 ? picked.first : nil
        m.refresh()
    }

    /// One swipe or one long-press action on the row at `index`. Feedback on the same frame: the model changes at once.
    /// Returns false only when there is no such row. A refusal keeps the row and names the account.
    @discardableResult
    public func act(_ action: SwipeAction, row index: Int, in m: AppModel) -> Bool {
        let visible = rows(m)
        guard visible.indices.contains(index) else { return false }
        let row = visible[index]
        if m.perform(action.command(row)) {
            failure = nil
            if action.removesRow { undo = UndoNote(verb: action.done, sender: row.senderName, at: Date()) }
        } else {
            failure = m.lastFailure.map { "\($0.accountName): \($0.reasonWords)" } ?? "\(action.title) was refused"
        }
        return true
    }

    @discardableResult
    public func undoLast(_ m: AppModel) -> Bool {
        guard m.perform(.undo) else { failure = m.lastFailure?.reasonWords; return false }
        undo = nil
        return true
    }

    public func expireUndo() { undo = nil }
    public func clearFailure() { failure = nil }

    /// Pull to refresh: read the core again; the rows on screen stay until the new read lands.
    public func refresh(_ m: AppModel) {
        m.refresh()
        refreshedAt = Date()
    }

    public func toggle(_ messageId: String) {
        if selected.contains(messageId) { selected.remove(messageId) } else { selected.insert(messageId) }
    }

    /// The selection bar's Archive / Snooze over every ticked row, one command.
    @discardableResult
    public func actOnSelection(_ action: SwipeAction, in m: AppModel) -> Bool {
        let ids = Array(selected)
        guard !ids.isEmpty else { return false }
        let cmd: Command = action == .snooze ? .snooze(messageIds: ids, when: .tomorrowMorning) : .archive(messageIds: ids)
        let ok = m.perform(cmd)
        if ok { undo = UndoNote(verb: action.done, sender: "\(ids.count) mails", at: Date()); selected = []; selecting = false }
        else { failure = m.lastFailure.map { "\($0.accountName): \($0.reasonWords)" } }
        return ok
    }
}
