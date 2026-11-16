import Foundation

/// Send, undo, send later (J3.5 to J3.8). Replies go through `quickSend`, which opens the core's undo window; a new
/// message and Scheduled need the core's `send(draftId)` and `schedule`, which the fixture core refuses in words.
extension ComposerModel {
    public var canSend: Bool {
        // a picture in the text is something to send even with no words (gap-composer-rich)
        guard !bodyText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !ComposerRichMail.pictureIds(session.document).isEmpty else { return false }
        return !to.isEmpty || !toInput.isEmpty
    }

    /// The text that goes out: the body, the signature, then the footer line when the owner switched it on. A picture in the text goes as
    /// its marker (`[picture:<attachment id>]`, ComposerRichMail); the core builds the HTML part from the marker and the signature's marks.
    var outgoing: String {
        let body = ComposerRichMail.outgoingText(session.document)
        return Self.assemble(body: body, signature: signature, footer: library.footerOn ? ComposerLibrary.footerLine : nil)
    }

    /// Text, signature, footer, in that order. The signature is always written here, after a blank line and with no "-- " line: the core
    /// (core/crates/send/src/compose.rs `with_signature`, rich.rs `strip_signature`, both through `split_signature`) knows the signature is
    /// already there when its whole text stands at the start of a line and at most a few short lines follow it (the footer), so one copy
    /// leaves. The core cannot place a signature before a footer it does not know, so with the footer on this function must write it.
    static func assemble(body: String, signature: String, footer: String?) -> String {
        var t = body
        if !signature.isEmpty { t += "\n\n" + signature }
        if let footer { t += "\n\n" + footer }
        return t
    }

    /// ⌘Enter. One entry per send: a second press during the window does nothing.
    @discardableResult
    public func send(_ m: AppModel, markDone: Bool = false, instant: Bool = false) -> Bool {
        if case .counting = phase { return false }
        if !toInput.isEmpty { _ = commitRecipient() }
        guard canSend else { line = ComposerWords.nothingToSend; return true }
        do {
            let created = try submit(m)
            keepPastedUntilSent(created, m)
            sentSnapshot = (bodyText, to, subject)
            phase = .counting(outboxId: created, countdown: countdown(for: created, m))
            closePanels(); line = ""
            applyReminder(to: created, m)
            m.refresh()
            if markDone { archiveAnswered(m) }
            if instant { expire(m) }
        } catch {
            // a refusal about the files keeps the composer editing: the sentence stays in the line until the file is removed
            if Self.refusalCode(error) == nil { phase = .failed(words: Self.words(error)) } else { refreshFiles() }
            line = Self.words(error)
            lineDetail = Self.details(error).map { (line, $0) }
        }
        return true   // the key did something: the composer now says why it did not send
    }

    /// ⌘⇧Enter: send, and the message that was answered leaves the inbox as done.
    @discardableResult
    public func sendAndMarkDone(_ m: AppModel) -> Bool { send(m, markDone: true) }

    /// ⌘⇧Z: no undo window; the bar says Sent at once. (The core's send-now door is a reported need.)
    @discardableResult
    public func sendInstantly(_ m: AppModel) -> Bool { send(m, instant: true) }

    private func archiveAnswered(_ m: AppModel) {
        guard let id = replyMessageId, m.perform(.archive(messageIds: [id])) else { return }
        markedDone = true
    }

    /// The window the outbox granted this entry (until minus created), not a number of this lane.
    private func countdown(for id: String, _ m: AppModel) -> UndoCountdown {
        guard let e = m.core.outbox().first(where: { $0.id == id }) else { return UndoCountdown(startedAt: Date()) }
        entryDraftId = e.draft.id
        guard case .undoable(let until) = e.state, until > e.createdAtMs else { return UndoCountdown(startedAt: Date()) }
        return UndoCountdown(startedAt: Date(), seconds: Int((until - e.createdAtMs) / 1000))
    }

    private func submit(_ m: AppModel) throws -> String {
        settleFiles()
        let id: String?
        if let d = try saveOnCoreDraft(m) {            // a draft with files: its text is saved, then the core sends the draft with its files
            id = try m.core.run(command: .send(draftId: d)).createdId
        } else if let t = threadId { id = try m.core.run(command: .quickSend(threadId: t, body: outgoing)).createdId }
        else { id = try m.core.run(command: .send(draftId: try saveNewDraft(m))).createdId }
        guard let id else { throw CoreError.Invalid(field: "send", reasonWords: ComposerWords.notSaved) }
        return id
    }

    /// The core's draft of this composer with its text saved, or nil when there is none. A draft the core no longer has (a send took it
    /// into the outbox and was undone; the fixture's cancel does not give it back) is forgotten, so the send goes the way a draft-less one does.
    private func saveOnCoreDraft(_ m: AppModel) throws -> String? {
        guard let d = coreDraftId else { return nil }
        do {
            _ = try m.core.run(command: .updateDraft(draftId: d, fromAccountId: fromId.isEmpty ? nil : fromId, to: to, cc: cc, bcc: bcc, subject: subject, body: outgoing))
            return d
        } catch CoreError.NotFound(let kind, _) where kind == "draft" {
            coreDraftId = nil
            return nil
        }
    }

    /// A message composed from scratch has no draft in the core yet (probes-r1, J6.7: `send(draftId: "draft-new")` was
    /// NotFound): save it first and send the id the core returns. A refused save throws the core's own words, so the
    /// composer stays open and says why (the catch in `send`). Bcc has no door on save: it lands by the update.
    private func saveNewDraft(_ m: AppModel) throws -> String {
        let from = fromId.isEmpty ? nil : fromId
        let saved = try m.core.run(command: .saveDraft(fromAccountId: from, to: to, cc: cc, subject: subject, body: outgoing, replyToThreadId: threadId)).createdId
        guard let d = saved else { throw CoreError.Invalid(field: "draft", reasonWords: ComposerWords.notSaved) }
        if !bcc.isEmpty {
            _ = try m.core.run(command: .updateDraft(draftId: d, fromAccountId: from, to: to, cc: cc, bcc: bcc, subject: subject, body: outgoing))
        }
        return d
    }

    // MARK: draft autosave

    /// How long the typing must rest before the draft is saved to the core. No ledger row exists for it (`grep -i draft docs/CONSTANTS.md`
    /// has none for typing); a private number of this lane, listed in docs/lanes/app-small-faults.md. Short enough that a closed laptop
    /// loses at most a second of words; long enough that a burst of keys is one save.
    static let draftPauseMs = 1000

    /// Starts watching the text, the subject and the recipients: the next change starts the pause. One watch at a time; it re-arms itself
    /// after each change while the composer is open. Never runs in the key's turn: the change only starts a timer.
    func watchEdits() {
        guard !autosaveWatching else { return }
        autosaveWatching = true
        withObservationTracking {
            _ = bodyText; _ = subject; _ = to; _ = cc; _ = bcc; _ = fromId
        } onChange: { [weak self] in
            Task { @MainActor in
                guard let self else { return }
                self.autosaveWatching = false
                self.edited()
                if self.isOpen { self.watchEdits() }
            }
        }
    }

    /// A change: the pause starts again; when it ends the draft is saved in the background.
    func edited() {
        guard isOpen, phase == .editing else { return }
        autosavePause?.cancel()
        autosavePause = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .milliseconds(Self.draftPauseMs))
            guard !Task.isCancelled, let self else { return }
            await self.autosave(refresh: false)
        }
    }

    /// Window closed or keyboard lost: no pause, the save starts at once (still after this turn, never inside the key).
    @discardableResult
    func saveNow(refresh: Bool) -> Task<Void, Never> {
        autosavePause?.cancel()
        return Task { @MainActor [weak self] in await self?.autosave(refresh: refresh) }
    }

    /// One save at a time. A request while one runs sets a flag; the running save goes round again and writes the latest text.
    func autosave(refresh: Bool) async {
        if autosaveRunning { autosaveAgain = true; return }
        autosaveRunning = true
        var again = false
        repeat {
            autosaveAgain = false
            await autosaveGate?()
            persistDraft(AppModel.shared)
            again = autosaveAgain
        } while again
        autosaveRunning = false
        if refresh { AppModel.shared.refresh() }
    }

    /// The write itself: the first one saves a new draft and keeps its core id (the send then updates that draft, never a second one);
    /// later ones update it. Nothing is written when nothing changed, when the message is empty or untouched (a reply with only its
    /// prefilled To and subject), or once the send has taken it. A refused background save says nothing: Send says why on its own turn.
    private func persistDraft(_ m: AppModel) {
        guard draftKey != nil, phase == .editing || isFailed else { return }
        // Round 20 (owner 2026-11-13 07:29, an empty "You · Draft" card under the thread): a REPLY with no words and no file is never kept;
        // one kept earlier whose words were all removed goes. A new message keeps its To and subject as before.
        if kind.canPopIn, bodyText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, fileTiles.isEmpty {
            if let d = coreDraftId { _ = try? m.core.run(command: .deleteDraft(draftId: d)); coreDraftId = nil; autosaveSaved = "" }
            return
        }
        let touched = !bodyText.isEmpty || to != seedTo || subject != seedSubject || !cc.isEmpty || !bcc.isEmpty
        guard touched || coreDraftId != nil else { return }
        let state = [fromId, to.joined(separator: ","), cc.joined(separator: ","), bcc.joined(separator: ","), subject, outgoing].joined(separator: "\u{1}")
        guard state != autosaveSaved else { return }
        do {
            if try saveOnCoreDraft(m) == nil { coreDraftId = try saveNewDraft(m) }
            autosaveSaved = state
            autosaveWrites += 1
        } catch {}
    }

    /// The id the core's draft command will return once it exists; until then one stable id per open draft.
    var draftId: String { "draft-" + (draftKey ?? "new") }

    /// Z inside the window: the entry is cancelled and the composer is back with the same text (J3.6).
    @discardableResult
    public func undo(_ m: AppModel) -> Bool {
        guard let id = outboxId else { return false }
        let carried = outboxSources(id)                // read before the cancel: the entry takes its files with it
        guard (try? m.core.run(command: .cancelSend(outboxId: id))) != nil else { return false }
        #if os(macOS)
        AttachmentPlaces.afterSend[id] = nil           // the files are the draft's again: not removed after a send that did not happen
        #endif
        phase = .editing
        restoreFiles(carried)                          // the undo gives the files back to the draft
        if markedDone { markedDone = false; _ = m.perform(.undo) }
        m.refresh()
        return true
    }

    /// The core is the truth about the entry: gone from the open list means it was cancelled (the shell's Z, another
    /// device), and a state other than undoable means the window closed. The view asks every 200 ms while counting.
    public func reconcile(_ m: AppModel) {
        guard let id = outboxId else { return }
        guard let e = m.core.outbox().first(where: { $0.id == id }) else {
            // the open list no longer holds the entry: it left (the Sent list holds its draft) or it was cancelled elsewhere (J6.7: the core's
            // window ended before the bar's own; reading that as a cancel reopened the composer after a send that went out)
            if wentOut(m) { expire(m) } else { phase = .editing }
            return
        }
        if case .undoable = e.state { return }
        expire(m)
    }

    /// True when the Sent list holds the draft of this send's entry.
    private func wentOut(_ m: AppModel) -> Bool {
        guard let d = entryDraftId else { return false }
        let q = InboxQuery(tabId: "", accountId: nil, labelId: nil, mailbox: .sent, accountIds: [], filters: [], smartFolderId: nil)
        return m.core.inbox(query: q).contains { $0.messageId == d }
    }

    /// The pasted pictures this entry carries are removed once it has gone out (the engine reads a file at send, not before).
    func keepPastedUntilSent(_ entry: String, _ m: AppModel) {
        #if os(macOS)
        let pasted = (fileTiles.compactMap(\.url) + pendingAdds).filter(AttachmentPlaces.isPasted)   // tiles and pictures in the text
        if !pasted.isEmpty { AttachmentPlaces.afterSend[entry] = pasted }
        AttachmentPlaces.sweepSent(m.core)
        #endif
    }

    /// The window ran out with the send live: the bar says "Sent", the text is gone, the composer closes after a beat.
    public func expire(_ m: AppModel) {
        guard outboxId != nil else { return }
        phase = .sent
        markedDone = false
        #if os(macOS)
        AttachmentPlaces.sweepSent(m.core)             // pasted pictures of sends that have gone out
        #endif
        discardDraft()
        m.refresh()
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(WhooshConstants.READER_CROSSFADE_MS * 10))
            if self.phase == .sent { self.isOpen = false; if m.surface == ComposerIds.surface { m.surface = "mac-main" }; self.hideWindow() }
        }
    }

    @discardableResult
    public func retry(_ m: AppModel) -> Bool {
        guard case .failed = phase else { return false }
        phase = .editing
        return send(m)
    }

    /// Send later: a time in words (J3.8). The entry goes to Scheduled with that time.
    @discardableResult
    public func schedule(_ date: Date, _ m: AppModel) -> Bool {
        if !toInput.isEmpty { _ = commitRecipient() }
        guard canSend else { line = ComposerWords.nothingToSend; return true }
        sendLaterOpen = false; datePicking = false
        let words = TimeInWords.describe(date, now: Date())
        do {
            // Only the core's own draft can be scheduled: the one files already made (its text updated), else a saved one. A reply saves as
            // a reply to its thread; `draft-<thread>` is a stand-in id the core does not know (par-composermac-r6, reply-actions r2).
            let scheduled: String
            if let d = try saveOnCoreDraft(m) { scheduled = d } else { scheduled = try saveNewDraft(m) }
            let entry = try m.core.run(command: .schedule(draftId: scheduled, atMs: Int64(date.timeIntervalSince1970 * 1000))).createdId
            if let entry { keepPastedUntilSent(entry, m) }   // a send later keeps its pasted pictures until it has gone out
            phase = .scheduled(words: ComposerWords.scheduled(words))
            line = ComposerWords.scheduled(words)
            m.refresh()
        } catch {
            line = Self.words(error)
            lineDetail = Self.details(error).map { (line, $0) }
        }
        return true
    }

    /// The engine's own words when the sentence shown is the catalogue's (they go under Details); nil when the words shown are the engine's own.
    static func details(_ error: Error) -> String? {
        if case CoreError.Invalid(let field, let w) = error, ComposerWords.refusal(field) != nil, !w.isEmpty { return w }
        return nil
    }

    static func words(_ error: Error) -> String {
        switch error {
        case CoreError.Action(let f): return f.reasonWords
        case CoreError.Invalid(let field, let w): return ComposerWords.refusal(field) ?? w
        default: return "\(error)"
        }
    }

    func hideWindow() {
        #if os(macOS)
        let window = WindowBridge.window(for: ComposerContext.slot)
        window?.orderOut(nil)
        WindowKeyReturn.afterHide(of: window)   // the keyboard goes to the window under it at once (finding 109: letters typed right after were lost)
        #endif
    }
}
