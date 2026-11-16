import Foundation
import Observation
#if os(macOS)
import AppKit
#endif

/// The quick reply's own state: which thread it answers, its identity, and where the send stands.
/// The text is the field's own (`draft`): another field (search, the palette) never reads or clears it, so a reply
/// survives ⌘K and back. The socket's `type` reaches it through QuickReplyFeature.typeText while it holds the keys.
/// Round 5 (board cards-s3, "Lifted until it lands"): the field is a floating glass slab with a header line that opens
/// to To, Cc, Cci and Objet (`header`); an edited header sends through a draft (`Command.send`, contract delta 12), a
/// plain reply through quickSend; a send the server refuses becomes a failed card in the thread, never a line in the slab.
@MainActor @Observable
public final class QuickReplyModel {
    public static let shared = QuickReplyModel()

    public enum Phase: Equatable {
        case closed
        case editing
        case counting(outboxId: String, countdown: UndoCountdown)
        /// The core refused the command itself (no outbox entry): the words show in the slab with Retry.
        case failed(words: String)
    }

    public internal(set) var phase: Phase = .closed   // the header-actions extension (another file) sets it too
    public private(set) var threadId: String?
    /// The message the reply answers (its sender is the To line, its subject the Objet row).
    public private(set) var replyMessageId: String?
    public private(set) var context = ReplyContext(from: "", signature: "", placeholder: QuickReplyCopy.placeholderFallback)
    /// The reply being written: the field's own text.
    public var draft = ""
    public var emojiOpen = false
    /// The field holds the keyboard focus (the draft card collapses to one line then).
    public var fieldFocused = false
    /// Moves on every door that must put the cursor in the reply's text (R, a draft card opened, an edit of a failed send): the bar takes the
    /// keyboard on each move, also when it was already drawn (hid-compose round 20, owner 07:28: "when hitting R I should have cursor INSIDE
    /// the answer ... it stays on list").
    public internal(set) var focusSerial = 0
    /// The reply was given the keyboard (R, Return on a row, Z after a cancelled send, a draft reopened, a click in its text) and the owner has
    /// not left it (a send, Esc, a click outside the reply). While it is true a typed key is never lost: the shell's key monitor puts the cursor
    /// back in the reply's text before the key is delivered (`ReplyKeyboard.reclaim`). Lead, 13 Nov (owner 07:28 "when hitting R I should have
    /// cursor INSIDE the ansser, and here I cannot type"; the reply check on main124: the mail's body took the keyboard after R and "456" was lost).
    public internal(set) var wantsKeys = false
    /// The reply box is drawn: open, and no send of its own in its window (owner, 13 Nov, picture 206: one object is shown once; the sending
    /// card alone carries the countdown and Cancel Z, the box folds at the send).
    public var showsBox: Bool { isOpen && outboxId == nil }

    /// Asks for the keyboard: the state first, then the view's own focus ask.
    func askKeys() { wantsKeys = true; focusSerial += 1 }
    /// The saved draft the field has open (its id in the core), and the draft just deleted (its card offers Undo).
    public internal(set) var openDraftId: String?
    public internal(set) var lastDeletedDraft: String?
    /// The conversation whose draft was deleted last: the line "Draft deleted · Undo Z" is that conversation's, and it goes at the next act on
    /// the reply there (a reply opened, a send). Lead, 13 Nov (walk s126): the line stayed under a later send, a second Z beside "Cancel Z",
    /// and it was drawn on whichever conversation was on screen.
    public internal(set) var lastDeletedDraftThread: String?
    /// The line shows on this conversation.
    public func showsDeletedLine(threadId: String?) -> Bool { lastDeletedDraft != nil && lastDeletedDraftThread != nil && lastDeletedDraftThread == threadId }
    var drafts: DraftSource = CoreDraftSource()
    /// Moves on every draft save or delete made here: the list rows read it to redraw their draft mark.
    public internal(set) var draftsRevision = 0
    /// The From picker: every account the core lists, the sender checked, the typed filter and the ↑↓ highlight.
    public var fromOpen = false
    public var fromFilter = ""
    public var fromHighlight = 0
    public private(set) var accounts: [AccountSummary] = []
    var threadAccountId = ""
    /// The text that went into the outbox, so Undo hands it back to the field.
    private var sentText = ""
    /// Moves on every send that opens an undo window (and on `fixture reset`): a window's timer acts only while its number is still the newest.
    var sendSerial = 0
    var source: ReplyContextSource = CoreReplyContext()
    /// The slab's header: one line, or the open To / Cc / Cci / Objet rows.
    public let header = QuickReplyHeaderState()
    /// The AI reply in the thread (⌘J, the draft's mark, the change field): QuickReplyAIState, acts in QuickReplyAIActions.
    public let ai = QuickReplyAIState()
    /// The slab's drawn height, so the thread keeps that much room under its last card (the cards never sit under it).
    public var slabHeight: CGFloat = 0

    public var isOpen: Bool { phase != .closed }
    public var outboxId: String? { if case .counting(let id, _) = phase { return id }; return nil }

    /// R: open the focused thread and the field under it, From its receiving address.
    public func open(_ m: AppModel) -> Bool {
        // From the search's results (the keys on them, the reader showing the selected one): the result opens first, as Return does, so the
        // search keeps its results and Esc later comes back to them (lead, 13 Nov, owner A162: R on a result typed into the search words).
        let search = SearchMacModel.shared
        if m.surface == SearchMacModel.surfaceId, search.showsResults, !search.holds, !search.onMore { guard search.openResult() else { return false } }
        guard let row = m.shownRow else { return false }
        let id = row.messageId
        return openReply(threadId: row.threadId, messageId: id, replyTo: row.senderName, m)
    }

    /// The reply for one message's thread, whoever the list has focused (R above, the notch's door in QuickReplyOpen).
    func openReply(threadId tid: String, messageId id: String, replyTo: String, _ m: AppModel, moveSurface: Bool = true) -> Bool {
        settle(m)
        if outboxId != nil { expire(m) }   // a send still in its window: its card keeps the countdown and Cancel; the box is free for the next reply
        if isOpen, threadId != tid { keepAsDraft(m); openDraftId = nil }   // the words stay with their thread
        _ = try? m.core.run(command: .openThread(threadId: tid))
        threadId = tid
        replyMessageId = id
        context = source.context(threadId: tid, messageId: id, replyTo: replyTo, core: m.core)
        accounts = m.core.accounts().map(AccountNames.localised)
        threadAccountId = context.accountId
        fromOpen = false; fromFilter = ""; fromHighlight = 0
        if moveSurface { m.surface = QuickReplyIds.surface }   // false: the caller (the notch) keeps the main window where it is
        if case .counting = phase {} else { draft = ""; phase = .editing }
        emojiOpen = false
        lastDeletedDraft = nil   // a reply is being written here: the "Draft deleted" line has had its moment
        loadHeader(m, messageId: id, replyTo: replyTo)
        if let to = header.to.first?.name, !to.isEmpty { context.placeholder = QuickReplyCopy.placeholder(to) }   // one name in the header and in the field
        resumeSavedDraft(m)   // one reply draft per conversation: R continues the saved one (QuickReplyDrafts)
        aiOpened(m)   // the thread's live AI draft (auto or asked) shows in the empty field with its mark
        askKeys()
        return true
    }

    /// The From control: open or close the menu of sending accounts (not while a send is counting down).
    public func toggleFrom() -> Bool {
        guard phase == .editing || isFailed else { return false }
        fromOpen.toggle()
        if fromOpen {
            emojiOpen = false; fromFilter = ""
            fromHighlight = accounts.firstIndex { $0.id == context.accountId } ?? 0
        }
        return true
    }

    /// A pick in the From menu: the sender shown changes on the same frame and the menu closes.
    public func pickAccount(_ id: String) -> Bool {
        guard fromOpen, phase == .editing || isFailed, let a = accounts.first(where: { $0.id == id }) else { return false }
        context.from = a.address
        context.accountId = a.id
        fromOpen = false
        return true
    }

    /// ⌘↵ (also ⌃↵ and Enter): an empty reply is refused; otherwise the text goes to the outbox and the undo window opens. A header the owner
    /// edited (To, Cc, Cci, Objet) travels through a saved draft and `Command.send` (delta 12); a plain reply through
    /// quickSend. In the fixture core an armed `fixture fail-next <op> <reason…> [code]` fails the queued entry instead,
    /// so the thread shows the failed card (board state "failure"), not a line in the slab.
    public func send(_ m: AppModel) -> Bool {
        settle(m)
        aiBeforeSend(m)   // a shown AI draft becomes an ordinary draft first; the AI path never sends
        guard let threadId, phase == .editing || isFailed else { return false }
        let text = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return false }
        // A word on Cc or Cci that is not an address stops the send, with its sentence under the line. Until 13 Nov 2026 the word was dropped and the
        // reply left without that copy, with no word to the owner.
        if let stray = header.strayWord { header.refuse(stray); return true }
        let body = context.signature.isEmpty ? text : text + "\n\n" + context.signature
        let account = context.accountId.isEmpty ? nil : context.accountId
        if m.host.kind == .fixture, let armed = FixtureFaults.take(header.isExpandedSend ? "send" : FixtureFaults.op(of: plainCommand(threadId, body))) {
            let (reason, code) = Self.splitCode(armed)
            guard let fx = m.core as? FixtureCore, fx.failNext("send", reason: reason, code: code) else { phase = .failed(words: armed); return true }
        }
        do {
            let outcome: CommandOutcome
            if header.isExpandedSend {
                // The From line's account sends (owner 11:45); saveDraft takes no Bcc, so a Cci lands by the update (delta 12).
                let fields = DraftFields(to: header.to.map(\.address), cc: header.addresses(header.cc), bcc: header.addresses(header.bcc),
                                         subject: header.subject.trimmingCharacters(in: .whitespaces))
                let draftId: String
                if let open = openDraftId {
                    try drafts.update(draftId: open, accountId: account, fields: fields, body: body, core: m.core)
                    draftId = open
                } else {
                    guard let created = try drafts.save(threadId: threadId, accountId: account, fields: fields, body: body, core: m.core) else {
                        throw CoreError.Invalid(field: "send", reasonWords: QuickReplyCopy.failedFallback)
                    }
                    if !fields.bcc.isEmpty { try drafts.update(draftId: created, accountId: account, fields: fields, body: body, core: m.core) }
                    draftId = created
                }
                outcome = try m.core.run(command: .send(draftId: draftId))
                openDraftId = nil   // the draft left the list with the send (Z brings it back)
            } else {
                outcome = try m.core.run(command: plainCommand(threadId, body))
                if let d = openDraftId { try? drafts.delete(draftId: d, core: m.core); openDraftId = nil }   // sent: no longer a draft
            }
            guard let created = outcome.createdId else { throw CoreError.Invalid(field: "send", reasonWords: QuickReplyCopy.failedFallback) }
            sentText = draft
            draft = ""
            emojiOpen = false; fromOpen = false
            header.reset()
            draftsRevision += 1
            if let e = m.core.outbox().first(where: { $0.id == created }), case .failed = e.state {
                // The failed card holds the words and offers Retry, Edit and Delete: the box folds (one object, shown once; lead, 13 Nov, walk
                // t126: the box stayed open and empty under the "Not sent" card). R starts a new reply, ⌘E brings these words back.
                phase = .closed
            } else {
                let window = countdown(for: created, m)
                phase = .counting(outboxId: created, countdown: window)
                // The timer acts for its own send only: the outbox id alone is not enough, as the test engine numbers its entries from 1 again
                // after `fixture reset` (lead, 16 Nov, lane send-timer: a send's timer from before a reset folded the next send with the same id).
                sendSerial += 1
                let serial = sendSerial
                DispatchQueue.main.asyncAfter(deadline: .now() + window.total + 0.05) { [weak self] in
                    MainActor.assumeIsolated { if let self, self.sendSerial == serial, self.outboxId == created { self.expire(AppModel.shared) } }
                }
            }
            letGoOfKeys()
            m.refresh()
            return true
        } catch CoreError.Action(let failure) {
            phase = .failed(words: failure.reasonWords)
        } catch CoreError.Invalid(let field, let words) {
            let shown = Self.refusalWords(field: field, engine: words)
            phase = .failed(words: shown.words)
            failedDetail = shown.details.map { (shown.words, $0) }
        } catch {
            phase = .failed(words: QuickReplyCopy.failedFallback)
        }
        return true   // the key did something: the slab now says why, and keeps the text
    }

    /// The words are in the outbox: the reply's text gives the keyboard back, in the same turn. The keys shown beside the send (Cancel Z) and
    /// every key of the list and the reader then act, and nothing more is typed into a box that reads "Sending in N s". Lead, 13 Nov (owner
    /// A162, walks on main116 and main118): after a send from a search result the text kept the keyboard and "e", "z", "/" were typed under
    /// the countdown; from the inbox list it lost the keyboard only because the shell moved the pane focus. Only the main window's own text view
    /// is asked (never a field's editor, never another window: the notch has its own card).
    func letGoOfKeys() {
        fieldFocused = false; wantsKeys = false
        #if os(macOS)
        if let w = WindowBridge.mainWindow(), let text = w.firstResponder as? NSTextView, !text.isFieldEditor { w.makeFirstResponder(nil) }
        #endif
    }

    /// A144: a field the app has a sentence for shows that sentence, with the engine's words behind Details; any other field shows the engine's words.
    static func refusalWords(field: String, engine: String) -> (words: String, details: String?) {
        guard let sentence = ComposerWords.refusal(field) else { return (engine, nil) }
        return (sentence, engine.isEmpty ? nil : engine)
    }

    /// The thread's own account through quickSend, a picked other one through quickSendFrom (contract delta 10), so the
    /// outbox entry, and the card, carry the address shown.
    func plainCommand(_ threadId: String, _ body: String) -> Command {
        context.accountId == threadAccountId || context.accountId.isEmpty
            ? Command.quickSend(threadId: threadId, body: body)
            : Command.quickSendFrom(threadId: threadId, body: body, fromAccountId: context.accountId)
    }

    /// "<reason words> 451 4.7.1" → the words and the server code the fixture arms (delta 12 `ActionFailed.code`).
    static func splitCode(_ armed: String) -> (String, String?) {
        var words = armed.split(separator: " ").map(String.init)
        var code: [String] = []
        while let last = words.last, Self.looksLikeCode(last), code.count < 2 { code.insert(last, at: 0); words.removeLast() }
        guard let first = code.first, first.count == 3, first.allSatisfy(\.isNumber) else { return (armed, nil) }
        return (words.joined(separator: " "), code.joined(separator: " "))
    }

    private static func looksLikeCode(_ s: String) -> Bool {
        (s.count == 3 && s.allSatisfy(\.isNumber)) || (s.count == 5 && s.filter(\.isNumber).count == 3 && s.filter { $0 == "." }.count == 2)
    }

    /// The window the outbox crate granted this entry (until minus created), not a number of our own.
    private func countdown(for id: String, _ m: AppModel) -> UndoCountdown {
        guard let e = m.core.outbox().first(where: { $0.id == id }), case .undoable(let until) = e.state, until > e.createdAtMs else {
            return UndoCountdown(startedAt: Date())
        }
        return UndoCountdown(startedAt: Date(), seconds: Int((until - e.createdAtMs) / 1000))
    }

    /// Undo inside the window: the send is cancelled and the text comes back to the field.
    public func undo(_ m: AppModel) -> Bool {
        guard let id = outboxId else { return false }
        let restored = m.core.outbox().first { $0.id == id }?.draft.id
        guard (try? m.core.run(command: .cancelSend(outboxId: id))) != nil else { return false }
        draft = sentText
        phase = .editing
        // hid-compose round 20 (owner 2026-11-13 07:26, "impossible to delete"): the real engine puts a cancelled send back among the
        // drafts (core/crates/outbox/src/usecases/cancel_send.rs: DraftEffect::Restore), a new copy each time, while these same words came
        // back to the field. The thread then drew that copy as a draft card under the open reply, one more per send and undo, and Delete
        // took one copy off while the next showed the same words. The field now holds that draft: no card while the reply is open, the
        // next keep updates it, a send deletes it. The fixture core restores nothing, so no draft is found there and nothing changes.
        if let r = restored, let held = restoredDraft(r, thread: threadId, m) { openDraftId = held }
        draftsRevision += 1
        // Lead, 13 Nov (owner A162, walk on main117): Z brought the words back but left the keys on the list, so the next letters ran
        // list commands. A cancelled send is cancelled to change the words: the cursor goes back into them.
        askKeys()
        m.refresh()
        return true
    }

    public func retry(_ m: AppModel) -> Bool {
        if isFailed { phase = .editing; return send(m) }
        return retryNewestFailed(m)
    }

    /// The window closed with the send still live: the message is on its way and the field is free again.
    public func expire(_ m: AppModel) {
        guard outboxId != nil else { return }
        sentText = ""
        draft = ""   // nothing can be typed in a folded box; a stray character must never bring it back open
        // The reply is sent: its box stays folded (it folded at the send). Before 13 Nov the box came back open and empty here.
        phase = draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? .closed : .editing
        if phase == .closed { wantsKeys = false; emojiOpen = false; fromOpen = false; header.reset() }
        m.refresh()
    }

    public func close(_ m: AppModel) -> Bool {
        settle(m)
        guard isOpen, outboxId == nil else { return false }
        keepAsDraft(m)
        openDraftId = nil   // the field is closing: no draft is open in it any more, so its card shows again
        phase = .closed; wantsKeys = false
        emojiOpen = false
        fromOpen = false
        header.reset()
        return true
    }

    /// The test copy's reset (`fixture reset`): the reply box as at launch, nothing kept (the test engine goes back to its seed in the same
    /// reset). Without it the box stayed open across a reset and, since it holds the keyboard as a state (`wantsKeys`), took the next case's keys
    /// (lead, 13 Nov, par-macshell on main136: Enter after a reset went into the reply left open by the case before).
    public func resetToLaunch() {
        draft = ""; phase = .closed; wantsKeys = false
        sendSerial += 1   // a timer armed before the reset never acts on a send after it
        openDraftId = nil
        lastDeletedDraft = nil; lastDeletedDraftThread = nil
        emojiOpen = false; fromOpen = false
        header.reset()
        ai.clear()
    }

    /// The address the core filed on the outbox entry of the send in its window (the sender actually used).
    func outboxSender(_ m: AppModel) -> String {
        guard let id = outboxId, let e = m.core.outbox().first(where: { $0.id == id }) else { return "" }
        return accounts.first { $0.id == e.draft.fromAccountId }?.address ?? e.draft.fromAccountId
    }

    public func insert(_ emoji: String, _ m: AppModel) -> Bool {
        guard phase == .editing || isFailed else { return false }
        draft += emoji
        emojiOpen = false
        return true
    }

    var isFailed: Bool { if case .failed = phase { return true }; return false }
    var failureWords: String? { if case .failed(let w) = phase { return w }; return nil }
    /// The engine's own words behind the catalogue sentence of a failed send (the Details tooltip); counts only while that sentence shows.
    var failedDetail: (words: String, details: String)?
    var failureDetails: String? { if case .failed(let w) = phase, let d = failedDetail, d.words == w { return d.details }; return nil }
}
