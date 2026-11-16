import Foundation
#if os(macOS)
import AppKit
#endif

/// Round 5 actions of the slab (board cards-s3): the header line and its rows, the From picker's keys, the pop-out to
/// the composer, and the two actions of a failed card. Each flips state on the key frame; the views only follow.
extension QuickReplyModel {
    /// The To line is the person answered, Objet is "Re: <subject>", read from the message the reply answers.
    func loadHeader(_ m: AppModel, messageId: String, replyTo: String) {
        guard let r = ComposerContext.reply(messageId: messageId, core: m.core) else {
            header.load(to: [], subject: "")
            return
        }
        let subject = r.subject.lowercased().hasPrefix("re:") ? r.subject : ComposerWords.replyPrefix + r.subject
        // Lead, 13 Nov (owner A162): the name is the person the reply GOES to. A reply to a mail the owner sent goes to its recipient, so the
        // sender's name (the owner's own, or none) is not that person's: the contact's name, else the address itself.
        let toSender = ReplyToRule.same(r.toAddress, m.core.message(messageId: messageId)?.fromAddress ?? "")
        let name = !r.toName.isEmpty ? r.toName : toSender ? replyTo : (ComposerContext.contactName(r.toAddress, core: m.core) ?? "")
        let first = ReplyRecipient(name: name.isEmpty ? r.toAddress : name, address: r.toAddress)
        let more = r.toAddresses.dropFirst().map { ReplyRecipient(name: "", address: $0) }
        header.load(to: [first] + more, subject: subject)
    }

    /// The header's chevron, or ⇧H: the To / Cc / Cci / Objet rows open or fold (no row while a send counts down).
    public func toggleHeader() -> Bool {
        guard phase == .editing || isFailed else { return false }
        header.toggle()
        return true
    }

    /// × on a To chip.
    public func removeRecipient(_ i: Int) -> Bool {
        guard header.open, header.remove(at: i) else { return false }
        return true
    }

    /// A click on a header row: the socket's `type` lands there until the field is clicked again.
    /// ⇧⌘C and ⇧⌘B (the preset's Cc and Bcc keys, the full composer's too): the header opens when it is folded and that row takes the cursor.
    /// They work from the reply's own text, where ⇧H is a typed capital letter (lead, 13 Nov, real-key audit on main118: the header's ⇧H key,
    /// drawn beside "Reply to …", typed "H" into the reply).
    public func openHeader(at field: QuickReplyHeaderState.Field) -> Bool {
        guard phase == .editing || isFailed else { return false }
        if !header.open { guard toggleHeader() else { return false } }
        return focusHeader(field)
    }

    public func focusHeader(_ field: QuickReplyHeaderState.Field) -> Bool {
        guard header.open else { return false }
        header.focus = field
        return true
    }

    /// The header's one-line title: "Répondre à <x>" while the field is empty, "Brouillon à <x>" with words (owner 23:04).
    var headerTitle: String {
        let name = header.to.first?.name ?? ""
        return draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? QuickReplyCopy.replyTo(name) : QuickReplyCopy.draftTo(name)
    }

    // MARK: the From picker's keys

    /// The accounts the picker lists, in the core's order (a pick never moves a row, so a positional id stays put);
    /// the filter narrows by name or address; the highlight opens on the current sender.
    var pickerAccounts: [AccountSummary] {
        let p = fromFilter.trimmingCharacters(in: .whitespaces).lowercased()
        return p.isEmpty ? accounts : accounts.filter { $0.name.lowercased().contains(p) || $0.address.lowercased().contains(p) }
    }

    /// ↑ ↓ in the picker.
    public func moveFromHighlight(_ by: Int) -> Bool {
        guard fromOpen else { return false }
        let n = pickerAccounts.count
        guard n > 0 else { return false }
        fromHighlight = (fromHighlight + by + n) % n
        return true
    }

    /// ↩ in the picker: the highlighted account sends.
    public func pickHighlighted() -> Bool {
        guard fromOpen, pickerAccounts.indices.contains(fromHighlight) else { return false }
        return pickAccount(pickerAccounts[fromHighlight].id)
    }

    /// Esc in the picker: closed, the sender as it was.
    public func closeFrom() -> Bool {
        guard fromOpen else { return false }
        fromOpen = false
        return true
    }

    // MARK: pop-out

    /// ⇧⌘P: the words and the header move to the composer window; the slab closes and keeps no draft of its own.
    public func popOut(_ m: AppModel) -> Bool {
        guard isOpen, outboxId == nil, let messageId = replyMessageId else { return false }
        aiBeforeSend(m)   // the AI's words leave as the owner's: the reply draft reads Edited
        let composer = ComposerModel.shared
        guard composer.openReply(m, messageId: messageId) else { return false }
        if header.isExpandedSend {
            composer.to = header.to.map(\.address)
            composer.cc = header.addresses(header.cc)
            composer.bcc = header.addresses(header.bcc)
            composer.subject = header.subject
        }
        if !context.accountId.isEmpty { composer.fromId = context.accountId }
        let words = draft
        draft = ""   // the composer holds them now: closing saves no second copy (an open draft with no words is removed)
        if let open = openDraftId { try? drafts.delete(draftId: open, core: m.core); openDraftId = nil; draftsRevision += 1 }
        phase = .closed
        emojiOpen = false; fromOpen = false
        header.reset()
        if !words.isEmpty { composer.session.setPlain(composer.bodyText.isEmpty ? words : composer.bodyText + words) }
        m.refresh()
        return true
    }

    #if os(macOS)
    // MARK: files into the reply (owner 2026-11-12: "copy-paste of file should add attachment")

    /// The reply holds no files: it sends through QuickSend, text only, and has no core draft of its own to hold them. A file pasted or
    /// dropped on it goes on in the full composer, in the same frame: the reply pops out (its words, recipients and From travel, as
    /// ⇧⌘P does) and the files are attached there, on the composer's own tile row.
    @discardableResult
    func takeFiles(_ urls: [URL], _ m: AppModel) -> Bool {
        guard !urls.isEmpty, popOut(m) else { return false }
        urls.forEach { AttachmentIntake.take($0) }
        return true
    }

    /// Files dropped on the field: the reply pops out and the drop lands on the composer, as a drop on its window does.
    func dropFiles(_ providers: [NSItemProvider], _ m: AppModel) -> Bool {
        guard !providers.isEmpty, popOut(m) else { return false }
        return AttachmentIntake.drop(providers)
    }

    /// The field's text changed. A paste of copied files puts their NAMES in a text field (the system's TextEditor reads the file's
    /// name as text): when the inserted text is exactly the pasteboard's text and the pasteboard holds files, the names are taken back out
    /// and the files are attached instead. One typed key (one character) never reads the pasteboard.
    @discardableResult
    func takePastedFiles(old: String, new: String, _ m: AppModel) -> Bool {
        guard new.count > old.count + 1, let inserted = Self.inserted(old: old, new: new) else { return false }
        let pb = AttachmentPasteboard.board
        let urls = (pb.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL]) ?? []
        guard !urls.isEmpty, let names = pb.string(forType: .string),
              inserted.trimmingCharacters(in: .whitespacesAndNewlines) == names.trimmingCharacters(in: .whitespacesAndNewlines) else { return false }
        if AttachmentRig.mutant("reply-paste-as-text") { return false }
        draft = old
        return takeFiles(urls, m)
    }

    /// What one edit put in: `new` minus the prefix and suffix it shares with `old`; nil when the edit removed text only.
    static func inserted(old: String, new: String) -> String? {
        let o = Array(old), n = Array(new)
        var p = 0
        while p < o.count, p < n.count, o[p] == n[p] { p += 1 }
        var s = 0
        while s < o.count - p, s < n.count - p, o[o.count - 1 - s] == n[n.count - 1 - s] { s += 1 }
        let piece = String(n[p..<(n.count - s)])
        return piece.isEmpty ? nil : piece
    }

    /// ⌘V of a picture copied as DATA (no file, no text, no HTML): the system's TextEditor would insert nothing, so the field's paste is
    /// taken here (QuickReplyPasteWatch, while the field holds the keys) and the reply goes on in the full composer with the picture IN
    /// ITS TEXT (lead ruling, round 7: picture data pastes inline; a copied file is a tile).
    @discardableResult
    func takePastedPicture(_ m: AppModel) -> Bool {
        let pb = AttachmentPasteboard.board
        let files = (pb.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL]) ?? []
        guard isOpen, files.isEmpty, pb.string(forType: .string) == nil, pb.string(forType: .html) == nil,
              let data = pb.data(forType: .png) ?? pb.data(forType: .tiff) else { return false }
        if AttachmentRig.mutant("reply-picture-dropped") { return false }
        guard popOut(m) else { return false }
        AttachmentIntake.takeImageInline(data)   // written under the app's data folder, then into the composer's text at the caret
        return true
    }

    /// RIG ONLY (`mac-main.quickreply.dev.paste`): what the field's own ⌘V does with the rig's pasteboard: its text at the end of the
    /// words (a copied file's name), or a picture copied as data. The system's TextEditor reads the owner's pasteboard, which a test never touches.
    func rigPaste(_ m: AppModel) -> Bool {
        guard isOpen else { return false }
        guard let t = AttachmentPasteboard.board.string(forType: .string) else { return takePastedPicture(m) }
        let old = draft
        draft = old + t
        _ = takePastedFiles(old: old, new: draft, m)
        return true
    }
    #endif

    // MARK: a failed card's two actions

    /// Réessayer ⌘↵ on a failed card: the refused entry leaves the outbox and the same draft is queued again, so the
    /// thread shows a fresh waiting card with its ring (the retry of this thread's newest failure when no id is given).
    public func retryFailed(outboxId id: String, _ m: AppModel) -> Bool {
        guard let e = m.core.outbox().first(where: { $0.id == id }), case .failed = e.state else { return false }
        guard (try? m.core.run(command: .cancelSend(outboxId: id))) != nil else { return false }
        let d = e.draft
        dropRestored(d.id, thread: d.replyToThreadId, m)   // round 20: the copy the engine's cancel put back is queued again below, not kept
        let plainReply = d.cc.isEmpty && d.bcc.isEmpty && d.to.count <= 1
        do {
            let outcome: CommandOutcome
            if plainReply, let thread = d.replyToThreadId {
                outcome = try m.core.run(command: d.fromAccountId == threadAccountId || d.fromAccountId.isEmpty
                    ? .quickSend(threadId: thread, body: d.body)
                    : .quickSendFrom(threadId: thread, body: d.body, fromAccountId: d.fromAccountId))
            } else {
                let fields = DraftFields(to: d.to, cc: d.cc, bcc: d.bcc, subject: d.subject)
                guard let saved = try drafts.save(threadId: d.replyToThreadId ?? threadId ?? "", accountId: d.fromAccountId, fields: fields, body: d.body, core: m.core) else { return false }
                if !d.bcc.isEmpty { try drafts.update(draftId: saved, accountId: d.fromAccountId, fields: fields, body: d.body, core: m.core) }
                outcome = try m.core.run(command: .send(draftId: saved))
            }
            guard let created = outcome.createdId else { return false }
            if isOpen, threadId == d.replyToThreadId, phase == .editing || isFailed {
                sendSerial += 1   // its own send: a timer of an earlier send (QuickReplyModel.send) never folds it
                phase = .counting(outboxId: created, countdown: UndoCountdown(startedAt: Date(), seconds: windowSeconds(created, m)))
            }
            m.refresh()
            return true
        } catch {
            return false
        }
    }

    /// Supprimer ⌫ on a failed card: the entry is cancelled and the card goes; nothing else changes.
    public func discardFailed(outboxId id: String, _ m: AppModel) -> Bool {
        guard let e = m.core.outbox().first(where: { $0.id == id }), case .failed = e.state else { return false }
        guard (try? m.core.run(command: .cancelSend(outboxId: id))) != nil else { return false }
        dropRestored(e.draft.id, thread: e.draft.replyToThreadId, m)   // round 20: Delete leaves no draft card behind
        m.refresh()
        return true
    }

    func retryNewestFailed(_ m: AppModel) -> Bool {
        guard let t = threadId ?? ThreadCards.shownThread(m) else { return false }
        guard let last = ThreadCards.outgoing(threadId: t, core: m.core).last(where: { if case .failed = $0.state { return true }; return false }) else { return false }
        return retryFailed(outboxId: last.id, m)
    }

    private func windowSeconds(_ id: String, _ m: AppModel) -> Int {
        guard let e = m.core.outbox().first(where: { $0.id == id }), case .undoable(let until) = e.state, until > e.createdAtMs else {
            return WhooshConstants.UNDO_SEND_DEFAULT_S
        }
        return Int((until - e.createdAtMs) / 1000)
    }
}

#if os(macOS)
/// The reply field's ⌘V for a picture copied as data: one local key monitor, only while the field holds the keys (installed and removed
/// with its focus), that takes ⌘V when `takePastedPicture` does; every other key, and every other paste, passes on untouched.
@MainActor
enum QuickReplyPasteWatch {
    private static var monitor: Any?
    static func watch(_ on: Bool) {
        if on, monitor == nil {
            monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { e in
                guard e.modifierFlags.intersection(.deviceIndependentFlagsMask) == .command, e.charactersIgnoringModifiers?.lowercased() == "v" else { return e }
                return MainActor.assumeIsolated { QuickReplyModel.shared.takePastedPicture(AppModel.shared) } ? nil : e
            }
        } else if !on, let m = monitor {
            NSEvent.removeMonitor(m)
            monitor = nil
        }
    }
}
#endif
