import Foundation

/// Remind me if no reply (⌘⇧H). The owner picks a time with the same words as Send later; when the message goes out, the
/// core's RemindIfNoReply is given the outbox entry and that time, and the thread comes back to the top of the inbox if
/// nobody answered by then. A reminder chosen after the send (the undo bar is up) goes to the entry at once.
extension ComposerModel {
    /// "Reminder set for Tomorrow 8:00 AM", or "" when none is set.
    var reminderWords: String { reminderAt.map { ComposerWords.reminderSet(TimeInWords.describe($0, now: Date())) } ?? "" }

    /// The reminder time in milliseconds since 1970 for the socket row; 0 when none is set.
    var reminderMs: Int64 { reminderAt.map { Int64($0.timeIntervalSince1970 * 1000) } ?? 0 }

    @discardableResult
    func toggleRemind() -> Bool {
        guard isOpen else { return false }
        let open = !remindOpen
        closePanels(); remindOpen = open
        if open { resignText() }
        return true
    }

    /// A chosen time: refused in a sentence when it has passed, otherwise kept (and sent to the core now when the message is
    /// already in the outbox).
    @discardableResult
    func setReminder(_ date: Date, _ m: AppModel) -> Bool {
        guard isOpen else { return false }
        if case .refused(let reason) = TimeInWords.validate(date, now: Date()) { line = reason; return true }
        reminderAt = date
        remindOpen = false; datePicking = false
        line = reminderWords
        if let id = outboxId { applyReminder(to: id, m) }
        session.hooks.focusBody()
        return true
    }

    @discardableResult
    func clearReminder() -> Bool {
        guard isOpen, reminderAt != nil else { return false }
        reminderAt = nil; remindOpen = false; line = ""
        session.hooks.focusBody()
        return true
    }

    /// The core keeps the reminder against the entry the send created. A refusal is said in the line, never swallowed.
    func applyReminder(to outboxId: String, _ m: AppModel) {
        guard let at = reminderAt else { return }
        do {
            _ = try m.core.run(command: .remindIfNoReply(draftId: outboxId, atMs: Int64(at.timeIntervalSince1970 * 1000)))
            line = reminderWords
        } catch {
            line = Self.words(error)
        }
    }
}
