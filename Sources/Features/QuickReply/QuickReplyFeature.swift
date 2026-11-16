import SwiftUI

/// The quick reply lane's registrations: the slab's rows (field, header and its To/Cc/Cci/Objet rows, From and its
/// picker, send, the countdown line, Retry, the emoji row, the AI pill and its two fields), the keys R · Enter · ⌘Enter · ⌃Enter ·
/// ⌘Enter · ⇧H · ⇧⌘F · ⇧⌘P · ⌘J · ↑ ↓ ↩ in the picker · Esc, and every click. The views live in this folder (QuickReplyBar) for the reader to mount.
@MainActor
public enum QuickReplyFeature: FeatureModule {
    public static let id = "quickreply"
    public static let surfaces: [String] = ["mac-main", QuickReplyIds.shotSurface]
    static var model: QuickReplyModel { .shared }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        ThreadCardsRegistry.rows().merging(fieldRows()) { a, _ in a }.merging(headerRows()) { a, _ in a }
            .merging(QuickReplyAIRegistry.rows()) { a, _ in a }
    }

    private static func fieldRows() -> [String: (AppModel) throws -> [ControlRow]] {
        [
            QuickReplyIds.field: { m in
                guard model.isOpen else { return [] }
                let c = model.context
                return [["ax": QuickReplyIds.field, "thread": model.threadId ?? "", "from": c.from, "signature": c.signature,
                         "placeholder": c.placeholder, "text": model.draft, "phase": phaseWord, "emoji_open": model.emojiOpen ? "1" : "0",
                         "header_open": model.header.open ? "1" : "0", "slab_height": Int(model.slabHeight)]]
            },
            QuickReplyIds.from: { _ in
                guard model.isOpen else { return [] }
                return [["ax": QuickReplyIds.from, "address": model.context.from, "account": model.context.accountId, "open": model.fromOpen ? "1" : "0",
                         "cap": QuickReplyCopy.fromCap]]
            },
            QuickReplyIds.fromMenu: { _ in
                guard model.fromOpen else { return [] }
                return model.pickerAccounts.enumerated().map { i, a in
                    ["ax": QuickReplyIds.fromItem(i), "account": a.id, "name": a.name, "address": a.address,
                     "checked": a.id == model.context.accountId ? "1" : "0", "highlighted": i == model.fromHighlight ? "1" : "0"]
                }
            },
            // doors-compose: the picker's filter field; the socket's `type` lands in it while the picker is open (typeText below).
            QuickReplyIds.fromFilter: { _ in
                guard model.fromOpen else { return [] }
                return [["ax": QuickReplyIds.fromFilter, "text": model.fromFilter, "shown": model.pickerAccounts.count]]
            },
            QuickReplyIds.send: { _ in
                guard model.isOpen else { return [] }
                let on = model.outboxId == nil && !model.draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                return [["ax": QuickReplyIds.send, "cap": QuickReplyCopy.sendCap, "help": QuickReplyCopy.sendHelp, "word": QuickReplyCopy.sendWord, "enabled": on ? "1" : "0"]]
            },
            // the notch card's send control: its cap, read like the slab's (spec A145; par-notch reads it)
            NotchIds.send: { _ in
                guard model.isOpen, NotchModel.shared.openItem != nil else { return [] }
                let on = model.outboxId == nil && !model.draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                return [["ax": NotchIds.send, "cap": NotchReply.sendCap, "help": NotchCopy.send, "enabled": on ? "1" : "0"]]
            },
            QuickReplyIds.undoBar: { m in
                guard case .counting(_, let countdown) = model.phase else { return [] }
                return [["ax": QuickReplyIds.undoBar, "seconds": countdown.secondsLeft(at: Date()), "message": QuickReplyCopy.sending, "sender": model.outboxSender(m),
                         "undo": QuickReplyIds.undo, "undo_cap": QuickReplyCopy.cancelCap]]
            },
            QuickReplyIds.retry: { m in
                // The slab's own refusal (no outbox entry), else the thread's newest failed card: one Retry either way.
                if let words = model.failureWords { return [["ax": QuickReplyIds.retry, "words": words, "where": "slab"]] }
                guard let t = model.threadId ?? ThreadCards.shownThread(m),
                      let e = m.core.outbox().last(where: { $0.draft.replyToThreadId == t && { if case .failed = $0.state { return true }; return false }($0) }),
                      case .failed(let failure, _) = e.state else { return [] }
                return [["ax": QuickReplyIds.retry, "words": failure.reasonWords, "code": failure.code ?? "", "where": "card", "card": e.id]]
            },
            QuickReplyIds.emojiPicker: { _ in
                guard model.emojiOpen else { return [] }
                return QuickReplyEmoji.set.enumerated().map { i, e in ["ax": QuickReplyIds.emojiItem(i), "emoji": e] }
            },
        ]
    }

    /// The people offered under the reply's Cc or Cci line, as drawn.
    private static func offerRows(_ field: QuickReplyHeaderState.Field, _ name: String, _ m: AppModel) -> [ControlRow] {
        guard model.isOpen, model.header.open else { return [] }
        return model.header.offered(field, core: m.core).enumerated().map {
            ["ax": QuickReplyIds.headerOffer(name, $0.offset), "name": $0.element.name, "address": $0.element.address, "highlighted": $0.offset == model.header.offerIndex]
        }
    }

    private static func headerRows() -> [String: (AppModel) throws -> [ControlRow]] {
        [
            QuickReplyIds.header: { _ in
                guard model.isOpen else { return [] }
                let h = model.header
                var row: ControlRow = ["ax": QuickReplyIds.header, "title": model.headerTitle, "open": h.open ? "1" : "0", "cap": QuickReplyCopy.headerCap,
                                       "toggle": QuickReplyIds.headerToggle, "popout": QuickReplyIds.popout, "popout_cap": QuickReplyCopy.popoutCap,
                                       "counting": countingWord]
                if h.open {
                    row["to"] = h.to.map(\.address).joined(separator: ", "); row["cc"] = h.cc; row["bcc"] = h.bcc; row["subject"] = h.subject
                    row["focus"] = focusWord(h.focus)
                    row["refusal"] = h.refusal ?? ""
                }
                return [row]
            },
            QuickReplyIds.headerOffers("cc"): { m in offerRows(.cc, "cc", m) },
            QuickReplyIds.headerOffers("bcc"): { m in offerRows(.bcc, "bcc", m) },
            QuickReplyIds.headerToRegion: { _ in
                guard model.isOpen, model.header.open else { return [] }
                return model.header.to.enumerated().map { i, r in
                    ["ax": QuickReplyIds.headerTo(i), "name": r.name, "address": r.address, "remove": QuickReplyIds.headerToRemove(i)]
                }
            },
        ]
    }

    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        #if os(macOS)
        ReplyKeyboard.installOnce()   // the reply's hold on the keyboard is a state the shell's key monitor honours
        #endif
        let sendKey: (AppModel) -> Bool = { m in model.isOpen ? model.send(m) : (ThreadCardsRegistry.sendShownDraft(m) || model.retryNewestFailed(m)) }
        return [
            // Plain Return never sends (owner 2026-11-13 07:27, "hitting enter actually sends", "not cmd+enter"; it replaces the 17:19 rule
            // "Enter, ⌘↵, ⌃↵ or the button"). In the reply's text it adds a line, as the field's own Return does; the picker's Return picks
            // and an AI field's Return runs its instruction (neither sends mail). Only ⌘↵, ⌃↵ and the Send button send.
            "Enter": { m in model.fromOpen ? model.pickHighlighted() : (model.aiEnter(m) || newLineKey(m)) },
            // ⌘↵ is the key every reply send control shows (owner 2026-11-11); ⌃↵ keeps sending, unshown. Both take one path: the
            // open field sends, else the shown draft card sends, else the newest failed card retries.
            "⌘Enter": sendKey,
            "⌃Enter": sendKey,
            // ⌘⌫ on a draft card or a failed card (owner, 13 Nov, picture 206: "cmd+del not just del it would clash": plain ⌫ deletes the open
            // mail everywhere else). A focused text keeps ⌘⌫ for itself (TextKeys: delete to the start of the line), so it never lands here.
            "⌘Backspace": { m in !(model.isOpen && model.fieldFocused) && ThreadCardsRegistry.deleteShownCard(m) },
            // Esc is the reply's only while it holds the keys: the picker closes first, then the slab.
            // Esc is the reply's only for what the reply can do with it: close its picker, an AI field, then the slab. While a send counts down the
            // slab cannot close, and the key GOES ON to the screen behind it (back to the search's results, back to the list), with the send still
            // in its window. Before 13 Nov the reply answered with the shell's base Escape itself and no other screen was asked: after a send
            // from a search result Esc left the results listed with the keys nowhere (lead, walk f118).
            "Escape": { m in
                guard holdsKeys(m) else { return false }
                if model.closeFrom() || model.aiEscape(m) { return true }
                // Lead, 13 Nov (owner 07:27 "impossible to go back to list with esc", A162; real-engine walk on main121): after a send landed the
                // reply sits open, empty and without the cursor, and the first Esc only folded it: one more press before the list or the
                // results. A reply with no word that does not hold the keyboard folds AND lets the same Esc go back.
                if model.foldIfIdle(m) { return false }
                return model.close(m)
            },
            // ⌘J: the AI draft (reply-thread). The quick reply's own key in every preset; Gmail and Apple Mail also reach it as a
            // context meaning of their ⌘J (accounts), which answers when no thread is open (aiKey says false then).
            "⌘J": { model.aiKey($0) },
            // ⌘E: Edit on the thread's newest failed send (round 20); nothing when no send of the thread failed, so the key goes on.
            "⌘E": { model.editNewestFailed($0) },
            "⇧H": { m in holdsKeys(m) && model.toggleHeader() }, "Shift+H": { m in holdsKeys(m) && model.toggleHeader() },
            "⇧⌘F": { m in holdsKeys(m) && model.toggleFrom() },
            "⇧⌘C": { m in holdsKeys(m) && model.openHeader(at: .cc) }, "⇧⌘B": { m in holdsKeys(m) && model.openHeader(at: .bcc) },
            "⇧⌘P": { m in holdsKeys(m) && model.popOut(m) },
            "Up": { _ in model.fromOpen && model.moveFromHighlight(-1) },
            "Down": { _ in model.fromOpen && model.moveFromHighlight(1) },
        ]
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        var t: [String: (AppModel) -> Bool] = [
            QuickReplyIds.send: { model.send($0) },
            QuickReplyIds.undo: { model.undo($0) },
            QuickReplyIds.from: { _ in model.toggleFrom() },
            QuickReplyIds.retry: { model.retry($0) },
            QuickReplyIds.emoji: { _ in
                guard model.phase == .editing || model.isFailed else { return false }
                model.emojiOpen.toggle(); return true
            },
            QuickReplyIds.headerToggle: { _ in model.toggleHeader() },
            QuickReplyIds.popout: { model.popOut($0) },
            QuickReplyIds.headerCc: { _ in model.focusHeader(.cc) },
            QuickReplyIds.headerBcc: { _ in model.focusHeader(.bcc) },
            QuickReplyIds.headerSubject: { _ in model.focusHeader(.subject) },
            QuickReplyIds.headerOffer("cc", 0): { m in model.isOpen && model.header.takeOffer(0, .cc, core: m.core) },
            QuickReplyIds.headerOffer("cc", 1): { m in model.isOpen && model.header.takeOffer(1, .cc, core: m.core) },
            QuickReplyIds.headerOffer("cc", 2): { m in model.isOpen && model.header.takeOffer(2, .cc, core: m.core) },
            QuickReplyIds.headerOffer("cc", 3): { m in model.isOpen && model.header.takeOffer(3, .cc, core: m.core) },
            QuickReplyIds.headerOffer("cc", 4): { m in model.isOpen && model.header.takeOffer(4, .cc, core: m.core) },
            QuickReplyIds.headerOffer("bcc", 0): { m in model.isOpen && model.header.takeOffer(0, .bcc, core: m.core) },
            QuickReplyIds.headerOffer("bcc", 1): { m in model.isOpen && model.header.takeOffer(1, .bcc, core: m.core) },
            QuickReplyIds.headerOffer("bcc", 2): { m in model.isOpen && model.header.takeOffer(2, .bcc, core: m.core) },
            QuickReplyIds.headerOffer("bcc", 3): { m in model.isOpen && model.header.takeOffer(3, .bcc, core: m.core) },
            QuickReplyIds.headerOffer("bcc", 4): { m in model.isOpen && model.header.takeOffer(4, .bcc, core: m.core) },
            QuickReplyIds.field: { _ in guard model.isOpen else { return false }; model.header.focus = .none; model.ai.focus = .reply; return true },
        ]
        t.merge(ThreadCardsRegistry.clicks()) { a, _ in a }
        t.merge(QuickReplyAIRegistry.clicks()) { a, _ in a }
        #if os(macOS)
        if AttachmentPlaces.isRig { t["mac-main.quickreply.dev.paste"] = { model.rigPaste($0) } }   // rig only: the field's ⌘V of the rig's pasteboard
        #endif
        // One open-by-thread target per thread the list or the open thread names (the notch's door, for checks).
        for tid in Set(AppModel.shared.rows.map(\.threadId)) {
            t[QuickReplyIds.openThread(tid)] = { model.open(threadId: tid, $0) }
            t[QuickReplyIds.openThreadStay(tid)] = { model.open(threadId: tid, $0, moveSurface: false) }
        }
        // Positional ids resolve when the click lands (the picker's order and the To line change under them).
        for i in 0..<UndoDepthBound.cards {
            t[QuickReplyIds.fromItem(i)] = { _ in
                let rows = model.pickerAccounts
                return rows.indices.contains(i) && model.pickAccount(rows[i].id)
            }
            t[QuickReplyIds.headerToRemove(i)] = { _ in model.removeRecipient(i) }
        }
        for (i, e) in QuickReplyEmoji.set.enumerated() { t[QuickReplyIds.emojiItem(i)] = { model.emojiOpen && model.insert(e, $0) } }
        return t
    }

    /// Keys reach the field first while it is open (the shell asks the focused feature first; a handler passes with false).
    /// Also while the open thread shows a draft or a failed card, so their ⌘↵ and ⌫ caps work; every other key passes (false).
    public static func hasFocus(_ m: AppModel) -> Bool {
        model.isOpen || m.surface == QuickReplyIds.shotSurface
            || (m.surface == QuickReplyIds.surface && (ThreadCardsRegistry.hasShownDraft(m) || hasFailedCard(m)))
    }

    private static func hasFailedCard(_ m: AppModel) -> Bool {
        guard let t = ThreadCards.shownThread(m) else { return false }
        return ThreadCards.outgoing(threadId: t).contains { if case .failed = $0.state { return true }; return false }
    }

    /// `type` lands in the draft while the field holds the keys: the reply is open, not sending, and no other field
    /// (search on `mac-search`, the palette on `mac-palette`) has taken them. The picker's filter and a clicked header
    /// row (Cc, Cci, Objet) take it first.
    public static func typeText(_ text: String, _ m: AppModel) -> Bool {
        model.settle(m)   // a send whose window the clock closed frees the field before the key lands
        guard holdsKeys(m), model.outboxId == nil else { return false }
        if model.fromOpen { model.fromFilter += text; model.fromHighlight = 0; return true }
        if model.header.open, model.header.type(text) { return true }
        if model.aiType(text) { return true }
        model.draft += text
        model.aiTextChanged(m)   // typing over the AI's words makes them the owner's draft
        return true
    }

    /// Return while the reply's text holds the keys: one line break, nothing else (hid-compose round 20). False while a header row has the
    /// keys or a send counts, so the key goes on.
    static func newLineKey(_ m: AppModel) -> Bool {
        model.settle(m)
        // only while the reply's text holds the keyboard: an open reply left unfocused (after a send, after a click on the thread) takes no Return,
        // so Return there stays the list's or the reader's (lead, 13 Nov: it added an unseen empty line to the open reply instead)
        guard holdsKeys(m), model.fieldFocused, model.outboxId == nil, model.header.focus == .none else { return false }
        model.draft += "\n"
        return true
    }

    /// The reply is open and no other field (search on `mac-search`, the palette on `mac-palette`) has the keys.
    static func holdsKeys(_ m: AppModel) -> Bool {
        model.isOpen && [QuickReplyIds.surface, SurfaceSlot.macMain.id, QuickReplyIds.shotSurface].contains(m.surface)
    }

    /// The reader column at the foot of which the slab floats, in a window of the shell's own surface scene.
    public static func view(for slot: SurfaceSlot) -> AnyView? {
        slot.id == QuickReplyIds.shotSurface ? AnyView(QuickReplyColumn()) : nil
    }

    public static func paletteCommands() -> [(title: String, key: String)] {
        [(Words.text("Quick reply"), "R"), (Words.text("Send quick reply"), "⌘Enter"),
         (Words.text("Expand header (⇧H)"), "⇧H"), (Words.text("Send from (⇧⌘F)"), "⇧⌘F"), (Words.text("Pop out (⇧⌘P)"), "⇧⌘P")]
    }

    private static var phaseWord: String {
        switch model.phase {
        case .closed: return "closed"
        case .editing: return "editing"
        case .counting: return "counting"
        case .failed: return "failed"
        }
    }

    /// "1" while the slab's header is the countdown: a window the clock already closed reads "0" even before a view tick.
    private static var countingWord: String {
        guard case .counting(_, let c) = model.phase else { return "0" }
        return c.isExpired(at: Date()) ? "0" : "1"
    }

    private static func focusWord(_ f: QuickReplyHeaderState.Field) -> String {
        switch f { case .none: return "field"; case .cc: return "cc"; case .bcc: return "bcc"; case .subject: return "subject" }
    }
}

/// R and Z arrive through the shortcuts lane's preset (CommandRegistry): this screen provides "reply" (open the
/// field under the focused thread) and "undo" (cancel a quick send in its window, else the shell's undo).
extension QuickReplyFeature: CommandProviding {
    public static func commands() -> [String: (AppModel) -> Bool] {
        [
            "reply": { model.open($0) },
            "ai-reply": { model.aiKey($0) },   // ⌘J reached through a preset (Gmail, Apple Mail context keys)
            // Z after a deleted draft is that line's own Undo (it also clears the "Draft deleted" line's memory, which a bare undo left set)
            "undo": { m in model.outboxId != nil ? model.undo(m) : (model.showsDeletedLine(threadId: ThreadCards.shownThread(m)) ? model.undoDraftDelete(m) : m.perform(.undo)) },
            // Lead, 13 Nov (owner 07:26 "impossible to delete", A162): the strip under an open reply reads "⇧⌘, Discard" and only the composer
            // window answered that key, so in the reply it did nothing.
            "discard-draft": { m in holdsKeys(m) && model.discard(m) },
        ]
    }
}
