import Foundation

// gap-replies (item ebde5eae, journey J10.8): the arrival door of the test engine for the automatic draft.
// The real core runs the three steps on each arrival and, when all three pass, STORES a draft (made_by auto, state proposed, instruction None) for the
// thread: core/crates/engine/src/replies_work/usecases/arrival_pass.rs. The fixture core did not, so J10.8 "a draft written alone on arrival" read
// COULD-NOT-DRIVE. This is that step for the fixture: the same three contract functions TryReply uses (replyEntryStep, replyAiStep, replyExitStep),
// the same text as an on-demand draft, served at `reply/draft/<thread>` and announced by ReplyDraftReady / ReplyDraftChanged.
// NOTHING IS SENT: the draft is stored and shown, no outbox entry is made (only the owner sends).
// Pass rows are not written (the counts of the auto pane stay the scripted ones), a design choice of the fixture named in docs/lanes/gap-replies.md.
extension FixtureCore {
    /// The test engine's stand-in for the model's one yes/no question ("does this mail wait for my reply?", the filter's AI description: a question,
    /// a request, a decision): yes when the mail is already filed as waiting, or when its subject or text holds a question mark. An arrival is
    /// inserted in Primary and no built-in label files it as Needs Reply, so the waiting fact alone would stop every arrival at this step.
    func asksSomething(_ f: MessageFacts) -> Bool { f.subject.contains("?") || f.body.contains("?") }

    /// An arrival the auto use is on for gets its live draft. Called with the lock held, once per arrival, after the label rules have filed it.
    func writeArrivalDraftLocked(_ id: String) {
        guard replyBookStore != nil else { return }                                            // the owner never looked at the replies: no book, no pass (the fixture builds it on first use)
        let b = replyBookLocked()
        guard b.uses[.auto]?.enabled == true, let m = fxMessage(id), !gone(id) else { return }
        let thread = threadOf(id)
        guard !b.drafts.contains(where: { $0.messageId == id }) else { return }               // never twice for one message
        let f = facts(m)
        // The model that files Needs Reply on a real arrival is stood in for by the same question as the AI step below: a mail that asks something carries the
        // built-in Needs Reply label for the entry step, else the filter's label ticks (Needs Reply, Clients) would stop every arrival there (J10.8 read no draft).
        let waits = f.unanswered || asksSomething(f)
        let labelIds = (assigned[m.id] ?? []) + [category[m.id] ?? m.tab ?? "primary"] + (waits ? ["needs-reply"] : [])
        let entry = replyEntryStep(filter: b.filter, messageLabelIds: labelIds, facts: f)
        let ai = (entry == .passed || entry == .skipped) ? replyAiStep(filter: b.filter, modelSaysReplyWanted: f.unanswered || asksSomething(f)) : .notReached
        let exit = (ai == .passed || ai == .skipped) ? replyExitStep(filter: b.filter, facts: f) : .notReached
        let through = { (r: ReplyStepResult) in r == .passed || r == .skipped }
        guard through(entry), through(ai), through(exit) else { return }
        let now = clock.nowMs()
        // one live draft per thread: an older one becomes Discarded
        if let i = b.drafts.firstIndex(where: { $0.threadId == thread && b.isLive($0) }), let old = replyDraftReplaced(live: b.drafts[i], nowMs: now) {
            b.drafts[i] = old
            pending.append((.thread, .replyDraftChanged(threadId: thread, draft: old)))
        }
        let draftId = newId("reply-draft")
        let d = ReplyDraft(id: draftId, threadId: thread, messageId: id, text: "Bonjour,\n\nMerci pour votre message.\n\nBien à vous,\nAssem", madeBy: .auto,
                           excerptIds: [chunkExcerptId(chunkId: 1)], instruction: nil, state: .proposed, revision: 0, createdAtMs: now, updatedAtMs: now)
        b.drafts.append(d)
        pending.append((.thread, .replyDraftReady(threadId: thread, draftId: draftId)))
        pending.append((.thread, .replyDraftChanged(threadId: thread, draft: d)))
    }
}
