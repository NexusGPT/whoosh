import Foundation

/// What the pill, the bar, the More panel and the note do. Each runs on the input frame: the model changes first, the
/// screen follows, and nothing waits for an animation.
extension ThreadPhoneState {
    // MARK: the code pill

    /// The code goes on the clipboard, the message is marked read, the pill says "Copied". Nothing else: the message
    /// is never archived and stays in the inbox. False when the open message carries no code.
    @discardableResult
    public func copyCode(_ m: AppModel) -> Bool {
        guard let t = thread(m), let code = t.open.code else { return false }
        ThreadClipboard.put(code)
        copiedFor = t.messageId
        m.perform(.markRead(messageIds: [t.messageId], read: true))
        return true
    }

    // MARK: the bar and the menu

    @discardableResult
    public func reply(_ m: AppModel) -> Bool {
        guard let id = target(m) else { return false }
        m.focus(messageId: id)
        // The composer opens on the tap, as PhoneRouter.reply does (PhoneRouter.swift:69), never in the sheet's onAppear: a sheet that
        // presents late (another one still leaving, the first-launch welcome over the stack) opened nothing, and the keys, the socket and
        // the dictation found no composer (phone round 4). The sheet's own onAppear then finds the model open and does nothing again.
        ComposerPhoneFeature.reply(m, messageId: id)
        m.overlay = "iphone-composer"
        return true
    }

    /// Archive or Snooze: the inbox's own act (same command, same undo capsule), then back to the list. A refusal
    /// keeps the thread open and says why. False when there is no open message.
    @discardableResult
    public func leave(_ action: SwipeAction, _ m: AppModel) -> Bool {
        guard let id = target(m) else { return false }
        let inbox = InboxPhoneState.shared
        guard let i = inbox.rows(m).firstIndex(where: { $0.messageId == id }) else { return false }
        inbox.act(action, row: i, in: m)
        if m.rows.contains(where: { $0.messageId == id }) {
            failure = m.lastFailure.map { "\($0.accountName): \($0.reasonWords)" } ?? "\(action.title) was refused"
            return true
        }
        return close(m)
    }

    @discardableResult
    public func togglePin(_ m: AppModel) -> Bool {
        guard let id = target(m), let row = m.rows.first(where: { $0.messageId == id }) else { return false }
        return m.perform(.pin(messageId: id, pinned: !row.pinned))
    }

    @discardableResult
    public func markUnread(_ m: AppModel) -> Bool {
        guard let id = target(m) else { return false }
        copiedFor = nil
        return m.perform(.markRead(messageIds: [id], read: false))
    }

    public func clearFailure() { failure = nil }

    // MARK: reload

    /// Pull to refresh and the reload control: the held bodies are forgotten, so the next draw reads the stored body
    /// (a body that arrived since swaps in place).
    @discardableResult
    public func reload(_ m: AppModel) -> Bool {
        guard let t = thread(m) else { return false }
        ReaderBodies.shared.forget(messageIds: t.earlier.compactMap { $0?.id } + [t.messageId])
        m.refresh()
        return true
    }

    // MARK: the thread's private note (J8.4)

    /// The note the core keeps for this thread: first of `notes(target:)`.
    public func note(_ m: AppModel) -> Note? {
        thread(m).flatMap { m.core.notes(target: .thread(threadId: $0.threadId)).first }
    }

    public func openNote(_ m: AppModel) {
        noteDraft = note(m)?.text ?? ""
        noteOpen = true
    }

    /// Saves the draft: an edit of the thread's note, or a first note. An empty draft on no note saves nothing.
    @discardableResult
    public func saveNote(_ m: AppModel) -> Bool {
        guard let t = thread(m) else { return false }
        let text = noteDraft.trimmingCharacters(in: .whitespacesAndNewlines)
        let ok: Bool
        if let existing = note(m) {
            ok = m.perform(.editNote(noteId: existing.id, text: text))
        } else if text.isEmpty {
            ok = true
        } else {
            ok = m.perform(.addNote(target: .thread(threadId: t.threadId), text: text))
        }
        if ok { noteOpen = false }
        return ok
    }
}
