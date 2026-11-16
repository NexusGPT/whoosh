import Foundation

/// The reply's text as a saved draft of its thread: leaving the field with words saves them, the thread's draft card
/// reopens them, sends them or deletes them. The drafts themselves live in the core (DraftSource); the model holds only
/// which one the field has open and the last delete, so the card can offer its undo.
extension QuickReplyModel {
    /// Esc or a move to another thread with words in the field: they become the thread's draft; a draft the field
    /// reopened is updated in place (`updateDraft`: same id, no undo entry).
    func keepAsDraft(_ m: AppModel) {
        settle(m)
        aiReleaseForKeep()   // an untouched AI draft stays the core's proposal; only the owner's own words are kept here
        guard let threadId, outboxId == nil else { return }
        let text = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        let account = context.accountId.isEmpty ? nil : context.accountId
        // An opened header (To, Cc, Cci, Objet; round 5) is saved with the words; a plain reply keeps the thread's own.
        let fields = header.isExpandedSend
            ? DraftFields(to: header.to.map(\.address), cc: header.addresses(header.cc), bcc: header.addresses(header.bcc), subject: header.subject)
            : DraftFields.plain
        if let open = openDraftId {
            if text == (drafts.body(draftId: open, core: m.core) ?? "").trimmingCharacters(in: .whitespacesAndNewlines), !header.isExpandedSend { return }
            if text.isEmpty {
                try? drafts.delete(draftId: open, core: m.core)   // the words were cleared: the draft goes (Z brings it back)
                openDraftId = nil
            } else {
                try? drafts.update(draftId: open, accountId: account, fields: fields, body: draft, core: m.core)   // same draft, no undo entry
            }
        } else {
            guard !text.isEmpty else { return }
            openDraftId = try? drafts.save(threadId: threadId, accountId: account, fields: fields, body: draft, core: m.core)
            if let saved = openDraftId, !fields.bcc.isEmpty { try? drafts.update(draftId: saved, accountId: account, fields: fields, body: draft, core: m.core) }
        }
        draftsRevision += 1
        m.refresh()
    }

    /// R on a conversation that already holds a saved reply draft CONTINUES that draft: its words are in the field, the cursor after them,
    /// and its card is not drawn while the field holds it (lead, 13 Nov, owner A162 and picture 205; walk on main117: R beside a draft card
    /// opened a second, empty reply, and its words became a second draft of the same conversation, then a third: the owner's Drafts count
    /// grew by one per try and a card stayed under the open reply). An empty field only: words already there are never replaced.
    func resumeSavedDraft(_ m: AppModel) {
        guard phase == .editing, draft.isEmpty, openDraftId == nil, let threadId,
              let saved = ThreadCardsRegistry.newestDraft(drafts.drafts(threadId: threadId, core: m.core)),
              let body = drafts.body(draftId: saved.id, core: m.core) else { return }
        draft = withoutSignature(body)
        openDraftId = saved.id
        lastDeletedDraft = nil
        restoreHeader(saved.id, m)
    }

    /// A saved draft's words as the reply box holds them: without the account's signature at their end. The full composer writes the signature
    /// INTO the body, the reply box shows it apart and adds it at the send, so a draft that went through the composer (pop-out, then Esc) came
    /// back with the signature in the text and a second one under it (lead, 13 Nov, walk u130).
    func withoutSignature(_ body: String) -> String {
        let signed = "\n\n" + context.signature
        guard !context.signature.isEmpty, body.hasSuffix(signed) else { return body }
        return String(body.dropLast(signed.count))
    }

    /// A click on the draft card: the field opens on the thread with the draft's text.
    public func openDraft(_ id: String, _ m: AppModel) -> Bool {
        settle(m)
        guard let body = drafts.body(draftId: id, core: m.core) else { return false }
        if !isOpen { guard open(m) else { return false } }
        guard outboxId == nil else { return false }
        draft = withoutSignature(body)
        openDraftId = id
        lastDeletedDraft = nil
        restoreHeader(id, m)
        askKeys()
        return true
    }

    /// The reopened draft's To, Cc, Cci and Objet come back with its words (DraftSummary.to/cc/bcc/subject, contract
    /// delta 12, the lead's ask of 02:20); a name the core knows labels the chip, else the address stands alone.
    private func restoreHeader(_ id: String, _ m: AppModel) {
        guard let threadId, let d = drafts.drafts(threadId: threadId, core: m.core).first(where: { $0.id == id }) else { return }
        let to = d.to.map { ReplyRecipient(name: m.core.contact(address: $0)?.name ?? "", address: $0) }
        header.restore(to: to, cc: d.cc, bcc: d.bcc, subject: d.subject)
    }

    /// Send ⌘↵ on the card: the draft goes through the same send as the field (outgoing card, undo window); the send
    /// removes it from the drafts.
    public func sendDraft(_ id: String, _ m: AppModel) -> Bool {
        guard openDraft(id, m), send(m) else { return false }
        return outboxId != nil
    }

    /// Delete ⌫ on the card: gone at once; a draft with words offers Undo (the core's Z entry), an empty one asks nothing.
    public func deleteDraft(_ id: String, _ m: AppModel) -> Bool {
        let words = (drafts.body(draftId: id, core: m.core) ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        guard (try? drafts.delete(draftId: id, core: m.core)) != nil else { return false }
        if openDraftId == id { openDraftId = nil }
        lastDeletedDraft = words.isEmpty ? nil : id
        lastDeletedDraftThread = lastDeletedDraft == nil ? nil : (ThreadCards.shownThread(m) ?? threadId)
        draftsRevision += 1
        m.refresh()
        return true
    }

    /// An open reply with no word that does not hold the keyboard (the state after a send landed, or a reply the pointer left empty). A reply that
    /// was just asked for (R) holds the keyboard by intent (`wantsKeys`) before its text has it: it is not idle, so an Esc pressed right after R
    /// closes the reply and stays on the mail (par-escape on main158 under load: that Esc also went back to the list).
    var isIdle: Bool { isOpen && !fieldFocused && !wantsKeys && phase == .editing && draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !fromOpen && !emojiOpen }

    /// Folds an idle reply. True when it folded: the Esc that asked goes on to the screen behind in the same press (lead, 13 Nov,
    /// real-engine walk on main121 and walk p123: after a sent reply the first Esc only folded the empty reply).
    @discardableResult
    func foldIfIdle(_ m: AppModel) -> Bool { isIdle && close(m) }

    /// Discard (⇧⌘, in the reply's text): the words go and the reply closes. A draft the field had open is deleted the way its card's Delete
    /// does, so the thread shows "Draft deleted" with its Undo; words never saved leave nothing behind. Never while a send counts down.
    public func discard(_ m: AppModel) -> Bool {
        settle(m)
        guard phase == .editing || isFailed, outboxId == nil else { return false }
        if let open = openDraftId { _ = deleteDraft(open, m) }
        draft = ""
        return close(m)
    }

    /// Undo on the "Draft deleted" line: the core's last undo entry is that delete.
    public func undoDraftDelete(_ m: AppModel) -> Bool {
        guard lastDeletedDraft != nil, m.perform(.undo) else { return false }
        lastDeletedDraft = nil
        draftsRevision += 1
        return true
    }

    /// The draft the real engine put back when a send was cancelled (core/crates/outbox/src/usecases/cancel_send.rs: DraftEffect::Restore,
    /// the entry's own draft id), when the thread holds it; nil when nothing came back (the fixture core restores nothing).
    func restoredDraft(_ draftId: String, thread: String?, _ m: AppModel) -> String? {
        guard let t = thread, drafts.drafts(threadId: t, core: m.core).contains(where: { $0.id == draftId }) else { return nil }
        return draftId
    }

    /// A cancel that is not an edit (Retry queues the words again, Delete drops them): the copy the engine put back is not a draft the
    /// owner made, so it goes, or the thread would draw it as a draft card (round 20).
    func dropRestored(_ draftId: String, thread: String?, _ m: AppModel) {
        guard let r = restoredDraft(draftId, thread: thread, m) else { return }
        try? drafts.delete(draftId: r, core: m.core)
        draftsRevision += 1
    }

    /// Edit ⌘E on a failed card (owner 2026-11-13 07:28; lead: "Not sent" with the engine's reason, Retry and Edit): the send leaves the
    /// outbox and its words come back into the reply's text, the cursor in it. Words already in the field are kept as the thread's draft first.
    public func editFailed(outboxId id: String, _ m: AppModel) -> Bool {
        settle(m)
        guard outboxId == nil, let e = m.core.outbox().first(where: { $0.id == id }), case .failed = e.state,
              let thread = e.draft.replyToThreadId else { return false }
        if !(isOpen && threadId == thread) { guard open(m), threadId == thread else { return false } }
        if !draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { keepAsDraft(m); openDraftId = nil }
        guard (try? m.core.run(command: .cancelSend(outboxId: id))) != nil else { return false }
        var words = e.draft.body
        let signed = "\n\n" + context.signature
        if !context.signature.isEmpty, words.hasSuffix(signed) { words.removeLast(signed.count) }   // the send added it; the field shows it apart
        openDraftId = restoredDraft(e.draft.id, thread: thread, m)
        draft = words
        phase = .editing
        draftsRevision += 1
        askKeys()
        m.refresh()
        return true
    }

    /// ⌘E: the newest failed send of the thread on screen goes back into the reply's text.
    func editNewestFailed(_ m: AppModel) -> Bool {
        guard let t = threadId ?? ThreadCards.shownThread(m) else { return false }
        guard let last = ThreadCards.outgoing(threadId: t, core: m.core).last(where: { if case .failed = $0.state { return true }; return false }) else { return false }
        return editFailed(outboxId: last.id, m)
    }

    /// The send in the window has left it (the core sent or cancelled it, with or without a view drawing the countdown):
    /// the field is free again. Every action of the reply asks this first, so a hidden window or a moved clock never
    /// leaves the field locked behind a finished send.
    func settle(_ m: AppModel) {
        guard let id = outboxId, !m.core.outbox().contains(where: { $0.id == id }) else { return }
        expire(m)
    }
}
