import Foundation

/// The socket face of the thread's cards: one row per outgoing card (state, seconds left, sent time, text) and the
/// Undo click of each, read from the same spine store the view draws.
@MainActor
enum ThreadCardsRegistry {
    static func rows() -> [String: (AppModel) throws -> [ControlRow]] {
        [
            ThreadCardIds.outgoingRegion: { m in
                guard let t = ThreadCards.shownThread(m) else { return [] }
                return ThreadCards.outgoing(threadId: t).enumerated().map { i, c in
                    // `material`: what the card is drawn as, lifted glass (yours) or flat (on the record), round 5.
                    var row: ControlRow = ["ax": ThreadCardIds.outgoing(i), "id": c.id, "text": c.text, "account": c.accountId, "cc": c.cc.joined(separator: ", "), "material": "lifted"]
                    switch c.state {
                    case .waiting:
                        row["state"] = "waiting"
                        if QuickReplyModel.shared.outboxId == c.id, case .counting(_, let cd) = QuickReplyModel.shared.phase {
                            row["seconds"] = cd.secondsLeft(at: Date())
                        }
                        row["undo"] = ThreadCardIds.outgoingUndo(i)
                        row["undo_cap"] = ThreadCardCopy.undoCap
                    case .sending: row["state"] = "sending"
                    case .offline: row["state"] = "offline"
                    case .sent(let at): row["state"] = "sent"; row["time"] = ThreadCardCopy.sent(at); row["material"] = "flat"
                    case .failed(let w, let code, let account):
                        row["state"] = "failed"; row["words"] = w; row["code"] = code ?? ""; row["account_name"] = account
                        row["retry"] = ThreadCardIds.outgoingRetry(i); row["retry_cap"] = ThreadCardCopy.sendCap
                        row["discard"] = ThreadCardIds.outgoingDiscard(i); row["discard_cap"] = ThreadCardCopy.deleteCap
                        row["edit"] = ThreadCardIds.outgoingEdit(i); row["edit_cap"] = ThreadCardCopy.editCap
                        if let next = c.retryAtMs { row["retry_at_ms"] = Int(next) }   // the engine tries again by itself at this time (the card counts down to it)
                    }
                    return row
                }
            },
            ThreadCardIds.draftRegion: { m in
                guard let t = ThreadCards.shownThread(m) else { return [] }
                let q = QuickReplyModel.shared
                guard let d = cardDraft(q.drafts.drafts(threadId: t, core: m.core), q) else { return [] }
                return [["ax": ThreadCardIds.draft, "id": d.id, "lines": d.firstLines, "collapsed": QuickReplyThreadCards.collapses(q) ? "1" : "0",
                         "material": "lifted", "send": ThreadCardIds.draftSend, "send_cap": ThreadCardCopy.sendCap,
                         "delete": ThreadCardIds.draftDelete, "delete_cap": ThreadCardCopy.deleteCap,
                         // the head's address, as the card is given it (nil: the card draws no address at all)
                         "address": (m.core.accounts().first(where: { $0.id == d.accountId })?.address).map { $0 as Any } ?? NSNull()]]
            },
            ThreadCardIds.draftDeleted: { m in
                guard QuickReplyModel.shared.showsDeletedLine(threadId: ThreadCards.shownThread(m)) else { return [] }
                return [["ax": ThreadCardIds.draftDeleted, "undo": ThreadCardIds.draftUndo, "undo_cap": ThreadCardCopy.undoCap]]
            },
        ]
    }

    /// The draft the card shows right now (resolved when a click lands).
    private static func shownDraft(_ m: AppModel) -> String? {
        guard let t = ThreadCards.shownThread(m) else { return nil }
        let q = QuickReplyModel.shared
        return cardDraft(q.drafts.drafts(threadId: t, core: m.core), q)?.id
    }

    /// The one draft the card draws, the view, the socket row and the card's keys alike.
    /// Owner, 13 Nov, picture 206: "when you have a draft you shouldn't have a draft + writing an email no ? THat doesn't make sense". NEVER a draft
    /// card while the reply is open on that conversation: the reply holds the conversation's draft (R continues it, `resumeSavedDraft`). Round 20
    /// drew "the first draft the open reply does not hold", so a conversation with two saved drafts (the builds before 13 Nov saved a new one at
    /// every R) showed a card over the open reply.
    static func cardDraft(_ drafts: [DraftSummary], _ q: QuickReplyModel) -> DraftSummary? {
        if q.showsBox, let open = q.threadId, drafts.contains(where: { $0.threadId == open }) { return nil }
        return newestDraft(drafts)
    }

    /// The conversation's draft: the newest saved one that has words (an empty one has nothing to send or reopen: round 20, owner 07:29). A
    /// conversation that still holds older drafts keeps them in the Drafts mailbox; the reply and the card are about this one.
    static func newestDraft(_ drafts: [DraftSummary]) -> DraftSummary? {
        drafts.filter { !$0.firstLines.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }.max { $0.savedAt < $1.savedAt }
    }

    static func hasShownDraft(_ m: AppModel) -> Bool { shownDraft(m) != nil }
    /// A draft card is drawn for this conversation (the reader then draws no closed reply line: the card is the way in, R continues it).
    static func showsDraftCard(threadId: String, _ m: AppModel = .shared) -> Bool {
        let q = QuickReplyModel.shared
        _ = q.draftsRevision
        return cardDraft(q.drafts.drafts(threadId: threadId, core: m.core), q) != nil
    }
    static func sendShownDraft(_ m: AppModel) -> Bool { shownDraft(m).map { QuickReplyModel.shared.sendDraft($0, m) } ?? false }
    static func deleteShownDraft(_ m: AppModel) -> Bool { shownDraft(m).map { QuickReplyModel.shared.deleteDraft($0, m) } ?? false }

    /// ⌘⌫, the key both cards draw on "Delete": the shown draft card's draft, else the newest failed send of the conversation on screen
    /// (its pill drew the key and no key ran it).
    static func deleteShownCard(_ m: AppModel) -> Bool {
        if deleteShownDraft(m) { return true }
        guard let t = ThreadCards.shownThread(m),
              let failed = ThreadCards.outgoing(threadId: t, core: m.core).last(where: { if case .failed = $0.state { return true }; return false }) else { return false }
        return QuickReplyModel.shared.discardFailed(outboxId: failed.id, m)
    }

    static func clicks() -> [String: (AppModel) -> Bool] {
        let q = QuickReplyModel.shared
        var t: [String: (AppModel) -> Bool] = [
            ThreadCardIds.draft: { m in shownDraft(m).map { q.openDraft($0, m) } ?? false },
            ThreadCardIds.draftSend: { m in shownDraft(m).map { q.sendDraft($0, m) } ?? false },
            ThreadCardIds.draftDelete: { m in shownDraft(m).map { q.deleteDraft($0, m) } ?? false },
            ThreadCardIds.draftUndo: { q.undoDraftDelete($0) },
        ]
        // Ids are positional; a click resolves the card at that place when it lands, not when the table was built.
        for i in 0..<UndoDepthBound.cards {
            t[ThreadCardIds.outgoingUndo(i)] = { m in
                guard let c = card(i, m), case .waiting = c.state else { return false }
                return ThreadCards.undo(outboxId: c.id, m)
            }
            t[ThreadCardIds.outgoingRetry(i)] = { m in
                guard let c = card(i, m), case .failed = c.state else { return false }
                return q.retryFailed(outboxId: c.id, m)
            }
            t[ThreadCardIds.outgoingDiscard(i)] = { m in
                guard let c = card(i, m), case .failed = c.state else { return false }
                return q.discardFailed(outboxId: c.id, m)
            }
            t[ThreadCardIds.outgoingEdit(i)] = { m in
                guard let c = card(i, m), case .failed = c.state else { return false }
                return q.editFailed(outboxId: c.id, m)
            }
        }
        return t
    }

    private static func card(_ i: Int, _ m: AppModel) -> ThreadCards.Outgoing? {
        guard let th = ThreadCards.shownThread(m) else { return nil }
        let cards = ThreadCards.outgoing(threadId: th, core: m.core)
        return cards.indices.contains(i) ? cards[i] : nil
    }
}

/// How many outgoing cards' Undo ids the registry answers: the undo history's depth (UNDO_DEPTH), since a send older
/// than that can no longer be undone.
enum UndoDepthBound { static let cards = WhooshConstants.UNDO_DEPTH }
