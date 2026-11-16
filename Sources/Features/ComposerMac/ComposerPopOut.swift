import Foundation

/// What a composer window holds. ⇧R opens a reply, ⇧↵ a reply to all, ⇧F a forward, ⇧C a new message.
public enum ComposerKind: String, Sendable {
    case new, reply, replyAll = "reply-all", forward

    /// A reply can go back into its thread's reply box; a new message and a forward have no box to go back to.
    var canPopIn: Bool { self == .reply || self == .replyAll }
}

/// How a composer window opens: its content fades in and rises a few points. One place, so the view draws what the
/// socket row reports. The rise is the ledger's COMPOSER_POPOUT_RISE_PT; the duration is the popover's (POPOVER_MS).
public enum ComposerOpenMotion {
    public static var risePoints: CGFloat { CGFloat(WhooshConstants.COMPOSER_POPOUT_RISE_PT) }
    public static var milliseconds: Int { Int(WhooshConstants.POPOVER_MS) }
    public static let words = "rise+opacity"
    /// The composer's ideal height, and the height its window opens with: the system opens a new window about 450 points tall, where a forward
    /// showed two empty lines and none of the forwarded mail (lead, 13 Nov 2026, picture on main155).
    public static let idealHeight: CGFloat = WhooshSpacing.s32 * 20
}

/// Pop-out windows (plan K10, Superhuman's Pop Out Compose keys). The composer window IS the pop-out: the same
/// ComposerView in the shell's own surface window. The pop-in side is the thread's reply box (QuickReply): ⇧⌘P carries the
/// words, the recipients and the sender between the two, so a draft is in one place at a time and never copied twice.
/// There is one draft window: a draft with work in it is never replaced by another (`canReplaceDraft`).
extension ComposerModel {
    // MARK: the draft is worth keeping

    /// Something would be lost if another draft took the window: words, files, a Cc or Bcc, a typed address, or a To or
    /// subject changed from what the window opened with. A message already given to the outbox (the undo bar is up, it is
    /// sent, or it is scheduled) is not at risk: the core holds it, so the next message may take the window.
    var hasWork: Bool {
        guard phase == .editing || isFailed else { return false }
        return bodyText != seedBody || !attachments.isEmpty || !cc.isEmpty || !bcc.isEmpty || !toInput.isEmpty
            || to != seedTo || subject != seedSubject
    }

    /// A draft with work in it is never replaced by another: its window comes forward and says why, and the key that
    /// wanted a second draft is refused (false), so no door moves words out of the draft that holds them.
    func canReplaceDraft(_ m: AppModel) -> Bool {
        guard draftKey != nil, hasWork else { return true }
        refusals += 1
        line = ComposerWords.finishFirst
        _ = present(m)
        return false
    }

    // MARK: the doors (⇧R, ⇧↵, ⇧F, ⇧C)

    /// ⇧R: the reply opens in its own window. Words already written in the thread's reply box travel along.
    @discardableResult
    public func popOutReply(_ m: AppModel) -> Bool {
        let slab = QuickReplyModel.shared
        if slab.isOpen, slab.outboxId == nil { return slab.popOut(m) }
        return openReply(m)
    }

    /// ⇧↵: the same window, with everyone the message went to: its other To addresses and its Cc, never an address of
    /// the owner's own accounts.
    @discardableResult
    public func popOutReplyAll(_ m: AppModel) -> Bool {
        guard popOutReply(m), let id = replyMessageId, let msg = m.core.message(messageId: id) else { return false }
        let mine = Set(accounts.map { $0.address.lowercased() })
        var seen = Set((to + cc).map { $0.lowercased() })
        func add(_ raw: [String]) -> [String] {
            raw.map(Self.address).filter { a in !mine.contains(a.lowercased()) && seen.insert(a.lowercased()).inserted }
        }
        to += add(msg.to)
        cc += add(msg.cc)
        showCc = showCc || !cc.isEmpty
        kind = .replyAll
        return true
    }

    /// ⇧F: a forward of the focused message: To empty, "Fwd: " in front of the subject, the message's text in the body.
    @discardableResult
    public func popOutForward(_ m: AppModel) -> Bool {
        guard let id = m.shownMessageId ?? replyMessageId ?? forwardOf, let msg = m.core.message(messageId: id) else { return false }
        let key = "fwd:" + id
        if draftKey != key {
            guard canReplaceDraft(m) else { return false }
            reset(m)
            kind = .forward; forwardOf = id
            threadId = nil; replyMessageId = nil; replySuggestion = nil; draftKey = key
            subject = msg.subject.lowercased().hasPrefix("fwd:") ? msg.subject : ComposerWords.forwardPrefix + msg.subject
            seedTo = []; seedSubject = subject
            if accounts.contains(where: { $0.id == msg.accountId }) { fromId = msg.accountId }      // a forward leaves from the account that received it
            let text = m.core.body(messageId: id)?.plainText ?? msg.preview
            session.setPlain(ComposerWords.forwardBody(name: msg.senderName, address: msg.fromAddress, recipients: msg.to, title: msg.subject, text: text, files: msg.attachmentNames))
            seedBody = bodyText                                   // the quoted message alone is not work worth keeping
            // the core makes every forward's draft (the original's files come as tiles, each removable), so a forward the core
            // refuses says so at once, files or not (main95: a forward of a mail with no file never asked the core, the line stayed empty)
            adoptForwardDraft(id)
        }
        guard present(m) else { return false }
        return focus(.to)               // a forward starts at the recipient
    }

    /// ⇧C: a new message in its own window (C opens the same window; the pop-out key is its twin).
    @discardableResult
    public func popOutNew(_ m: AppModel) -> Bool { openNew(m) }

    /// "Karen Bosch <karen@x.example>" -> "karen@x.example"; a bare address stays as it is.
    static func address(_ raw: String) -> String {
        guard let a = raw.firstIndex(of: "<"), let b = raw.firstIndex(of: ">"), a < b else { return raw.trimmingCharacters(in: .whitespaces) }
        return String(raw[raw.index(after: a)..<b])
    }

    // MARK: ⇧⌘P: out and in

    /// The window is out: it goes in. Otherwise the thread's reply box is out: its draft goes to the window.
    @discardableResult
    public func togglePop(_ m: AppModel) -> Bool {
        if isOpen { return popIn(m) }
        let slab = QuickReplyModel.shared
        return slab.isOpen && slab.outboxId == nil && slab.popOut(m)
    }

    /// The window's draft goes back into its thread's reply box: the words, the To, Cc, Bcc and subject, and the sender.
    /// Only a reply can go back. A draft with files stays out: the reply box carries none, and a send from it would
    /// drop them without a word, so the key answers with the reason and keeps everything (`line`).
    @discardableResult
    public func popIn(_ m: AppModel) -> Bool {
        guard isOpen, !isCounting else { return false }
        guard kind.canPopIn, let thread = threadId, replyMessageId != nil else { line = ComposerWords.onlyReplyGoesBack; return true }
        guard attachments.isEmpty else { line = ComposerWords.filesStayOut; return true }
        if !toInput.isEmpty { _ = commitRecipient() }
        let slab = QuickReplyModel.shared
        let words = bodyText
        if slab.isOpen, slab.threadId == thread, slab.outboxId == nil {
            slab.draft = slab.draft.isEmpty ? words : slab.draft + "\n\n" + words      // both are kept, the window's after the box's
        } else {
            guard slab.open(threadId: thread, m) else { return false }
            slab.draft = words
            hand(to: slab, m)
        }
        leaveWindow(m)
        return true
    }

    /// The header the box shows: To chips, Cc, Bcc, subject (opened when it differs from the plain reply), and the sender.
    private func hand(to slab: QuickReplyModel, _ m: AppModel) {
        let people = to.map { ReplyRecipient(name: ComposerContext.contactName($0, core: m.core) ?? "", address: $0) }
        slab.header.restore(to: people, cc: cc, bcc: bcc, subject: subject)
        if slab.header.isExpandedSend, !slab.header.open { slab.header.toggle() }
        if fromId != slab.context.accountId, slab.accounts.contains(where: { $0.id == fromId }) {
            slab.fromOpen = true
            _ = slab.pickAccount(fromId)
            slab.fromOpen = false
        }
    }

    /// The window closes without keeping a second copy: the draft is in the reply box now.
    private func leaveWindow(_ m: AppModel) {
        isOpen = false
        closePanels()
        discardDraft()
        if m.surface == ComposerIds.surface { m.surface = "mac-main" }
        hideWindow()
        #if os(macOS)
        WindowBridge.bringMainToFront()
        #endif
    }

    // MARK: ⌘/ and ⌘D

    /// ⌘/: the draft goes out (or is out already) and the search takes the keys, so a mail can be found while writing.
    @discardableResult
    public func popOutAndSearch(_ m: AppModel) -> Bool {
        let slab = QuickReplyModel.shared
        var out = isOpen
        if !out, slab.isOpen, slab.outboxId == nil { out = slab.popOut(m) }
        let searched = CommandRegistry.handler("search")?(m) ?? false
        return out || searched
    }

    /// ⌘D: between the draft and its thread. From the window it goes to the main window on the draft's thread (the window
    /// stays, the draft is kept); from the main window it brings the window forward; with no window, the thread's
    /// saved draft opens in its reply box.
    @discardableResult
    public func switchDraft(_ m: AppModel) -> Bool {
        if isOpen {
            guard ComposerMacFeature.hasFocus(m) else { return present(m) }
            if let id = replyMessageId ?? forwardOf { m.focus(messageId: id) }
            m.surface = "mac-main"
            #if os(macOS)
            WindowBridge.bringMainToFront()
            #endif
            return true
        }
        return ThreadCardsRegistry.clicks()[ThreadCardIds.draft]?(m) ?? false
    }

    // MARK: the socket row

    /// `rows mac-composer.popout`: whether the window is up, what it holds, the reply box it goes back to (its words, To,
    /// Cc, Bcc, subject), why a key refused (`line`), and how the window opens.
    var popoutRow: [String: Any] {
        let slab = QuickReplyModel.shared
        let h = slab.header
        return ["ax": ComposerIds.popout, "open": isOpen, "window": WindowBridge.isShowing(ComposerContext.slot), "kind": kind.rawValue,
                "title": ComposerWords.title(kind), "to": to, "cc": cc, "subject": subject, "files": attachments.count, "line": line,
                "can_pop_in": kind.canPopIn && attachments.isEmpty,
                "slab_open": slab.isOpen, "slab_text": slab.draft, "slab_to": h.to.map(\.address), "slab_cc": h.addresses(h.cc),
                "slab_bcc": h.addresses(h.bcc), "slab_subject": h.subject,
                "motion": ComposerOpenMotion.words, "rise_pt": Double(ComposerOpenMotion.risePoints), "ms": ComposerOpenMotion.milliseconds]
    }
}
