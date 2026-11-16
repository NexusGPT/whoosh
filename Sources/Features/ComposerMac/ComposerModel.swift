import Foundation
import Observation
#if os(macOS)
import AppKit
#endif

/// The Mac composer's state: who it is from and to, the subject, the quoted text, where a send stands and the line
/// that says why something did not work. The body lives in the shared rich-text session (RichTextFeature.session),
/// never here. Replies go out through `quickSend`; a new message needs the core's draft commands (see the lane notes).
/// Escape closes the window and keeps the draft in this model; the same message or a new one reopens it as it was.
@MainActor @Observable
public final class ComposerModel {
    public static let shared = ComposerModel()

    public enum Phase: Equatable {
        case editing
        case counting(outboxId: String, countdown: UndoCountdown)
        case sent
        case failed(words: String)
        case scheduled(words: String)
    }

    /// Where a file of the draft stands, as its tile says it. `adding` is the one state the composer keeps itself (the tile is
    /// there at once); the others are the core's own words for the file, read from `draft/<id>/attachments`.
    public enum FileTile: String, Equatable, Sendable { case adding, ready, willDownload = "will_download", gone, tooLarge = "too_large" }

    /// One tile. From the core's read: `id` is the core's attachment id, `url` the file on this Mac (nil for a forwarded file the
    /// core fetches itself). While being added the tile is a local stand-in with `state == .adding` and the id `adding:<path>`.
    public struct Attachment: Equatable, Identifiable {
        public let id: String
        public let name: String
        public let url: URL?
        public let sizeBytes: UInt64
        public let state: FileTile
        public var sizeWords: String { ByteCountFormatter.string(fromByteCount: Int64(sizeBytes), countStyle: .file) }
    }

    public internal(set) var isOpen = false
    public internal(set) var accounts: [Account] = []
    public var fromId = ""
    public var to: [String] = []
    public var cc: [String] = []
    public var bcc: [String] = []
    public var toInput = ""
    public var ccInput = ""
    public var bccInput = ""
    public var showCc = false
    public var showBcc = false
    public var subject = ""
    public internal(set) var threadId: String?
    internal(set) var quoted: [ComposerQuote] = []
    public var quotedFolded = true
    public internal(set) var phase: Phase = .editing
    public internal(set) var line = ""
    /// The engine's own words behind a catalogue sentence in `line` (the Details tooltip); it counts only while `line` is still that sentence.
    var lineDetail: (line: String, details: String)?
    var lineHelp: String? { lineDetail.flatMap { $0.line == line ? $0.details : nil } }
    /// The tiles: the core's read of the draft's files, then the files still being added. Rebuilt by `rebuildTiles`; never edited by hand.
    public internal(set) var attachments: [Attachment] = []
    /// The core's draft once there is one (the first attach saves it; a forward starts with one). Distinct from `draftId` (Send later's stand-in id).
    var coreDraftId: String?
    var fileTiles: [Attachment] = []
    var pendingAdds: [URL] = []
    var pendingRemovals: [String] = []
    /// RIG ONLY (`mac-composer.dev.hold-files`): the adds and removals wait, so a check can read the "being added" state.
    var filesHeld = false
    public var aiFieldOpen = false
    public var aiInput = ""
    public internal(set) var aiText: String?
    public internal(set) var aiBusy = false
    public var rewriteOpen = false
    public var sendLaterOpen = false
    public var fromOpen = false
    public var templatesOpen = false
    public var snippetsOpen = false
    public var instantOpen = false
    public var moreSendOpen = false
    /// Round 4: the remind-me-if-no-reply panel, and the time the owner chose (kept until the send, then given to the core).
    public var remindOpen = false
    public internal(set) var reminderAt: Date?
    public internal(set) var instantOptions: [String] = []
    // Keys and fields (round 2): where the focus ring sits, and the panels the keys open.
    /// The field the focus ring is on. Set on the key's own frame; the view moves the real focus after.
    public var ringField: ComposerField?
    /// True when the ring slides to its new row, false when it fades (docs/MOTION.md rule 11).
    public internal(set) var ringSlide = true
    public internal(set) var focusRequest: ComposerFocusRequest?
    var focusSerial = 0
    /// When a key last asked for a field (system uptime, seconds): a focus report the view sends back inside one popover
    /// duration that names another field than the key asked for is the old field letting go, not a click (`focusFollows`).
    var keyFocusAt: TimeInterval = 0
    /// Round 6: the date line of the Send later and Remind panels ("Pick a date…", its picker and its button). It lives here so a
    /// click door reaches it the way the button does; only one of the two panels is open at a time.
    public var datePicking = false
    public var dateChosen = Date().addingTimeInterval(3600 * 24)
    public var dateRefusal: String?
    public var attachOpen = false
    public var discardAsk = false
    public var linkOpen = false
    public var linkInput = ""
    public var colorOpen = false
    var listIndex = 0
    var inlineDismissed: RichTrigger?
    public var inlineIndex = 0
    /// the message a reply answers, so "send and mark done" can archive it
    public internal(set) var replyMessageId: String?
    /// who the draft is for (thread id, "fwd:<message id>" or "new"); Escape keeps it, the same key reopens it
    var draftKey: String?
    /// Round 3 (pop-out windows): what the window holds, the To and subject it opened with (a change to either is work worth
    /// keeping), and the message a forward quotes.
    public internal(set) var kind: ComposerKind = .new
    var seedTo: [String] = []
    var seedSubject = ""
    var seedBody = ""
    var forwardOf: String?
    var replySuggestion: ComposerSuggestion?
    var markedDone = false
    /// Bumped by the dictation controller on every change, so the view redraws its state and live words.
    public internal(set) var dictationRevision = 0
    var sentSnapshot: (text: String, to: [String], subject: String)?
    /// The draft id of the outbox entry this send made, read when its window opened (the Sent list names a sent entry by it).
    var entryDraftId: String?
    var writer: any ComposerWriter = NoProviderWriter()
    @ObservationIgnored var observer: NSObjectProtocol?
    /// Draft autosave (ComposerModel+Send): the pause that restarts on every edit, the save that runs (one at a time), the flag that asks the
    /// running one for another round with the latest text, what the core holds now, and how many writes there were (read by the unit tests).
    @ObservationIgnored var autosavePause: Task<Void, Never>?
    @ObservationIgnored var autosaveRunning = false
    /// Counts the times a door was refused because the draft in the window has work (canReplaceDraft): a refused door key was taken.
    @ObservationIgnored var refusals = 0
    @ObservationIgnored var autosaveAgain = false
    @ObservationIgnored var autosaveWatching = false
    @ObservationIgnored var autosaveSaved = ""
    @ObservationIgnored var autosaveWrites = 0
    /// Test seam: awaited at the start of each save round, so a test can change the text while a save is running.
    @ObservationIgnored var autosaveGate: (@MainActor () async -> Void)?
    @ObservationIgnored var resignObserver: NSObjectProtocol?

    public var session: RichTextSession { RichTextFeature.session }
    public var bodyText: String { session.document.plainText }
    public var from: Account? { accounts.first { $0.id == fromId } }
    public var signature: String { ComposerContext.signature(address: from?.address ?? "") }
    var library: ComposerLibrary { .shared }
    var hasDraft: Bool { !bodyText.isEmpty || !subject.isEmpty || !to.isEmpty }
    var anyPanelOpen: Bool {
        aiFieldOpen || rewriteOpen || sendLaterOpen || fromOpen || templatesOpen || snippetsOpen || instantOpen || moreSendOpen || remindOpen
            || attachOpen || discardAsk || linkOpen || colorOpen
    }
    var isFailed: Bool { if case .failed = phase { return true }; return false }
    public var isCounting: Bool { if case .counting = phase { return true }; return false }
    public var outboxId: String? { if case .counting(let id, _) = phase { return id }; return nil }
    var completions: [ComposerSuggestion] { offered(.to) }

    /// The people offered under a recipient field for the words typed in it, without those already in To, Cc or Bcc. One engine read per change of the
    /// words: the list is kept until they change (the view asks for it several times a frame).
    func offered(_ field: RecipientField) -> [ComposerSuggestion] {
        let typed = input(field).trimmingCharacters(in: CharacterSet(charactersIn: " ,;"))   // the comma that takes a recipient is not one of its letters
        let taken = to + cc + bcc
        let key = "\(field)|\(typed)|\(replySuggestion?.address ?? "")|\(taken.joined(separator: ","))"
        if let kept = completionKept, kept.key == key { return kept.list }
        let list = ComposerContext.suggestions(for: typed, replyTo: field == .to ? replySuggestion : nil, core: CoreHost.shared.core)
            .filter { s in !taken.contains { $0.lowercased() == s.address.lowercased() } }
        completionKept = (key, list)
        return list
    }
    @ObservationIgnored private var completionKept: (key: String, list: [ComposerSuggestion])?

    func input(_ field: RecipientField) -> String { field == .to ? toInput : field == .cc ? ccInput : bccInput }

    /// The highlighted line of the list under a recipient field; -1: none (Return then takes the address as typed).
    var completionIndex = 0
    /// The owner chose the line with the arrows.
    var completionMoved = false

    /// A whole address as typed ("ann@a.co"): Return may take it as it stands.
    static func isAddress(_ s: String) -> Bool {
        s.range(of: #"^[^@\s]+@[^@\s]+\.[A-Za-z]{2,}$"#, options: .regularExpression) != nil
    }

    /// The words of a recipient field changed: the first line is highlighted again. A whole address that is nobody's of the list highlights no line,
    /// so Return takes what was typed and never a longer address that only starts like it.
    func recipientTyped(_ field: RecipientField) {
        completionMoved = false
        let raw = input(field).trimmingCharacters(in: CharacterSet(charactersIn: " ,;"))
        let list = offered(field)
        if let exact = list.firstIndex(where: { $0.address.lowercased() == raw.lowercased() }) { completionIndex = exact }
        else { completionIndex = Self.isAddress(raw) ? -1 : 0 }
    }

    /// ↑ ↓ in a recipient field while its list shows. False when there is no list (the key is the field's own).
    func moveCompletion(_ delta: Int, _ field: RecipientField) -> Bool {
        let n = min(offered(field).count, 5)
        guard n > 0 else { return false }
        completionIndex = min(max(completionIndex + delta, 0), n - 1)
        completionMoved = true
        return true
    }

    /// The line Return and Tab take: the highlighted one of the first five.
    func highlightedCompletion(_ field: RecipientField) -> ComposerSuggestion? {
        let list = Array(offered(field).prefix(5))
        return list.indices.contains(completionIndex) ? list[completionIndex] : nil
    }

    /// Tab in a recipient field with a highlighted line: that person becomes a chip and the cursor stays in the field. False: Tab moves on as ever.
    func takeCompletion(_ field: RecipientField) -> Bool {
        guard !input(field).trimmingCharacters(in: .whitespaces).isEmpty, highlightedCompletion(field) != nil else { return false }
        return commitRecipient(field)
    }

    /// The AI mark stays while the body is exactly the AI's words; any edit clears it (J3.3).
    public var aiMarkActive: Bool { if let t = aiText { return bodyText == t }; return false }

    init() {
        #if os(macOS)
        observer = NotificationCenter.default.addObserver(forName: .dictationText, object: nil, queue: .main) { n in
            let text = n.userInfo?["text"] as? String ?? ""
            MainActor.assumeIsolated { ComposerModel.shared.insertDictated(text) }
        }
        // the composer window loses the keyboard: the draft is saved (a click in the mail, another app, the window closed)
        resignObserver = NotificationCenter.default.addObserver(forName: NSWindow.didResignKeyNotification, object: nil, queue: .main) { n in
            let id = (n.object as? NSWindow).map { ObjectIdentifier($0) }   // a plain value is made here: the notification must not cross into the main-actor block
            MainActor.assumeIsolated { ComposerModel.shared.windowLostKeyboard(id) }
        }
        #endif
    }

    // MARK: open and close

    #if os(macOS)
    /// The composer's window lost the keyboard: the draft is saved (in the background).
    func windowLostKeyboard(_ window: ObjectIdentifier?) {
        guard isOpen, let window, let mine = WindowBridge.window(for: ComposerContext.slot), ObjectIdentifier(mine) == window else { return }
        saveNow(refresh: true)
    }
    #endif

    /// A blank message, From the account the list is filtered to, else the first account. A draft Escape kept comes back.
    @discardableResult
    public func openNew(_ m: AppModel) -> Bool {
        if draftKey != "new" {
            guard canReplaceDraft(m) else { return false }
            reset(m); threadId = nil; replyMessageId = nil; replySuggestion = nil; draftKey = "new"
            kind = .new; seedTo = []; seedSubject = ""; seedBody = ""; forwardOf = nil
        }
        guard present(m) else { return false }
        return focus(to.isEmpty ? .to : .body)       // a new message starts at the recipient
    }

    /// A reply: From the thread's receiving account, To its sender, the quoted text folded (J3.1).
    @discardableResult
    public func openReply(_ m: AppModel, messageId: String? = nil) -> Bool {
        guard let id = messageId ?? m.shownMessageId, let r = ComposerContext.reply(messageId: id, core: m.core) else { return false }
        let thread = (m.shownRow?.messageId == id ? m.shownRow?.threadId : m.rows.first { $0.messageId == id }?.threadId) ?? r.threadId
        if draftKey != thread {
            guard canReplaceDraft(m) else { return false }
            reset(m)
            kind = .reply; forwardOf = nil
            threadId = thread
            replyMessageId = id
            fromId = r.accountId
            to = r.toAddresses.isEmpty ? [r.toAddress] : r.toAddresses
            replySuggestion = ComposerSuggestion(name: r.toName, address: r.toAddress)
            subject = r.subject.lowercased().hasPrefix("re:") ? r.subject : ComposerWords.replyPrefix + r.subject
            quoted = r.quoted
            draftKey = thread
            seedTo = to; seedSubject = subject; seedBody = ""
        }
        guard present(m) else { return false }
        return focus(.body)                          // a reply starts in the text
    }

    func reset(_ m: AppModel) {
        accounts = ComposerContext.accounts()
        fromId = m.accountFilter.flatMap { id in accounts.contains { $0.id == id } ? id : nil } ?? accounts.first?.id ?? ""
        to = []; cc = []; bcc = []; toInput = ""; ccInput = ""; bccInput = ""; showCc = false; showBcc = false; subject = ""; quoted = []; quotedFolded = true
        phase = .editing; line = ""; reminderAt = nil; dropCoreDraft(); aiFieldOpen = false; aiInput = ""; aiText = nil; aiBusy = false
        closePanels(); instantOptions = []; sentSnapshot = nil; ringField = nil; inlineDismissed = nil
        session.reset()
    }

    func closePanels() {
        aiFieldOpen = false; rewriteOpen = false; sendLaterOpen = false; fromOpen = false
        templatesOpen = false; snippetsOpen = false; instantOpen = false; moreSendOpen = false; remindOpen = false
        attachOpen = false; discardAsk = false; linkOpen = false; colorOpen = false
        datePicking = false; dateRefusal = nil
    }

    /// The panel that is open, in a word for the control socket ("" when none is).
    var panelWord: String {
        if aiFieldOpen { return "ai" }
        if rewriteOpen { return "rewrite" }
        if sendLaterOpen { return "send-later" }
        if fromOpen { return "from" }
        if templatesOpen { return "templates" }
        if snippetsOpen { return "snippets" }
        if instantOpen { return "instant" }
        if moreSendOpen { return "more" }
        if remindOpen { return "remind" }
        if attachOpen { return "attach" }
        if discardAsk { return "discard" }
        if linkOpen { return "link" }
        if colorOpen { return "color" }
        return ""
    }

    /// The composer takes the shell's window slot for its surface (WindowBridge), as every surface window does.
    func present(_ m: AppModel) -> Bool {
        isOpen = true
        watchEdits()
        m.typed = ""
        if !WindowBridge.open(ComposerContext.slot, in: m) { m.surface = ComposerIds.surface }
        return true
    }

    /// `fixture reset` (the test engine's verb): the composer lets go of what it holds and of any save it has waiting, so nothing of it is written into
    /// the core after the reset restored it. The window itself is closed by the reset.
    func resetToLaunch() {
        autosavePause?.cancel(); autosavePause = nil
        discardDraft()
        isOpen = false
    }

    /// The draft is spent (sent or discarded): the next open starts clean.
    func discardDraft() { autosaveSaved = ""; draftKey = nil; reminderAt = nil; coreDraftId = nil; fileTiles = []; pendingAdds = []; pendingRemovals = []; rebuildTiles(); session.reset() }

    /// Escape: close a popover first, then the composer. A counting send is not closed (undo it or let it go).
    @discardableResult
    public func close(_ m: AppModel) -> Bool {
        guard isOpen else { return false }
        if let t = session.triggerState?.trigger, !inlineItems.isEmpty { inlineDismissed = t; return true }
        if anyPanelOpen {
            let format = linkOpen || colorOpen
            closePanels()
            if format { session.hooks.focusBody() }
            return true
        }
        if isCounting { return false }
        _ = saveNow(refresh: true)                   // the window closes: what is typed is saved to the core (in the background)
        isOpen = false
        if m.surface == ComposerIds.surface { m.surface = "mac-main" }
        hideWindow()
        return true
    }

    // MARK: recipients, subject, attachments

    public enum RecipientField { case to, cc, bcc }

    /// Return or comma in the To field: the typed address (or the first suggestion) becomes a chip.
    @discardableResult
    public func commitRecipient(_ field: RecipientField = .to) -> Bool {
        let typed = input(field)
        let raw = typed.trimmingCharacters(in: CharacterSet(charactersIn: " ,;"))
        guard !raw.isEmpty else { return false }
        // the highlighted person of the field's list, else the address as typed
        let pick = highlightedCompletion(field)?.address ?? raw
        guard pick.contains("@") else { line = ComposerWords.notAnAddress(raw); return false }
        switch field {
        case .to: to.append(pick); toInput = ""
        case .cc: cc.append(pick); ccInput = ""
        case .bcc: bcc.append(pick); bccInput = ""
        }
        completionIndex = 0; completionMoved = false
        line = ""
        return true
    }

    public func removeRecipient(_ address: String) { to.removeAll { $0 == address }; cc.removeAll { $0 == address }; bcc.removeAll { $0 == address } }

    // MARK: dictation and typing

    /// Dictation finished: the cleaned words go in at the caret (J3.2). The audio is the dictation lane's to delete.
    func insertDictated(_ text: String) {
        guard isOpen, !text.isEmpty else { return }
        insertAtCaret(text)
    }

    /// Text typed through the socket's `type` verb or any non-editor source lands at the caret.
    func insertAtCaret(_ text: String) {
        #if os(macOS)
        // The composer's own window first: a rig (or a second display) may leave another window key.
        let window = WindowBridge.window(for: ComposerContext.slot) ?? (NSApp as NSApplication?)?.keyWindow
        if let tv = window?.firstResponder as? NSTextView, tv.isEditable {
            // One undo step of its own (dictated words, a snippet): typing before or after it is not merged into it, so ⌘Z after typing
            // takes back the typing only (main92: one ⌘Z removed the inserted text and the typing together, NSTextView's typing coalescing).
            tv.breakUndoCoalescing()
            tv.insertText(text, replacementRange: NSRange(location: NSNotFound, length: 0))
            tv.breakUndoCoalescing()
            return
        }
        #endif
        let b = bodyText
        session.setPlain(b.isEmpty ? text : b + text)
    }
}
