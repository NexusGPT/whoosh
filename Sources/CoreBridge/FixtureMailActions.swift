import Foundation

/// Contracts delta 18 under --fixture-core, parts 2 and 3: leave a mailing list, mute a thread, remind me if no reply. The commands
/// (`Unsubscribe`, `Mute`, `RemindIfNoReply`) were served since delta 10; this adds what the screens read (`message/<id>/unsubscribe`,
/// `thread/<id>/reminder`), `ClearReplyReminder`, the Reminders view, the contract's refusals, and `fixture reply-arrived`.
/// Every rule is the contract's own function (`unsubscribeState`, `checkUnsubscribe`, `replyReminderState`, `arrivalInThread`, ...).
extension FixtureCore {
    /// The thread a message or thread id names: a fixture message's own thread, else the id itself (a draft with no reply thread is its own).
    func threadKeyLocked(_ id: String) -> String { threadOf(id) }

    /// The thread a reminder rides on, remembered when it was set (the draft may be sent and gone by the time it is read).
    func reminderThreadLocked(_ draftId: String) -> String {
        if let t = replyReminderThread[draftId] { return t }
        let t = draftRows[draftId]?.thread ?? entries.first(where: { $0.draft.id == draftId || $0.id == draftId })?.draft.replyToThreadId
        return threadKeyLocked(t ?? draftId)
    }

    /// The contract's reading of one reminder: waiting, fired or answered, from its time, the first reply of its thread and now.
    func reminderStateLocked(_ draftId: String) -> ReplyReminderState {
        replyReminderState(atMs: replyReminders[draftId], firstReplyMs: replyArrivedAt[reminderThreadLocked(draftId)], nowMs: clock.nowMs())
    }

    /// The reminder id of a thread, if one is set (a thread holds one: setting again replaces it).
    func reminderIdLocked(thread: String) -> String? {
        let key = threadKeyLocked(thread)
        return replyReminders.keys.sorted().first { reminderThreadLocked($0) == key }
    }

    /// Whether the message's thread already holds a muted message: the arrival rule asks this.
    func threadMutedLocked(_ messageId: String) -> Bool {
        let key = threadKeyLocked(messageId)
        return muted.contains { threadKeyLocked($0) == key }
    }

    /// The first message of every thread whose reminder fired, in fixture order: the rows of the Reminders view.
    func firedReminderIdsLocked() -> [String] {
        let fired = Set(replyReminders.keys.filter { replyReminderInView(state: reminderStateLocked($0)) }.map { reminderThreadLocked($0) })
        var seen = Set<String>()
        return order.filter { id in
            let t = threadKeyLocked(id)
            guard fired.contains(t), !seen.contains(t) else { return false }
            seen.insert(t); return true
        }
    }

    // MARK: commands (inside the lock)

    func unsubscribeLocked(_ id: String) throws -> CommandOutcome {
        guard fxMessage(id) != nil else { throw CoreError.NotFound(kind: "message", id: id) }
        let how = try checkUnsubscribe(method: unsubscribeMethodLocked(id), done: listsLeft[id] != nil || unsubscribed.contains(id))
        try refuseFailing([id])
        listsLeft[id] = how
        listsLeftAt[id] = clock.nowMs()
        return CommandOutcome(undoEntryId: nil, createdId: nil)
    }

    func remindIfNoReplyLocked(_ draftId: String, _ at: Int64) throws -> CommandOutcome {
        guard draftRows[draftId] != nil || entries.contains(where: { $0.draft.id == draftId || $0.id == draftId }) else {
            throw CoreError.NotFound(kind: "draft", id: draftId)
        }
        try checkRemindIfNoReply(atMs: at, nowMs: clock.nowMs())
        let thread = reminderThreadLocked(draftId)
        // a thread holds one reminder: setting again replaces the earlier one (and forgets the reply that answered it)
        if let other = reminderIdLocked(thread: thread), other != draftId { replyReminders[other] = nil; replyReminderThread[other] = nil }
        replyReminders[draftId] = at; replyReminderThread[draftId] = thread
        return CommandOutcome(undoEntryId: nil, createdId: draftId)
    }

    func clearReplyReminderLocked(_ draftId: String) throws -> CommandOutcome {
        try checkClearReplyReminder(state: replyReminders[draftId] == nil ? .noReminder : reminderStateLocked(draftId))
        replyReminders[draftId] = nil; replyReminderThread[draftId] = nil
        return CommandOutcome(undoEntryId: nil, createdId: draftId)
    }

    // MARK: fixture verb

    /// `fixture reply-arrived <thread or message id>`: someone replied in the thread now. Only the first reply counts (a reply after the
    /// reminder fired does not unfire it: the contract's `replyReminderState`). Returns the thread's reminder state word afterwards.
    @discardableResult
    public func fixtureReplyArrived(threadId: String) throws -> String {
        let word: String = try lock.withLock {
            guard fxMessage(threadId) != nil || order.contains(where: { threadKeyLocked($0) == threadId }) || reminderIdLocked(thread: threadId) != nil else {
                throw CoreError.NotFound(kind: "thread", id: threadId)
            }
            let key = threadKeyLocked(threadId)
            if replyArrivedAt[key] == nil { replyArrivedAt[key] = clock.nowMs() }
            return replyReminderStateKey(state: reminderIdLocked(thread: key).map { reminderStateLocked($0) } ?? .noReminder)
        }
        emit()
        return word
    }

    /// `fixture filing-refuse <message id> [words]`: the filing AI refuses to sort this mail from now on (what the real core serves as `ai: refused`).
    public func fixtureFilingRefuse(messageId: String, words: String) throws {
        try lock.withLock {
            guard fxMessage(messageId) != nil else { throw CoreError.NotFound(kind: "message", id: messageId) }
            filingRefused[messageId] = words
        }
        emit()
    }

    // MARK: state reads

    /// `message/<id>/unsubscribe` and `thread/<id>/reminder`; nil = not a delta 18 path.
    func mailActionsStateLocked(_ p: [String]) -> D9Hit? {
        switch (p.first, p.count) {
        case ("message", 3) where p[2] == "unsubscribe":
            guard fxMessage(p[1]) != nil else { return D9Hit(value: nil) }
            let state = unsubscribeState(method: unsubscribeMethodLocked(p[1]), done: listsLeft[p[1]] != nil || unsubscribed.contains(p[1]))
            return D9Hit(value: d18Json(unsubscribeStateKey(state: state)))
        case ("message", 3) where p[2] == "unsubscribed_at":
            guard fxMessage(p[1]) != nil, listsLeft[p[1]] != nil || unsubscribed.contains(p[1]) else { return D9Hit(value: nil) }
            return D9Hit(value: listsLeftAt[p[1]].map { String($0) })
        case ("message", 3) where p[2] == "filing":   // the engine's shape (label_flow.rs filing_json): ai, ai_refusal, card, chain_id, decided_by, place
            guard fxMessage(p[1]) != nil else { return D9Hit(value: nil) }
            let words = filingRefused[p[1]]
            let o: [String: Any] = ["ai": words == nil ? "configured" : "refused", "ai_refusal": words.map { $0 as Any } ?? NSNull(), "card": "hand",
                                    "chain_id": NSNull(), "decided_by": "default", "place": "primary"]
            return D9Hit(value: d18Json(o))
        case ("message", 3) where p[2] == "fetched":   // J7.1: the test engine holds every body it lists, so a listed message is on the phone
            return fxMessage(p[1]) == nil ? D9Hit(value: nil) : D9Hit(value: "true")
        case ("thread", 3) where p[2] == "summary":   // the engine's shape (thread_summary.rs): the kept summary as a JSON string, or null
            guard let t = fx.open_thread, t.id == p[1], let words = t.messages?.last?.agent_summary else { return D9Hit(value: "null") }
            return D9Hit(value: d18Json(words))
        case ("thread", 3) where p[2] == "reminder":
            let id = reminderIdLocked(thread: p[1])
            let state = id.map { reminderStateLocked($0) } ?? .noReminder
            var o: [String: Any] = ["state": replyReminderStateKey(state: state), "at_ms": NSNull(), "draft_id": id.map { $0 as Any } ?? NSNull()]
            switch state { case .waiting(let at), .fired(let at), .answered(let at): o["at_ms"] = at; default: break }
            return D9Hit(value: d18Json(o))
        default: return nil
        }
    }
}

extension FixtureCore {
    /// The subject without its Re:/Fwd: prefixes, lower case: how the fixture (which gives an arrival no thread) recognises a thread.
    static func subjectKey(_ subject: String) -> String {
        var s = subject.trimmingCharacters(in: .whitespaces).lowercased()
        while let r = ["re:", "fwd:", "fw:"].first(where: { s.hasPrefix($0) }) { s = String(s.dropFirst(r.count)).trimmingCharacters(in: .whitespaces) }
        return s
    }

    /// Whether a muted message other than `id` carries this subject (so the arrival belongs to a muted thread).
    func mutedThreadMatchesLocked(subject: String, except id: String) -> Bool {
        let key = Self.subjectKey(subject)
        return !key.isEmpty && muted.contains { $0 != id && fxMessage($0).map { Self.subjectKey($0.subject) == key } ?? false }
    }
}
