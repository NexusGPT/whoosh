import Foundation

/// A second implementation of the contract's `WhooshCore`, serving the design fixture in memory.
/// Selected by the launch flag `--fixture-core`. Archive, snooze, pin, label, undo, move and quick
/// send act on the in-memory copy, so the journeys can drive the app with no mail server.
/// The fixture's failing account is acc-18 (Provanta RFP): every command on it fails with ActionFailed.
public final class FixtureCore: WhooshCore, @unchecked Sendable {
    let lock = NSLock()
    let fx: FixtureFile
    var messages: [FixtureFile.Message] {    // the fixture's rows, or a loaded corpus's
        didSet { messageById = Self.index(messages) }
    }
    /// The meaning ranker the search arm asks (delta 15): `MeaningSearch.rank(_:ids:text:)` in the app; a test swaps in a
    /// counting stand-in to prove the arm builds no body before the ranker asks for it.
    nonisolated(unsafe) static var meaningRanker: (String, [String], @escaping @Sendable (String) -> (title: String?, body: String)?) -> [(id: String, score: Float)] = {
        MeaningSearch.rank($0, ids: $1, text: $2)
    }

    /// `messages` by id, kept with it (delta 13): fxMessage was a linear scan, so every list and count over the
    /// full-history fixture (300,000 rows) was quadratic. The first row of an id wins, as `first(where:)` did.
    var messageById: [String: FixtureFile.Message] = [:]
    static func index(_ rows: [FixtureFile.Message]) -> [String: FixtureFile.Message] { Dictionary(rows.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a }) }
    var receivedAt: [String: Int64] = [:]    // message id -> received time, derived once at load
    let clock: FixtureClock
    /// The fixture's failing account; `fixture reauth acc-18` makes it healthy (nil).
    public internal(set) var failingAccountId: String? = "acc-18"

    var order: [String]                    // message ids in fixture order
    var archived: Set<String> = []
    var deleted: Set<String> = []
    var folderRows: [SmartFolder] = []     // smart folders in order: the file's (or the corpus's), then the owner's
    var starred: Set<String> = []
    var muted: Set<String> = []
    var spam: Set<String> = []
    var listsLeft: [String: UnsubscribeMethod] = [:]      // message id -> how its list was left (Unsubscribe, delta 10)
    var listsLeftAt: [String: Int64] = [:]               // message id -> the fixture clock's ms when its list was left (`message/<id>/unsubscribed_at`)
    var filingRefused: [String: String] = [:]            // message id -> the words the filing AI refused with (`fixture filing-refuse`, `message/<id>/filing`)
    var replyReminders: [String: Int64] = [:]             // draft or outbox id -> remind-if-no-reply time (delta 10)
    var replyReminderThread: [String: String] = [:]       // draft or outbox id -> the thread its reminder rides on (delta 18)
    var replyArrivedAt: [String: Int64] = [:]             // thread -> when the first reply reached it (`fixture reply-arrived`, delta 18)
    var draftRows: [String: (account: String, to: [String], cc: [String], subject: String, body: String, thread: String?)] = [:]
    var indexedAccounts: Int? = nil              // `fixture indexing <n>`: how many fixture accounts are indexed; nil = all (delta 12)
    var draftBcc: [String: [String]] = [:]       // draft id -> its Bcc (delta 12; kept beside draftRows so their tuple stays as it was)
    var sentFromDraft: [String: Int64] = [:]       // outbox id of a Send of a saved draft -> the draft's saved time, so Z brings the draft back (delta 12)
    var draftSavedAt: [String: Int64] = [:]   // draft id -> when it was last saved (delta 11; a draft with no entry reads as born with the core)
    var mcpRequests: [String: (app: String, tool: String, state: String)] = [:]
    var mcpQueries: [String: String] = [:]        // request id -> what an app asked `search_mail` for (delta 13)
    var mcpResults: [String: [String]] = [:]      // request id -> the subjects the approved search returned (delta 13)
    var contactCards: [Contact] = []       // one per sender address, merged by MergeContacts
    var snoozed: [String: Int64] = [:]     // message id -> return time
    var pinned: [String: Bool] = [:]
    var read: Set<String> = []
    var labelRows: [MailLabel] = []        // the label tree, flat, a parent before its children
    var assigned: [String: [String]] = [:] // message id -> label ids
    /// Replies the owner sent into each thread (`joinThreadLocked`), counted again only when the message count changes: a thread row's count includes them, as the engine's does.
    var sentCountsStamp = -1
    var sentCountsMemo: [String: Int] = [:]
    func sentCountsLocked() -> [String: Int] {
        let all = messages
        if all.count != sentCountsStamp {
            sentCountsMemo = all.reduce(into: [:]) { out, m in if m.outgoing == true, let t = m.thread { out[t, default: 0] += 1 } }
            sentCountsStamp = all.count
        }
        return sentCountsMemo
    }
    var account: [String: String] = [:]    // message id -> account id (after a move)
    var tabNames: [String: String] = [:]
    var added: [(id: String, address: String, provider: MailProvider)] = []   // accounts added by AddAccount
    var addedAt: [String: Int64] = [:]       // account id -> when the owner added it
    var accountOrder: [String]?              // the owner's order, once set by ReorderAccounts
    var accountNames: [String: String] = [:]       // account id -> the owner's name for it (RenameAccount, delta 11)
    var pausedAccounts: Set<String> = []            // PauseAccount (delta 11); the fixture's own `paused` flags are read beside it
    var removedAccounts: Set<String> = []           // RemoveAccount (delta 11)
    var aiSetAside: [String: Int] = [:]             // `fixture ai-set-aside <label> <n>`: the mails the label AI's judge set aside (label/<id>/flow/ai_set_aside); none unless armed
    var noAccounts = false                          // `fixture accounts none`: the core lists no account (the sidebar's empty state); `all` restores
    var accountSignatures: [String: String] = [:]  // account id -> signature (SetSignature, delta 11)
    var lastRemoval: AccountRemoval?
    var removedByUser: Set<String> = []             // "<thread id>|<label id>": labels the owner took off a thread (delta 11)                // what the last RemoveAccount took (state path account_removal/last)
    var presetName = "Superhuman"
    var noteRows: [Note] = []                // private notes, oldest first; ids note-1..
    var splitTabRows: [SplitTab] = []        // the strip: ids in order; the name lives in tabNames
    let bornMs: Int64
    var undo: [UndoEntry] = []
    var consumedLast = false
    var entries: [OutboxEntry] = []
    /// Test doors of the outbox (`fixture offline`, `fixture server-refuse`): what a send that is due does.
    struct OutboxHold { var offline = false; var refusal: String? }
    /// `fixture sync broken|ok` (a test copy only): while on, `settings_sync/error` answers the reason below, as a core whose last sync failed does. `fixture reset` puts it off.
    var syncBroken = false
    static let syncBrokenReason = "The Mac did not answer."

    var outboxHold = OutboxHold()
    /// The AI draft the composer last reported (`noteAIJob`); served as `ai_job/last/*`; `fixture reset` clears it.
    var aiJobNote: FixtureAIJobNote?
    var sinks: [UInt64: (StreamKind, EventSink)] = [:]
    var nextSub: UInt64 = 1
    var nextId = 1
    /// the Move-accounts link and the added accounts; the token sits in a rig-prefixed Keychain item
    lazy var deviceLink = DeviceLinkModel(expirySeconds: WhooshConstants.QR_EXPIRY_S, clock: { [clock] in clock.nowMs() },
                                                  keychain: DeviceLinkKeychain(service: "rig.whoosh.device-link", account: "fixture-core-\(ObjectIdentifier(self).hashValue)"))
    var pending: [(StreamKind, CoreEvent)] = []   // stream events a command produced, sent after it commits
    /// Ask my inbox, its providers and agents (delta 7): the rules of core/crates/agents over a fake provider.
    // delta 9: labels as the tabs, label rules, suggestions and blocks (logic in FixtureLabels.swift)
    var labelRuleSets: [String: [RuleSet]] = [:]       // label id -> its fixed rule sets: the sheet's first, then always-label additions (delta 17: the ENTRY chains)
    // delta 17: a label is a flow of three cards (logic in FixtureFlow.swift)
    var entryChainIds: [String: [UInt32]] = [:]        // label id -> the chain numbers of `labelRuleSets`, same order (numbers are never reused)
    var exitChains: [String: [LabelChain]] = [:]       // label id -> its exit chains
    var flowCard: [String: [String: (card: WhyCard, chain: UInt32?)]] = [:]   // message id -> label id -> which card placed it, by which chain
    var flowRejected: [String: (ai: String, to: String, chain: UInt32?)] = [:] // message id -> the AI's pick the exit filter sent elsewhere
    var repliedByMe: Set<String> = []                  // message ids the owner answered (delta 17 fact RepliedByMe; `fixture reply` flips it)
    var unansweredState: [String: Bool] = [:]          // message id -> the owner's Unanswered fact once set; absent = filed in Needs Reply
    var attachmentTransfers: [String: (startMs: Int64, messageId: String)] = [:]   // delta 23: DownloadAttachment in flight (FixtureDraftFiles.swift)
    var landedAttachments: Set<String> = []                                     // delta 23: received files the fixture says are on this Mac now
    var decisionConsentGiven: Set<String> = []   // delta 23: providers the owner allowed ("jev"); none = not given
    var draftFileRows: [String: [OutgoingAttachment]] = [:]   // delta 22: owner id (a draft, after Send its outbox entry) -> its files (FixtureDraftFiles.swift)
    var newInstall = false                              // `fixture install-state new` (FixtureFirstRun.swift): the Replies settings and labels of a new installation
    var replyBookStore: FixtureReplyBook?               // delta 20: AI replies (FixtureReplies.swift); built on first use, dropped by a reset
    var flowEdited: Set<String> = []                   // labels whose flow the owner changed: their figures are derived, not the file's example
    var filedBy: [String: [String: DecidedBy]] = [:]   // message id -> label id -> who filed it
    var category: [String: String] = [:]                // message id -> built-in label id after a re-filing (else the fixture's tab)
    /// vip-mark: the test engine's VIP list as `MarkVip` changed it, sender address (lower case) -> marked (true) or unmarked (false). A marked
    /// sender's mail is re-filed under "vips" through `category` (as smart-inbox EditVips moves it to the VIPs tab), an unmarked one's back to Primary.
    var vipSenders: [String: Bool] = [:]
    var designedForRig: Set<String> = []                // the notch's full-mail check: mails served as designed (FixtureCore.designForRig)
    var rerunState: [String: (filed: Int, refiled: Int, unfiled: Int, modelPending: Bool)] = [:]
    var suggestionRows: [Suggestion] = []
    var snoozesOfThread: [String: Int] = [:]     // thread id -> how many times the owner snoozed it (delta 12 trigger)
    var labelsOfSender: [String: Int] = [:]      // "sender|label id" -> how many times the owner put that label on that sender (delta 12)
    var blockRows: [Block] = []
    var unsubscribed: Set<String> = []
    var inserted: Set<String> = []                      // messages the insert doors added: not part of the fixture's tab figures
    var archiveRefused: [String: String] = [:]          // message id -> why the server refuses its archive (group 8)
    var pinnedFromCorpus: [String]? = nil
    var defaultPins: Set<String> = []                   // the pins the file (or corpus) started with: what RestoreDefaultSplitTabs returns to

    var agents = FixtureAgentBook()
    /// `fixture provider-endpoint <provider> <url>`: where a pasted key is checked (POST <url>/validate with the bearer key). No endpoint = the length rule only.
    var providerEndpoints: [String: String] = [:]
    /// Notch and notification settings, account icon and colour (delta 7).
    var settings = FixtureSettingsBook()
    var oauth = FixtureOAuthBook()   // delta 14: the fake authority's sign-ins
    var importBook = FixtureImportBook()   // delta 18: the scripted mail import
    /// Add account by IMAP (delta 8): recorded detections and validations per fixture domain.
    var addAccount = FixtureAddAccountBook()
    /// Recent searches, the fixture ranker and what was read out of attachments (delta 8).
    var searchBook = FixtureSearchBook()
    /// the attachment index of the loaded corpus (Sources/Features/AttachmentIndex); nil until a corpus with attachment_content is loaded
    var attachmentBook: AttachmentIndexBook?
    /// What a rule in words matched, kept until the mail set changes (FixtureFiling.swift `ruleMatchIdsLocked`)
    var ruleMatchMemo: [String: (stamp: String, ids: [String])] = [:]

    /// the fixture file this core loaded (the attachments folder sits beside it)
    let sourceURL: URL?

    public init(url: URL, clock: FixtureClock) throws {
        sourceURL = url
        fx = try FixtureFile.load(from: url)
        messages = fx.messages
        messageById = Self.index(messages)
        self.clock = clock
        bornMs = clock.nowMs()
        order = []
        resetMessages()
        for t in fx.tabs { tabNames[t.id] = t.name }
        buildLabels(fx.labels ?? [], parent: nil)
        installBuiltInLabels()
        seedPlacementsLocked()
        seedFlowMessagesLocked()   // delta 17: the rows that exercise every card of À répondre
        // delta 15: each fixture account's own signature, read by AccountSummary.signature and signature(address:)
        for a in fx.accounts { if let s = a.signature, !s.isEmpty { accountSignatures[a.id] = s } }
        folderRows = Self.folders(fx.smart_folders ?? [])
        clock.onAdvance = { [weak self] in self?.clockMoved() }
    }

    func buildLabels(_ nodes: [FixtureFile.LabelNode], parent: MailLabel?) {
        for n in nodes {
            var row = Self.labelRow(id: "lbl-\(labelRows.count + 1)", name: n.name, parent: parent)
            row.unread = UInt32(max(0, n.unread ?? 0))
            row.colour = n.colour
            row.description = n.description ?? ""
            row.pinned = n.pinned ?? false
            if let a = n.account { row.scope = .account(accountId: a); row.accountIds = [a] }
            if let r = n.rules { labelRuleSets[row.id] = [r.ruleSet] }
            labelRows.append(row)
            buildLabels(n.children ?? [], parent: row)
        }
    }

    static func folders(_ nodes: [FixtureFile.SmartFolderNode]) -> [SmartFolder] {
        nodes.enumerated().map { SmartFolder(id: "sf-\($0.offset + 1)", name: $0.element.name, ruleWords: $0.element.rule, parentId: nil) }
    }

    /// The former split tabs are the built-in category labels now (delta 9): the same ids and names, Social off, VIPs a mailbox.
    static func defaultSplitTabs(_ tabs: [FixtureFile.Tab]) -> [SplitTab] {
        builtinLabels().enumerated().map { SplitTab(id: $0.element.id, name: $0.element.name, order: UInt32($0.offset), rule: .builtin(name: $0.element.id), enabled: $0.element.pinned) }
    }

    public var focusedMessageId: String? { fx.focused_message }
    /// The tab list the UI reads (delta 16): Primary and All (views), then the pinned labels in tree order, then the owner's rule tabs.
    public var tabIds: [String] { lock.withLock { inboxTabsLocked().map(\.id) } }
    public func splitTabs() -> [SplitTab] {
        lock.withLock { splitTabRows.enumerated().map { (e) -> SplitTab in var t = e.element; t.order = UInt32(e.offset); t.name = tabNames[t.id] ?? t.name; return t } }
    }
    public func tabName(_ id: String) -> String { lock.withLock { tabNames[id] ?? id } }

    func fxMessage(_ id: String) -> FixtureFile.Message? { messageById[id] }
    func accountName(_ id: String) -> String { fx.accounts.first { $0.id == id }?.name ?? id }
    /// The thread a message belongs to (the fixture's thread id, else the message is its own thread).
    /// THE thread id of a test-engine mail, one rule for every place that needs it: the row's own `thread`, else the id of the design file's open
    /// thread for the design thread's first message (msg-01 is the thread `thr-01` of docs/design/fixtures/mac-main-sample.json), else the mail's own id.
    func threadIdOf(_ m: FixtureFile.Message) -> String {
        if let t = m.thread { return t }
        if let open = fx.open_thread, m.id == fx.messages.first?.id { return open.id }
        return m.id
    }

    func threadOf(_ id: String) -> String { fxMessage(id).map(threadIdOf) ?? id }

    func accountExists(_ id: String) -> Bool { fx.accounts.contains { $0.id == id } || added.contains { $0.id == id } }

    /// How a message's list can be left (delta 10): the contract's rule over the fixture row's List-Unsubscribe data.
    func unsubscribeMethodLocked(_ id: String) -> UnsubscribeMethod? {
        guard let m = fxMessage(id), let urls = m.list_unsubscribe else { return nil }
        return unsubscribeMethodFrom(urls: urls, oneClickOffered: m.list_unsubscribe_one_click ?? false)
    }

    public func unsubscribeMethod(messageId: String) -> UnsubscribeMethod? { lock.withLock { unsubscribeMethodLocked(messageId) } }

    /// The drafts replying in a thread, newest first (delta 11). A thread is named by its id; a draft saved against one of
    /// its messages (the fixture's reply-all and forward drafts carry the message id) counts too.
    public func drafts(threadId: String) -> [DraftSummary] {
        lock.withLock {
            let members = Set(order.filter { threadOf($0) == threadId }).union([threadId])
            return draftRows.compactMap { id, d -> DraftSummary? in
                guard let t = d.thread, members.contains(t) else { return nil }
                return DraftSummary(id: id, threadId: threadId, accountId: d.account, savedAt: draftSavedAt[id] ?? bornMs,
                                    firstLines: draftFirstLines(body: d.body, n: UInt32(WhooshConstants.DRAFT_SUMMARY_LINES)),
                                    to: d.to, cc: d.cc, bcc: draftBcc[id] ?? [], subject: d.subject)   // the header (delta 12)
            }.sorted { ($0.savedAt, $0.id) > ($1.savedAt, $1.id) }
        }
    }

    /// The folder the fixture's attachment files live in (docs/design/fixtures/attachments, the attachments-mac lane's):
    /// $WHOOSH_ATTACHMENTS_DIR, else beside the fixture the core loaded. A file found there is served with its real size.
    static func attachmentsFolder(fixture: URL?) -> URL? {
        if let d = ProcessInfo.processInfo.environment["WHOOSH_ATTACHMENTS_DIR"], !d.isEmpty { return URL(fileURLWithPath: d) }
        return fixture?.deletingLastPathComponent().appendingPathComponent("attachments")
    }

    static func mimeType(_ name: String) -> String {
        switch (name as NSString).pathExtension.lowercased() {
        case "pdf": return "application/pdf"
        case "png": return "image/png"
        case "jpg", "jpeg": return "image/jpeg"
        case "svg": return "image/svg+xml"
        case "txt": return "text/plain"
        case "csv": return "text/csv"
        case "docx": return "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        case "xlsx": return "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        case "zip": return "application/zip"
        case "mp4": return "video/mp4"
        case "m4a": return "audio/mp4"
        default: return "application/octet-stream"
        }
    }

    /// Attachments of a message (delta 10): one row per name the fixture lists ("quote.pdf (212 KB)" keeps its file name);
    /// `local_path` and the real size when the file is in the attachments folder, else not downloaded (no path, size 0).
    public func attachments(messageId: String) -> [AttachmentFile] { lock.withLock { attachmentFilesLocked(messageId) } }
    func attachmentFilesLocked(_ messageId: String) -> [AttachmentFile] {
        do {
            guard let m = fxMessage(messageId) else { return [] }
            let folder = Self.attachmentsFolder(fixture: sourceURL)
            return (m.attachments ?? []).enumerated().map { i, raw in
                let name = raw.components(separatedBy: " (").first ?? raw
                let file = folder?.appendingPathComponent(name)
                let size = file.flatMap { try? FileManager.default.attributesOfItem(atPath: $0.path)[.size] as? NSNumber }
                let id = "\(messageId)-att-\(i)"
                // delta 23: a file the fixture "downloaded" is on this Mac (a made-up path: nothing is written)
                let landed = landedAttachments.contains(id) ? "/tmp/whoosh-fixture-attachments/\(name)" : nil
                return AttachmentFile(id: id, messageId: messageId, fileName: name, mimeType: Self.mimeType(name),
                                      sizeBytes: size?.uint64Value ?? 0, localPath: size == nil ? landed : file?.path, inline: false, contentId: nil)
            }
        }
    }
    /// d15b: a sent copy (`outgoing`) lives in its thread and in Sent only: never an inbox, tab, mailbox, contact or smart-folder
    /// row, never a count. Every list builder filters on this; the thread read and the Sent mailbox (outbox entries) do not.
    func listable(_ id: String) -> Bool { fxMessage(id)?.outgoing != true }

    func gone(_ id: String) -> Bool { archived.contains(id) || deleted.contains(id) || snoozed[id] != nil || muted.contains(id) || spam.contains(id) }
    func newId(_ kind: String) -> String { defer { nextId += 1 }; return "\(kind)-\(nextId)" }

    /// The accounts an id belongs to: a message id gives its account; a thread id (the quick send passes one) gives the accounts of
    /// the thread's messages (`account` is keyed by message id, so reading it with a thread id found nothing).
    func accountsOf(_ id: String) -> [String] {
        if let a = account[id] { return [a] }
        return messages.filter { $0.thread == id }.map { account[$0.id] ?? $0.account }
    }

    func refuseFailing(_ ids: [String]) throws {
        if let failing = failingAccountId, ids.contains(where: { accountsOf($0).contains(failing) }) {
            throw CoreError.Action(failure: ActionFailed(accountName: accountName(failing), reasonWords: "needs you to sign in again", retryable: true))
        }
    }

    func pushUndo(_ action: UndoAction) -> String {
        let id = newId("undo")
        undo.append(UndoEntry(id: id, action: action, atMs: clock.nowMs(), consumed: false))
        if undo.count > WhooshConstants.UNDO_DEPTH { undo.removeFirst(undo.count - WhooshConstants.UNDO_DEPTH) }
        return id
    }

    // MARK: WhooshCore

    public func run(command: Command) throws -> CommandOutcome {
        try checkViewCommand(command: command)   // delta 16: Primary and All are views; a command that treats one as a label is refused in words, code view_not_a_label
        let outcome: CommandOutcome = try lock.withLock {
            settleOutboxLocked()
            settleKeyChecksLocked()
            settleAttachmentTransfersLocked()   // delta 23
            switch command {
            case .archive(let ids):
                if let bad = ids.first(where: { archiveRefused[$0] != nil }) {
                    throw CoreError.Action(failure: ActionFailed(accountName: accountName(account[bad] ?? ""), reasonWords: archiveRefused[bad] ?? "", retryable: false))
                }
                try refuseFailing(ids)
                ids.forEach { archived.insert($0) }
                let undo = pushUndo(.archive(messageIds: ids))
                return CommandOutcome(undoEntryId: undo, createdId: offerAfterArchiveLocked(ids))
            case .snooze(let ids, let when):
                try refuseFailing(ids)
                let at = returnTime(when)
                if at <= clock.nowMs() { throw CoreError.Invalid(field: "when", reasonWords: "that time has already passed") }
                ids.forEach { snoozed[$0] = at }
                let undo = pushUndo(.snooze(messageIds: ids))
                return CommandOutcome(undoEntryId: undo, createdId: offerAfterSnoozeLocked(ids))
            case .pin(let id, let on):
                let was = pinned[id] ?? false
                pinned[id] = on
                return CommandOutcome(undoEntryId: pushUndo(.pin(messageId: id, wasPinned: was)), createdId: nil)
            case .label(let ids, let name, let create):
                // the owner labelling again lifts their earlier removal (fileMessage, by owner; delta 11)
                return try labelMessages(ids, name: name, create: create)
            case .moveToAccount(let ids, let to):
                try refuseFailing(ids)
                let from = ids.first.flatMap { account[$0] } ?? ""
                ids.forEach { account[$0] = to }
                return CommandOutcome(undoEntryId: pushUndo(.move(messageIds: ids, fromAccountId: from)), createdId: nil)
            case .markRead(let ids, let on):
                ids.forEach { if on { read.insert($0) } else { read.remove($0) } }
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .openThread(let id):
                // the id is a message's (a base row is its own thread) or a thread's (`threadIdOf`: msg-01 is thr-01); the mail it names is read
                if let m = messages.first(where: { $0.id == id }) ?? messages.first(where: { threadIdOf($0) == id && $0.outgoing != true }) {
                    read.insert(m.id)
                    if answers(citing: m.id) { agents.lastQuoted = m.id }
                }
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .quickSend(let threadId, let body):
                let from = account[threadId] ?? fx.open_thread?.account ?? ""
                try refuseFailing([threadId])
                let draft = Draft(id: newId("draft"), fromAccountId: from, to: [], cc: [], subject: "", body: body, attachmentIds: [], aiMarks: [], replyToThreadId: threadId)
                return try queueSendLocked(draft)
            case .quickSendFrom(let threadId, let body, let from):
                guard accountExists(from) else { throw CoreError.NotFound(kind: "account", id: from) }
                try refuseFailing([threadId])
                if from == failingAccountId { throw CoreError.Action(failure: ActionFailed(accountName: accountName(from), reasonWords: "needs you to sign in again", retryable: true)) }
                let draft = Draft(id: newId("draft"), fromAccountId: from, to: [], cc: [], subject: "", body: body, attachmentIds: [], aiMarks: [], replyToThreadId: threadId)
                return try queueSendLocked(draft)
            case .removeLabel(let ids, let labelId):
                try refuseFailing(ids)
                let carrying = ids.filter { (assigned[$0] ?? []).contains(labelId) }
                for id in carrying { assigned[id]?.removeAll { $0 == labelId }; filedBy[id]?[labelId] = nil; removedByUser.insert("\(threadOf(id))|\(labelId)") }
                return CommandOutcome(undoEntryId: carrying.isEmpty ? nil : pushUndo(.unlabel(messageIds: carrying, labelId: labelId)), createdId: nil)
            case .removeAllLabels(let ids):
                try refuseFailing(ids)
                var removed: [LabelAssignment] = []
                for id in ids {
                    for l in assigned[id] ?? [] { removed.append(LabelAssignment(labelId: l, messageId: id)); removedByUser.insert("\(threadOf(id))|\(l)") }
                    assigned[id] = []; filedBy[id] = nil
                }
                return CommandOutcome(undoEntryId: removed.isEmpty ? nil : pushUndo(.unlabelAll(removed: removed)), createdId: nil)
            case .moveToLabel(let ids, let labelId):
                guard labelRows.contains(where: { $0.id == labelId }) else { throw CoreError.NotFound(kind: "label", id: labelId) }
                if let unknown = ids.first(where: { fxMessage($0) == nil }) { throw CoreError.NotFound(kind: "message", id: unknown) }
                try refuseFailing(ids)
                let labelled = ids.filter { !(assigned[$0] ?? []).contains(labelId) }
                let wasIn = ids.filter { !archived.contains($0) }
                labelled.forEach { assigned[$0, default: []].append(labelId); filedBy[$0, default: [:]][labelId] = .owner }
                wasIn.forEach { archived.insert($0) }
                let none = labelled.isEmpty && wasIn.isEmpty
                return CommandOutcome(undoEntryId: none ? nil : pushUndo(.moveToLabel(labelId: labelId, labelled: labelled, archived: wasIn)), createdId: nil)
            case .deleteMessages(let ids):
                if let unknown = ids.first(where: { fxMessage($0) == nil }) { throw CoreError.NotFound(kind: "message", id: unknown) }
                try refuseFailing(ids)
                let fresh = ids.filter { !deleted.contains($0) }
                // the origin of each (delta 11): the fixture's folder words and its archived mark, so Z restores both
                let origins = fresh.map { MessageOrigin(messageId: $0, folder: spam.contains($0) ? "Spam" : "INBOX", archived: archived.contains($0)) }
                fresh.forEach { deleted.insert($0) }
                return CommandOutcome(undoEntryId: fresh.isEmpty ? nil : pushUndo(.deleteMessages(messageIds: fresh, origins: origins)), createdId: nil)
            case .unarchive(let ids):
                try refuseFailing(ids)
                let was = ids.filter { archived.contains($0) }   // delta 19: Z archives again the messages the command changed
                ids.forEach { archived.remove($0) }
                return CommandOutcome(undoEntryId: was.isEmpty ? nil : pushUndo(.unarchive(messageIds: was)), createdId: nil)
            case .unsubscribe(let id): return try unsubscribeLocked(id)   // delta 18, FixtureMailActions.swift
            case .remindIfNoReply(let draftId, let at): return try remindIfNoReplyLocked(draftId, at)
            case .clearReplyReminder(let draftId): return try clearReplyReminderLocked(draftId)
            case .importScan(let picked): return try importScanLocked(picked)   // delta 18, FixtureImport.swift
            case .importStart(let sourceId, let into): return try importStartLocked(sourceId, into)
            case .importCancel(let runId): return try importCancelLocked(runId)
            case .cancelSend(let id):
                guard let i = entries.firstIndex(where: { $0.id == id }) else { throw CoreError.NotFound(kind: "outbox entry", id: id) }
                // the real engine's refusals, core/crates/outbox/src/usecases/cancel_send.rs:14-19 (words from lines 16-18)
                switch entries[i].state {
                case .cancelled: return CommandOutcome(undoEntryId: nil, createdId: nil)
                case .sent: throw CoreError.Invalid(field: "outbox_id", reasonWords: "this message was already sent")
                case .sending: throw CoreError.Invalid(field: "outbox_id", reasonWords: "this message is being sent now")
                case .undoable(let until) where clock.nowMs() >= until: throw CoreError.Invalid(field: "outbox_id", reasonWords: "the undo window is over")
                default: break
                }
                entries[i].state = .cancelled
                pending.append((.outbox, .outboxTick(entry: entries[i])))   // the stream hears the cancel (a thread's card goes)
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .renameSplitTab(let id, let name):
                guard tabNames[id] != nil else { throw CoreError.NotFound(kind: "split tab", id: id) }
                let trimmed = name.trimmingCharacters(in: .whitespaces)
                if trimmed.isEmpty { throw CoreError.Invalid(field: "name", reasonWords: "a tab needs a name") }
                tabNames[id] = trimmed
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .undo:
                guard let i = undo.lastIndex(where: { !$0.consumed }) else {
                    throw CoreError.Invalid(field: "undo", reasonWords: "Nothing to undo")
                }
                reverse(undo[i].action)
                undo[i].consumed = true
                consumedLast = true
                return CommandOutcome(undoEntryId: undo[i].id, createdId: nil)
            case .addAccount(let address, let provider):
                deviceLink.scan()   // stands for the paired device's first request (devicesync lane)
                let a = address.trimmingCharacters(in: .whitespaces).lowercased()
                let parts = a.split(separator: "@")
                guard parts.count == 2, !parts[0].isEmpty, parts[1].contains(".") else {
                    throw CoreError.Invalid(field: "address", reasonWords: "\(address) is not a mail address")
                }
                // Welcome's rows and the sign-in route send this address-only add. The design fixture's own 30 addresses are what a pasted list names (J4.6, the
                // sample list): the add adopts the account the fixture already lists (its id, its sync state) instead of refusing every row as "already added".
                if let mine = fx.accounts.first(where: { $0.address.lowercased() == a }), !added.contains(where: { $0.address == a }) {
                    noAccounts = false
                    return CommandOutcome(undoEntryId: nil, createdId: mine.id)
                }
                if added.contains(where: { $0.address == a }) {
                    throw CoreError.Invalid(field: "address", reasonWords: "\(a) is already added")
                }
                let id = String(format: "acc-%02d", fx.accounts.count + added.count + 1)
                added.append((id, a, provider))
                noAccounts = false   // an add ends `fixture accounts none`: the core lists the new account (first-start step 6 counts it)
                addedAt[id] = clock.nowMs()
                return CommandOutcome(undoEntryId: nil, createdId: id)
            case .saveSmartFolder(let name, let rule):
                let n = name.trimmingCharacters(in: .whitespacesAndNewlines)
                if n.isEmpty { throw CoreError.Invalid(field: "name", reasonWords: "a smart folder needs a name") }
                if rule.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { throw CoreError.Invalid(field: "rule", reasonWords: "a smart folder needs a rule") }
                if smartFoldersLocked().contains(where: { $0.name.caseInsensitiveCompare(n) == .orderedSame }) {
                    throw CoreError.Invalid(field: "name", reasonWords: "the name exists already")
                }
                let id = "sf-\(smartFoldersLocked().count + 1)"
                folderRows.append(SmartFolder(id: id, name: n, ruleWords: rule.trimmingCharacters(in: .whitespacesAndNewlines), parentId: nil))
                return CommandOutcome(undoEntryId: nil, createdId: id)
            case .createLabel(let name, let parentId):
                let made = try createLabel(name, parentId: parentId)
                return CommandOutcome(undoEntryId: pushUndo(.createLabel(labelId: made)), createdId: made)
            case .renameLabel(let id, let name):
                let was = labelRows.first { $0.id == id }?.name
                try renameLabel(id, name)
                return CommandOutcome(undoEntryId: was.map { pushUndo(.renameLabel(labelId: id, previousName: $0)) }, createdId: nil)
            case .moveLabel(let id, let parentId):
                let was = labelRows.first { $0.id == id }?.parentId
                let moved = was != parentId
                try moveLabel(id, to: parentId)
                return CommandOutcome(undoEntryId: moved ? pushUndo(.moveLabel(labelId: id, previousParentId: was)) : nil, createdId: nil)
            case .deleteMessage(let id):
                guard fxMessage(id) != nil else { throw CoreError.NotFound(kind: "message", id: id) }
                try refuseFailing([id])
                deleted.insert(id)
                return CommandOutcome(undoEntryId: pushUndo(.deleteMessage(messageId: id)), createdId: nil)
            case .mergeContacts(let keep, let merge):
                try mergeContacts(keep: keep, merge: merge)
                return CommandOutcome(undoEntryId: nil, createdId: keep)
            case .ask(let agentId, let question):
                let run = try agents.ask(agentId: agentId, question: question, mail: agentMail(), nowMs: clock.nowMs())
                run.events.forEach { pending.append((.agent, $0)) }
                return CommandOutcome(undoEntryId: nil, createdId: run.conversationId)
            case .approveProposal(let id, let approve):
                return try agents.decide(id, approve: approve, nowMs: clock.nowMs()) { ids, label in try labelMessages(ids, name: label, create: true) }
            case .giveAgentConsent(let providerId):
                try agents.giveConsent(providerId: providerId, nowMs: clock.nowMs())
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .setNotchSetting(let setting):
                try settings.setNotch(setting)
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .setNotchAlerts(let alerts):
                try settings.setAlerts(alerts)
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .setNotificationSetting(let setting):
                try settings.setNotification(setting, known: accountIdsLocked())
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .setNotification(let accountId, let enabled):
                let cur = settings.notification(accountId)
                try settings.setNotification(NotificationSetting(accountId: accountId, enabled: enabled, delivery: cur.delivery), known: accountIdsLocked())
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .setAccountIcon(let accountId, let source):
                try settings.setIcon(accountId, source, known: accountIdsLocked())
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .setAccountColour(let accountId, let colour):
                try settings.setColour(accountId, colour, known: accountIdsLocked())
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .removeProviderKey(let providerId):   // delta 22: forgets the key; a provider that has none is not an error
                guard FixtureSettingsBook.credentialIds.contains(providerId) else { throw CoreError.NotFound(kind: "provider", id: providerId) }
                forgetKeyLocked(providerId)
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .saveProviderKey(let providerId, let key):   // answers at once; the check lands on ai_provider/<id>/state (FixtureProviderKeys.swift)
                guard FixtureSettingsBook.credentialIds.contains(providerId) else { throw CoreError.NotFound(kind: "provider", id: providerId) }
                try beginKeyCheckLocked(providerId, key: key.value)
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .saveAgentConfig(let config):
                let name = config.name.trimmingCharacters(in: .whitespacesAndNewlines)
                if name.isEmpty { throw CoreError.Invalid(field: "name", reasonWords: "an agent needs a name") }
                guard agents.provider(config.providerId) != nil else { throw CoreError.Invalid(field: "provider", reasonWords: "pick a provider for this agent first") }
                var c = config; c.name = name
                if c.id.isEmpty {
                    if agents.agents.contains(where: { $0.name.caseInsensitiveCompare(name) == .orderedSame }) { throw CoreError.Invalid(field: "name", reasonWords: "an agent called “\(name)” exists already") }
                    c.id = "agent-\(agents.agents.count + 1)"; c.builtIn = false
                    agents.agents.append(c)
                } else if let i = agents.agents.firstIndex(where: { $0.id == c.id }) {
                    c.builtIn = agents.agents[i].builtIn
                    agents.agents[i] = c
                } else { throw CoreError.NotFound(kind: "agent", id: c.id) }
                return CommandOutcome(undoEntryId: nil, createdId: c.id)
            case .testProviderKey(let providerId):   // hid-ai round 4: Jev's Test, the real core's contract (FixtureDecide.swift beginJevTestLocked)
                try beginJevTestLocked(providerId)
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .refitDecisionModel:                // hid-ai round 4: refused in words on both engines until a refit exists (FixtureDecide.swift refitLocked)
                try refitLocked()
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .giveDecisionConsent(let provider, let given):   // delta 23
                try checkGiveDecisionConsent(provider: provider)
                if given { decisionConsentGiven.insert("jev") } else { decisionConsentGiven.remove("jev") }
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .saveDecisionModel(let model):
                try checkDecisionModelConsent(provider: model.provider, consentGiven: decisionConsentGiven.contains("jev"))
                try settings.saveDecisionModel(model, hasKey: { agents.credentialStored.contains($0) })
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .chooseSpeechEngine(let kind):
                try settings.chooseSpeech(kind, hasKey: { agents.credentialStored.contains($0) })
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .previewDecisions:
                let names = fx.tabs.map(\.name)
                let recent = order.compactMap { id -> (id: String, tab: String, other: String)? in
                    guard let m = fxMessage(id), !gone(id) else { return nil }
                    let i = fx.tabs.firstIndex { $0.id == (m.tab ?? "primary") } ?? 0
                    return (id, names[i], names[(i + 1) % names.count])
                }
                settings.runPreview(recent: recent, nowMs: clock.nowMs())
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .previewDecisionList(let page, let entries): return try previewDecisionListLocked(page: page, entries: entries)   // gap-preview-core, FixtureDecisionPreview.swift
            case .saveDecisionList(let page, let nodeIds, let rules): return try saveDecisionListLocked(page: page, nodeIds: nodeIds, rules: rules)   // gap-decisions, FixtureDecisionList.swift
            case .applyDecisionPreview:
                try settings.applyPreview()
                // delta 13: the placement the preview applied is written where every reader reads it
                for (id, who) in settings.placedBy {
                    guard let m = fxMessage(id), let d = Self.decidedBy(who) else { continue }
                    filedBy[id, default: [:]][category[id] ?? m.tab ?? "primary"] = localDecider(d)
                }
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .setMcpGrant(let app, let allowed):
                try settings.setMcp(app, allowed: allowed, nowMs: clock.nowMs())
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .setMcpServer(let on):
                settings.setMcpServer(on)
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .addSplitTab(let name, let rule):
                return CommandOutcome(undoEntryId: nil, createdId: try addSplitTab(name, rule))
            case .reorderSplitTabs(let ids):
                // the door (checkViewCommand, above) already refused a view moved; the views first are the real core's own rows (its strip lists Primary), so they are let by here
                let lead = ids.prefix(while: { inboxViewFromId(id: $0) != nil })
                let tabsOnly = Array(ids.dropFirst(lead.count))
                guard Set(lead).count == lead.count, tabsOnly.count == splitTabRows.count, Set(tabsOnly) == Set(splitTabRows.map(\.id)) else { throw CoreError.Invalid(field: "ids", reasonWords: "the order must list every tab once") }
                splitTabRows = tabsOnly.compactMap { id in splitTabRows.first { $0.id == id } }
                syncLabelsFromTabs()
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .setSplitTabEnabled(let id, let on):
                guard let i = splitTabRows.firstIndex(where: { $0.id == id }) else { throw CoreError.NotFound(kind: "split tab", id: id) }
                if !on && splitTabRows.filter(\.enabled).count == 1 && splitTabRows[i].enabled { throw CoreError.Invalid(field: "enabled", reasonWords: "at least one tab has to stay") }
                splitTabRows[i].enabled = on
                syncLabelsFromTabs()
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .restoreDefaultSplitTabs:
                splitTabRows = Self.defaultSplitTabs(fx.tabs)
                tabNames = [:]
                for t in fx.tabs { tabNames[t.id] = t.name }
                syncLabelsFromTabs(restoring: true)
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .addNote(let target, let text):
                let t = text.trimmingCharacters(in: .whitespacesAndNewlines)
                if t.isEmpty { throw CoreError.Invalid(field: "text", reasonWords: "a note needs words") }
                switch target {
                case .thread(let tid): if !threadExistsLocked(tid) { throw CoreError.NotFound(kind: "thread", id: tid) }
                case .message(let mid): if messageIdKnownLocked(mid) == false { throw CoreError.NotFound(kind: "message", id: mid) }
                }
                var maxN = 0
                for n in noteRows { if let k = Int(n.id.dropFirst(5)) { maxN = max(maxN, k) } }
                let note = Note(id: "note-\(maxN + 1)", target: target, text: t, writtenAtMs: clock.nowMs())
                noteRows.append(note)
                return CommandOutcome(undoEntryId: pushUndo(.noteAdd(noteId: note.id)), createdId: note.id)
            case .editNote(let id, let text):
                guard let i = noteRows.firstIndex(where: { $0.id == id }) else { throw CoreError.NotFound(kind: "note", id: id) }
                let t = text.trimmingCharacters(in: .whitespacesAndNewlines)
                if t.isEmpty { throw CoreError.Invalid(field: "text", reasonWords: "a note needs words") }
                let previous = noteRows[i].text
                noteRows[i].text = t
                return CommandOutcome(undoEntryId: pushUndo(.noteEdit(noteId: id, previousText: previous)), createdId: nil)
            case .deleteNote(let id):
                guard let gone = noteRows.first(where: { $0.id == id }) else { throw CoreError.NotFound(kind: "note", id: id) }
                noteRows.removeAll { $0.id == id }
                return CommandOutcome(undoEntryId: pushUndo(.noteDelete(note: gone)), createdId: nil)
            case .setShortcutPreset(let preset):
                guard ["Superhuman", "GotEmail"].contains(preset) else { throw CoreError.Invalid(field: "preset", reasonWords: "the presets are Superhuman and GotEmail") }
                presetName = preset
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .reorderAccounts(let ids):
                let all = accountIdsLocked()
                guard ids.count == all.count, Set(ids) == Set(all) else { throw CoreError.Invalid(field: "ids", reasonWords: "the order must list every account once") }
                accountOrder = ids
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .addAccountConfigured(let address, let displayName, let config, let password):
                if let code = addAccount.armedEngineCode { addAccount.armedEngineCode = nil; throw FixtureAddAccountBook.engineFailure(code) }
                let a = address.trimmingCharacters(in: .whitespaces).lowercased()
                guard let domain = FixtureAddAccountBook.domain(of: a) else { throw CoreError.Invalid(field: "address", reasonWords: "\(address) is not a mail address") }
                if fx.accounts.contains(where: { $0.address.lowercased() == a }) || added.contains(where: { $0.address == a }) {
                    throw CoreError.Invalid(field: "address", reasonWords: "\(a) is already added")
                }
                // the password is used for this check only; it is never stored, echoed or put in a state path
                let report = addAccount.validate(config, password: password.value, address: a)
                guard report.ok else {
                    throw CoreError.Action(failure: ActionFailed(accountName: (displayName?.isEmpty == false ? displayName : nil) ?? a, reasonWords: report.summaryWords, retryable: false))
                }
                deviceLink.scan()
                let id = String(format: "acc-%02d", fx.accounts.count + added.count + 1)
                added.append((id, a, MailProvider(words: domain)))
                noAccounts = false   // same: the configured add path
                addedAt[id] = clock.nowMs()
                addAccount.save(id, config, address: a)
                return CommandOutcome(undoEntryId: nil, createdId: id)
            case .updateSmartFolder(let folderId, let rule, let parentId):
                guard let i = folderRows.firstIndex(where: { $0.id == folderId }) else { throw CoreError.NotFound(kind: "smart folder", id: folderId) }
                let r = rule.trimmingCharacters(in: .whitespacesAndNewlines)
                if r.isEmpty { throw CoreError.Invalid(field: "rule", reasonWords: "a smart folder needs a rule") }
                var parent = parentId
                if let p = parentId {
                    if p == folderId { throw CoreError.Invalid(field: "parent", reasonWords: "a folder cannot sit inside itself") }
                    // JOURNEYS J2.6 (lead, 2026-11-12: the signed journey wins): a smart folder also nests under a LABEL, named by its id or its name;
                    // the label's id is kept. An unknown parent is refused as before.
                    if !folderRows.contains(where: { $0.id == p }) {
                        guard let l = labelRows.first(where: { $0.id == p || $0.name.caseInsensitiveCompare(p) == .orderedSame }) else {
                            throw CoreError.NotFound(kind: "smart folder", id: p)
                        }
                        parent = l.id
                    }
                }
                folderRows[i].ruleWords = r
                folderRows[i].parentId = parent
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .star(let ids, let on):
                try refuseFailing(ids)
                ids.forEach { if on { starred.insert($0) } else { starred.remove($0) } }
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .mute(let ids, let on):
                try refuseFailing(ids)
                let wasMuted = ids.filter { muted.contains($0) }   // delta 19: Z puts each message's earlier flag back
                let changes = ids.contains { muted.contains($0) != on }
                ids.forEach { if on { muted.insert($0) } else { muted.remove($0) } }
                return CommandOutcome(undoEntryId: changes ? pushUndo(.mute(messageIds: ids, wasMuted: wasMuted)) : nil, createdId: nil)
            case .markVip(let id, let on):   // vip-mark: the message's sender joins or leaves the VIP list; an unknown id is refused, as the real core does
                guard let m = fxMessage(id) else { throw CoreError.NotFound(kind: "message", id: id) }
                try refuseFailing([id])
                let address = m.from.trimmingCharacters(in: .whitespaces).lowercased()
                vipSenders[address] = on
                for other in messages where other.from.trimmingCharacters(in: .whitespaces).lowercased() == address && !gone(other.id) {
                    if on { category[other.id] = "vips" } else if (category[other.id] ?? other.tab) == "vips" { category[other.id] = "primary" }
                }
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .markSpam(let ids):
                try refuseFailing(ids)
                ids.forEach { spam.insert($0) }
                return CommandOutcome(undoEntryId: nil, createdId: offerAfterJunkLocked(ids))
            case .markNotSpam(let ids):   // delta 19: the messages leave Spam; state message/<id>/spam reads false
                try refuseFailing(ids)
                ids.forEach { spam.remove($0) }
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .saveDraft(let from, let to, let cc, let subject, let body, let thread):
                if to.allSatisfy({ $0.trimmingCharacters(in: .whitespaces).isEmpty }) && subject.trimmingCharacters(in: .whitespaces).isEmpty && body.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    throw CoreError.Invalid(field: "draft", reasonWords: "there is nothing to save: add a recipient, a subject or some words")
                }
                let known = accountIdsLocked()
                let acc = from ?? thread.flatMap { account[$0] } ?? fx.open_thread?.account ?? known.first ?? ""
                guard known.contains(acc) else { throw CoreError.NotFound(kind: "account", id: acc) }
                let id = newId("draft")
                draftRows[id] = (acc, to, cc, subject, body, thread)
                draftSavedAt[id] = clock.nowMs()
                return CommandOutcome(undoEntryId: nil, createdId: id)
            case .renameAccount(let id, let name):
                try renameAccountLocked(id, name)
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .pauseAccount(let id, let paused):
                try pauseAccountLocked(id, paused)
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .removeAccount(let id):
                _ = try removeAccountLocked(id)
                return CommandOutcome(undoEntryId: nil, createdId: id)
            case .setSignature(let id, let text):
                try setSignatureLocked(id, text)
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .updateDraft(let draftId, let from, let to, let cc, let bcc, let subject, let body):
                guard let d = draftRows[draftId] else { throw CoreError.NotFound(kind: "draft", id: draftId) }
                if to.allSatisfy({ $0.trimmingCharacters(in: .whitespaces).isEmpty }) && subject.trimmingCharacters(in: .whitespaces).isEmpty && body.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    throw CoreError.Invalid(field: "draft", reasonWords: "there is nothing to save: add a recipient, a subject or some words")
                }
                let acc = from ?? d.account
                guard accountIdsLocked().contains(acc) else { throw CoreError.NotFound(kind: "account", id: acc) }
                draftRows[draftId] = (acc, to, cc, subject, body, d.thread)
                draftBcc[draftId] = bcc.isEmpty ? nil : bcc
                draftSavedAt[draftId] = clock.nowMs()
                return CommandOutcome(undoEntryId: nil, createdId: draftId)
            case .deleteDraft(let draftId):
                guard let d = draftRows[draftId] else { throw CoreError.NotFound(kind: "draft", id: draftId) }
                let saved = draftSavedAt[draftId] ?? bornMs
                draftRows[draftId] = nil
                draftSavedAt[draftId] = nil
                let draft = Draft(id: draftId, fromAccountId: d.account, to: d.to, cc: d.cc, subject: d.subject, body: d.body, attachmentIds: [], aiMarks: [],
                                  replyToThreadId: d.thread, bcc: draftBcc[draftId] ?? [])
                draftBcc[draftId] = nil
                return CommandOutcome(undoEntryId: pushUndo(.deleteDraft(draft: draft, savedAt: saved)), createdId: nil)
            case .sendNow(let id):
                guard let i = entries.firstIndex(where: { $0.id == id }) else { throw CoreError.NotFound(kind: "outbox entry", id: id) }
                switch entries[i].state {
                case .sent, .cancelled: throw CoreError.Invalid(field: "outbox", reasonWords: "this message is already sent or cancelled")
                default: break
                }
                let now = clock.nowMs()
                entries[i].state = .sent(atMs: now)
                pending.append((.outbox, .outboxTick(entry: entries[i])))
                pending.append((.outbox, .sendCompleted(outboxId: id, sentAtMs: now)))
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .removeSplitTab(let id):
                guard let i = splitTabRows.firstIndex(where: { $0.id == id }) else { throw CoreError.NotFound(kind: "split tab", id: id) }
                if case .builtin = splitTabRows[i].rule { throw CoreError.Invalid(field: "id", reasonWords: "the default tabs cannot be removed: hide a tab instead") }
                splitTabRows.remove(at: i)
                tabNames[id] = nil
                for k in splitTabRows.indices { splitTabRows[k].order = UInt32(k) }
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .answerMcpApproval(let requestId, let allowed):
                guard let r = mcpRequests[requestId], r.state == "pending" else { throw CoreError.NotFound(kind: "approval request", id: requestId) }
                // delta 13: an approved search runs now, after the owner's yes; the grant is read again, so a revoke made
                // while the prompt was open still wins (the real gate, core/crates/mcp usecases/gate.rs, does the same)
                let stillGranted = settings.mcpAllowedNow(r.app)
                mcpRequests[requestId]?.state = allowed && stillGranted ? "allowed" : "refused"
                if allowed, stillGranted, let q = mcpQueries[requestId] { mcpResults[requestId] = mcpSearchSubjectsLocked(q) }
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .recordSearch(let query):
                try searchBook.record(query)
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .search(let query):
                // a pill filters exactly and the words rank what is kept (SPEC A143): the engine is asked `from:"x" before:2026-06-01 revised price`
                let parsed = SearchQuery.typed(query)
                let nameOf = Dictionary(messages.map { ($0.from.lowercased(), $0.sender) }, uniquingKeysWith: { a, _ in a })
                let rows: [FixtureSearchBook.Row] = order.compactMap { id in
                    guard !deleted.contains(id), let m = fxMessage(id) else { return nil }
                    return FixtureSearchBook.Row(id: id, subject: m.subject, body: (m.body ?? [m.preview]).joined(separator: " "), sender: m.sender, attachments: m.attachments ?? [], receivedMs: receivedAt[id] ?? 0,
                                                 from: m.from, to: (m.to ?? []).map { (nameOf[$0.lowercased()] ?? "") + " " + $0 },
                                                 labels: (assigned[id] ?? []).compactMap { lid in labelRows.first { $0.id == lid }?.name })
                }
                // an exact-words query (quoted) is the text index's alone; anything else also asks the meaning index, with the words alone (never the pills)
                // delta 15 (embedding r8): the meaning ranker gets ids and a lookup, never a body string per message; the lookup
                // reads an immutable snapshot, not the core under its lock (the ranker's builder may call it while `run` holds it)
                let snapshot = messageById
                let live = order.filter { !deleted.contains($0) }
                let wordText = parsed.words.joined(separator: " ")
                let meaning = parsed.phrase != nil || wordText.isEmpty ? [] : Self.meaningRanker(wordText, live) { id in
                    snapshot[id].map { (title: Optional($0.subject), body: ($0.body ?? [$0.preview]).joined(separator: " ")) }
                }
                if let words = searchBook.refusal { throw CoreError.Invalid(field: "query", reasonWords: words) }   // `fixture search-refuse`
                let hits = try searchBook.run(query, rows: rows, meaning: meaning, attachmentHits: parsed.phrase != nil || wordText.isEmpty ? [] : (attachmentBook?.hits(for: wordText) ?? []), everyMail: true)
                if searchBook.hold {   // `fixture search-hold on`: the exact matches and the other mails by date now, the ranking when released
                    let byDate = Dictionary(rows.map { ($0.id, $0.receivedMs) }, uniquingKeysWith: { a, _ in a })
                    // a related mail (SPEC A150) is no word match; the first list carries no related mark, as the real engine's first send
                    func exact(_ h: SearchHit) -> Bool { switch h.matchedIn { case .meaningOnly, .related: return false; default: return true } }
                    func unmarked(_ h: SearchHit) -> SearchHit { var x = h; if case .related = x.matchedIn { x.matchedIn = .meaningOnly }; return x }
                    let first = hits.filter(exact) + hits.filter { !exact($0) }.map(unmarked).sorted { (byDate[$0.messageId] ?? 0) > (byDate[$1.messageId] ?? 0) }
                    searchBook.held = (query, hits); searchBook.stageOverride = "words"
                    pending.append((.search, .searchResults(query: query, hits: first)))
                    return CommandOutcome(undoEntryId: nil, createdId: nil)
                }
                pending.append((.search, .searchResults(query: query, hits: hits)))
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .replyAll(let id): return try composeFromLocked(id, forward: false)
            case .forward(let id):
                if let words = agents.forwardRefusal { throw CoreError.Invalid(field: "forward", reasonWords: words) }   // `fixture forward-refuse on` (test copy only)
                return try composeFromLocked(id, forward: true)
            case .renameSmartFolder(let id, let name): return try renameSmartFolderLocked(id, name)
            case .deleteSmartFolder(let id): return try deleteSmartFolderLocked(id)
            case .createLabelFull(let spec): return try createLabelFullLocked(spec)
            case .updateLabel(let id, let spec): return try updateLabelLocked(id, spec)
            case .reorderLabel(let id, let parent, let index): return try reorderLabelLocked(id, parent, index)
            case .pinLabel(let id, let on): return try pinLabelLocked(id, on)
            case .deleteLabel(let id): return try deleteLabelLocked(id)
            case .rerunLabel(let id): return try rerunLabelLocked(id)
            case .setLabelCard(let id, let card, let enabled): return try setLabelCardLocked(id, card, enabled)   // delta 17, FixtureFlow.swift
            case .setLabelAiReads(let id, let reads): return try setLabelAiReadsLocked(id, reads)
            case .setLabelExitFallback(let id, let fallback): return try setLabelExitFallbackLocked(id, fallback)
            case .addLabelChain(let id, let placement, let chain): return try addLabelChainLocked(id, placement, chain)
            case .updateLabelChain(let id, let placement, let chainId, let chain): return try updateLabelChainLocked(id, placement, chainId, chain)
            case .removeLabelChain(let id, let placement, let chainId): return try removeLabelChainLocked(id, placement, chainId)
            case .acceptSuggestion(let id): return try acceptSuggestionLocked(id)
            case .dismissSuggestion(let id): return try dismissSuggestionLocked(id)
            case .blockSender(let address): return try blockSenderLocked(address)
            case .blockDomain(let domain): return try blockDomainLocked(domain)
            case .send(let draftId):
                guard let d = draftRows[draftId] else { throw CoreError.NotFound(kind: "draft", id: draftId) }
                let bcc = draftBcc[draftId] ?? []
                let nobody = (d.to + d.cc + bcc).allSatisfy { $0.trimmingCharacters(in: .whitespaces).isEmpty }
                if nobody && d.thread == nil { throw CoreError.Invalid(field: "draft", reasonWords: "add a recipient before you send") }
                guard accountIdsLocked().contains(d.account) else { throw CoreError.NotFound(kind: "account", id: d.account) }
                let files = draftFileRows[draftId] ?? []
                try checkDraftAttachmentsSendable(list: refreshedFilesLocked(files))   // delta 22: a file gone or over a cap stops the send, nothing is queued
                let draft = Draft(id: draftId, fromAccountId: d.account, to: d.to, cc: d.cc, subject: d.subject, body: d.body, attachmentIds: files.map(\.id), aiMarks: [],
                                  replyToThreadId: d.thread, bcc: bcc)
                let saved = draftSavedAt[draftId] ?? bornMs
                let out = try queueSendLocked(draft)
                if let outboxId = out.createdId, !files.isEmpty {   // the files move to the queued message; the draft's go with it
                    draftFileRows[outboxId] = files.map { var f = $0; f.ownerId = outboxId; return f }
                    draftFileRows[draftId] = nil
                }
                // the draft leaves the list once it is queued; Z brings it back (undo below)
                draftRows[draftId] = nil; draftSavedAt[draftId] = nil; draftBcc[draftId] = nil
                if let id = out.createdId, out.undoEntryId != nil { sentFromDraft[id] = saved }
                return out
            case .schedule(let draftId, let atMs): return try scheduleLocked(draftId, atMs)
            case .setOAuthClient(let draft, let provider, let clientId):   // delta 14, FixtureOAuth.swift
                try oauth.setClient(draft, provider: provider, clientId: clientId)
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .startOAuthSignIn(let draft):
                if let code = addAccount.armedEngineCode { addAccount.armedEngineCode = nil; throw FixtureAddAccountBook.engineFailure(code) }
                try oauth.start(draft)
                return CommandOutcome(undoEntryId: nil, createdId: nil)
            case .setReplyUseEnabled, .setReplyUseTone, .setReplyUseMatchThread, .setReplyUsePrompt, .setReplyFilterScope, .setReplyFilterCard, .setReplyFilterEntryRule, .setReplyFilterExitRule, .setReplyFilterAiReads, .setReplyFilterAiDescription, .setOnDemandOption, .setRetrievalSetting, .setReplySourceEnabled, .addReplyLocalFolder, .removeReplyLocalFolder, .runVoiceAnalysis, .setVoiceText, .setVoiceSamePersonExamples, .saveReplyTone, .deleteReplyTone, .saveReplyPromptVersion, .restoreReplyPromptVersion, .saveFixedNote, .deleteFixedNote, .draftReplyOnDemand, .requestReplyChange, .acceptReplyDraft, .discardReplyDraft, .tryReply, .syncReplyCalendars:   // delta 20: AI replies (logic in FixtureReplies.swift)
                return try repliesCommandLocked(command)
            case .downloadAttachment(let attachmentId): return try downloadAttachmentLocked(attachmentId)   // delta 23, FixtureDraftFiles.swift
            case .addDraftAttachment(let draftId, let source): return try addDraftAttachmentLocked(draftId, source)   // delta 22, FixtureDraftFiles.swift
            case .removeDraftAttachment(let draftId, let attachmentId): return try removeDraftAttachmentLocked(draftId, attachmentId)
            }
        }
        emit()
        return outcome
    }

    /// Queues a send in the outbox with the undo window (QuickSend, QuickSendFrom and Send). When the fail-next "send" is
    /// armed, the entry fails at once with the armed words and server code (delta 12) and there is nothing to undo.
    func queueSendLocked(_ draft: Draft) throws -> CommandOutcome {
        let now = clock.nowMs()
        if let f = settings.takeFailure("send") {
            let failure = ActionFailed(accountName: accountName(draft.fromAccountId), reasonWords: f.reason, retryable: true, code: f.code)
            let entry = OutboxEntry(id: newId("outbox"), draft: draft, state: .failed(failure: failure, attempts: 1), createdAtMs: now)
            entries.append(entry)
            pending.append((.outbox, .outboxTick(entry: entry)))
            return CommandOutcome(undoEntryId: nil, createdId: entry.id)
        }
        let entry = OutboxEntry(id: newId("outbox"), draft: draft, state: .undoable(untilMs: now + Int64(WhooshConstants.UNDO_SEND_DEFAULT_S) * 1000), createdAtMs: now)
        entries.append(entry)
        pending.append((.outbox, .outboxTick(entry: entry)))
        return CommandOutcome(undoEntryId: pushUndo(.send(outboxId: entry.id)), createdId: entry.id)
    }

    /// Send later (core/crates/outbox/src/usecases/schedule_send.rs:9-24): a time not in the future is refused with the engine's words
    /// (line 15); the entry waits as `.scheduled(atMs:)` and the Outbox/Scheduled list shows it (waitingEntriesLocked); the draft leaves the
    /// drafts. Open point: the engine's `schedule` takes a draft id; the seam test passes the id of an entry it queued, so a draft id that
    /// is not in the drafts falls back to the draft of the outbox entry with that id (the entry itself stays).
    func scheduleLocked(_ draftId: String, _ atMs: Int64) throws -> CommandOutcome {
        let now = clock.nowMs()
        let draft: Draft
        if let d = draftRows[draftId] {
            let bcc = draftBcc[draftId] ?? []
            let nobody = (d.to + d.cc + bcc).allSatisfy { $0.trimmingCharacters(in: .whitespaces).isEmpty }
            if nobody && d.thread == nil { throw CoreError.Invalid(field: "draft", reasonWords: "add a recipient before you send") }
            guard accountIdsLocked().contains(d.account) else { throw CoreError.NotFound(kind: "account", id: d.account) }
            let files = draftFileRows[draftId] ?? []
            draft = Draft(id: draftId, fromAccountId: d.account, to: d.to, cc: d.cc, subject: d.subject, body: d.body, attachmentIds: files.map(\.id), aiMarks: [],
                          replyToThreadId: d.thread, bcc: bcc)
        } else if let e = entries.first(where: { $0.id == draftId }) {
            draft = e.draft
        } else {
            throw CoreError.NotFound(kind: "draft", id: draftId)
        }
        if atMs <= now { throw CoreError.Invalid(field: "at_ms", reasonWords: "pick a time in the future") }
        let entry = OutboxEntry(id: newId("outbox"), draft: draft, state: .scheduled(atMs: atMs), createdAtMs: now)
        entries.append(entry)
        pending.append((.outbox, .outboxTick(entry: entry)))
        if draftRows[draftId] != nil { draftRows[draftId] = nil; draftSavedAt[draftId] = nil; draftBcc[draftId] = nil; draftFileRows[draftId] = nil }
        return CommandOutcome(undoEntryId: nil, createdId: entry.id)
    }

    /// The Label command, also what an approved proposal runs.
    func labelMessages(_ ids: [String], name: String, create: Bool) throws -> CommandOutcome {
        try refuseFailing(ids)
        let labelId = try resolveLabel(name, create: create)
        for id in ids where !(assigned[id] ?? []).contains(labelId) { fileMessage(id, under: labelId, by: .owner) }
        offerAfterLabelLocked(ids, labelId: labelId)
        return CommandOutcome(undoEntryId: pushUndo(.label(messageIds: ids, labelId: labelId)), createdId: labelId)
    }

    /// What the fake provider is shown: the messages still in the inbox, with their searchable text.
    func accountIdsLocked() -> [String] { fx.accounts.map(\.id) + added.map(\.id) }
    /// The name the iPhone shows for the Mac it is paired with, and how long ago that Mac last synced (the fixture's "synced 2 min ago").
    static let pairedMacName = "Mac"
    static let pairedMacSyncAgeMs: Int64 = 120_000
    /// Whether an account's credential arrived with the scan: every account the fixture lists has one (`account_credential/<id>/stored`).
    func credentialCarriedLocked(_ id: String) -> Bool { accountIdsLocked().contains(id) }

    func agentMail() -> [FixtureMail] {
        order.compactMap { id in
            guard let m = fxMessage(id), !gone(id) else { return nil }
            let text = ([m.sender, m.from, m.subject, m.preview] + (m.attachments ?? []) + (m.body ?? [])).joined(separator: " ").lowercased()
            return FixtureMail(id: id, accountId: account[id] ?? m.account, sender: m.sender, subject: m.subject, preview: m.preview, receivedAtMs: receivedAt[id] ?? 0, text: text)
        }
    }

    func reverse(_ a: UndoAction) {
        switch a {
        case .archive(let ids): ids.forEach { archived.remove($0) }
        case .snooze(let ids): ids.forEach { snoozed[$0] = nil }
        case .pin(let id, let was): pinned[id] = was
        case .label(let ids, let labelId): ids.forEach { assigned[$0]?.removeAll { $0 == labelId } }
        case .move(let ids, let from): ids.forEach { account[$0] = from }
        case .send(let id):
            if let i = entries.firstIndex(where: { $0.id == id }) {
                entries[i].state = .cancelled
                pending.append((.outbox, .outboxTick(entry: entries[i])))   // the quickreply branch's hook: the card stream hears the undo
                // a saved draft sent with Send comes back to the draft list (delta 12)
                if let saved = sentFromDraft.removeValue(forKey: id) {
                    let d = entries[i].draft
                    draftRows[d.id] = (d.fromAccountId, d.to, d.cc, d.subject, d.body, d.replyToThreadId)
                    draftBcc[d.id] = d.bcc.isEmpty ? nil : d.bcc
                    draftSavedAt[d.id] = saved
                    if let back = draftFileRows.removeValue(forKey: id) { draftFileRows[d.id] = back.map { var f = $0; f.ownerId = d.id; return f } }   // delta 22: the files come back
                }
            }
        case .renameLabel(let id, let previous): try? renameLabel(id, previous)
        case .moveLabel(let id, let previousParent): try? moveLabel(id, to: previousParent)
        case .createLabel(let id):
            // the label is removed with its subtree, and with every assignment of it
            let gone = Set(subtree(of: id).map(\.id))
            labelRows.removeAll { gone.contains($0.id) }
            for k in Array(assigned.keys) { assigned[k]?.removeAll { gone.contains($0) } }
        case .deleteMessage(let id): deleted.remove(id)
        case .deleteMessages(let ids, _):
            // the fixture's delete leaves the archived mark and the spam mark as they were, so lifting it is the whole undo;
            // the origins are recorded for the real core, whose delete moves the folder (actions trash.rs)
            ids.forEach { deleted.remove($0) }
        case .deleteDraft(let d, let saved):
            draftRows[d.id] = (d.fromAccountId, d.to, d.cc, d.subject, d.body, d.replyToThreadId)
            draftBcc[d.id] = d.bcc.isEmpty ? nil : d.bcc
            draftSavedAt[d.id] = saved
        case .moveToLabel(let labelId, let labelled, let wasIn):
            labelled.forEach { id in assigned[id]?.removeAll { $0 == labelId }; filedBy[id]?[labelId] = nil }
            wasIn.forEach { archived.remove($0) }
        case .unlabel(let ids, let labelId): ids.forEach { if !(assigned[$0] ?? []).contains(labelId) { assigned[$0, default: []].append(labelId) } }
        case .unlabelAll(let removed): removed.forEach { if !(assigned[$0.messageId] ?? []).contains($0.labelId) { assigned[$0.messageId, default: []].append($0.labelId) } }
        case .mute(let ids, let wasMuted):   // delta 19: every message loses the flag, then the ones that had it take it back
            ids.forEach { muted.remove($0) }
            wasMuted.forEach { muted.insert($0) }
        case .unarchive(let ids): ids.forEach { archived.insert($0) }
        case .noteAdd(let id): noteRows.removeAll { $0.id == id }
        case .noteEdit(let id, let previous): if let i = noteRows.firstIndex(where: { $0.id == id }) { noteRows[i].text = previous }
        case .noteDelete(let note):
            if !noteRows.contains(where: { $0.id == note.id }) { noteRows.append(note); noteRows.sort { $0.writtenAtMs < $1.writtenAtMs } }
        }
    }

    func returnTime(_ when: SnoozeWhen) -> Int64 {
        let now = Date(timeIntervalSince1970: Double(clock.nowMs()) / 1000)
        var cal = Calendar(identifier: .gregorian); cal.timeZone = .current
        func at8(_ d: Date) -> Int64 { Int64((cal.date(bySettingHour: 8, minute: 0, second: 0, of: d) ?? d).timeIntervalSince1970 * 1000) }
        switch when {
        case .at(let ms): return ms
        case .laterToday: return clock.nowMs() + 3 * 3600 * 1000
        case .tomorrowMorning: return at8(cal.date(byAdding: .day, value: 1, to: now) ?? now)
        case .nextWeek:
            let monday = cal.nextDate(after: now, matching: DateComponents(weekday: 2), matchingPolicy: .nextTime) ?? now
            return at8(monday)
        }
    }

    public func inbox(query: InboxQuery) -> [InboxRow] {
        lock.withLock { rows(query) }
    }

    /// The list up to `rows` rows, as the engine serves it (contract `inbox_rows`): the newest first, cut at the count asked.
    public func inboxRows(query: InboxQuery, rows: UInt32) -> [InboxRow] {
        Array(inbox(query: query).prefix(Int(rows)))
    }

    /// Search's rows (any folder, no tab, account filter or page limit): one row per id that names a stored message, in the ids' order,
    /// built the way `inbox` builds its rows (the sent-mail seed included). An unknown id is skipped.
    public func rowsForMessages(messageIds: [String]) -> [InboxRow] {
        lock.withLock { messageIds.compactMap { inboxRow($0) } }
    }

    func rows(_ q: InboxQuery) -> [InboxRow] {
        let accounts = queryAccounts(query: q)          // the account SET (delta 10); nil = every account
        let filtered: (String) -> Bool = { id in passesFilters(facts: self.rowFactsLocked(id), filters: q.filters) }
        if let folder = q.smartFolderId {   // delta 14: the saved search's rows the inbox still shows, newest first; tab, mailbox and labels ignored
            return smartFolderIdsLocked(folder).filter { id in
                guard let m = fxMessage(id), !gone(id), listable(id) else { return false }
                if let set = accounts, !set.contains(account[id] ?? m.account) { return false }
                return filtered(id)
            }.compactMap { inboxRow($0) }
        }
        if let mailbox = q.mailbox { return mailboxRowsLocked(mailbox, accountIds: accounts).filter { filtered($0.messageId) } }
        let ids = order.filter { id in
            guard let m = fxMessage(id), !gone(id), listable(id) else { return false }
            if !belongs(id, toTab: q.tabId) { return false }
            if let set = accounts, !set.contains(account[id] ?? m.account), !(pinned[id] ?? false) { return false }
            if let l = q.labelId, !(assigned[id] ?? []).contains(l) { return false }
            return filtered(id)
        }
        let sorted = ids.filter { pinned[$0] ?? false } + ids.filter { !(pinned[$0] ?? false) }
        return sorted.compactMap { inboxRow($0) }
    }

    /// What the list filters read (delta 10). The fixture's decision signal: a message filed in Needs Reply or from a VIP is
    /// important; one filed in Needs Reply is awaiting the owner's reply (the fixture holds no reply history).
    func rowFactsLocked(_ id: String) -> ListFilterFacts {
        let tab = fxMessage(id)?.tab ?? "primary"
        return ListFilterFacts(unread: !read.contains(id), starred: starred.contains(id), important: tab == "needs-reply" || tab == "vips",
                        awaitingReply: tab == "needs-reply")
    }

    /// Delta 13: every placed row the fixture serves has its decided_by in the store (`filedBy`, read at
    /// message/<id>/label/<category>/decided_by and by the list row): the fixture row's decider is written there at load.
    /// A row the fixture leaves undecided (the default placement) writes nothing.
    func seedPlacementsLocked() {
        for m in messages {
            guard let d = Self.decidedBy(m.decided_by) else { continue }
            if case .default = d { continue }
            filedBy[m.id, default: [:]][m.tab ?? "primary"] = d
        }
    }

    /// SPEC A133: a local answer is filed under the on-device model in use: `.model(model:)` when the saved decision model names a line of
    /// the list other than Laya (`settings.decision?.model`; the list is the catalogue's lines, a held line a test copy lists, and the models
    /// the owner added), else as before. Any other decider is kept.
    func localDecider(_ d: DecidedBy) -> DecidedBy {
        guard case .laya = d, let id = settings.decision?.model, id != DecisionModelCatalogue.laya.id,
              DecisionModels.shared.lines.contains(where: { $0.id == id }) else { return d }
        return .model(model: id)
    }

    static func decidedBy(_ words: String?) -> DecidedBy? {
        switch words {
        case nil: return nil
        case "Laya": return .laya
        case "Jev": return .jev
        case "Owner": return .owner
        case let w? where w.hasPrefix("Rule: "): return .rule(ruleId: String(w.dropFirst(6)))
        default: return .default
        }
    }

    /// Whether a message is in a split tab: a built-in tab holds the messages the fixture filed there; a tab with a
    /// rule holds every message in the inbox that matches it (a message can sit in more than one tab).
    func belongs(_ id: String, toTab tabId: String) -> Bool {
        guard let m = fxMessage(id) else { return false }
        if let view = inboxViewFromId(id: tabId) {   // delta 16: a view reads the message's category and place, never a label
            return viewContains(view: view, facts: ViewFacts(
                categoryId: category[id] ?? m.tab ?? "primary",
                inInbox: listable(id) && !archived.contains(id) && snoozed[id] == nil && !muted.contains(id),
                junk: spam.contains(id), trashed: deleted.contains(id)))
        }
        guard let tab = splitTabRows.first(where: { $0.id == tabId }) else { return (m.tab ?? "primary") == tabId }
        switch tab.rule {
        case .builtin(let name): return (category[id] ?? m.tab ?? "primary") == name
        case .sender(let a): return m.from.lowercased() == a.lowercased()
        case .domain(let d): return m.from.lowercased().hasSuffix("@" + d.lowercased())
        case .account(let a): return (account[id] ?? m.account) == a
        case .label(let l):
            let under = Set(subtree(of: l).map(\.id) + [l])
            return (assigned[id] ?? []).contains { under.contains($0) }
        case .decisionCategory(let c): return (category[id] ?? m.tab ?? "primary") == c
        }
    }

    func slug(_ name: String) -> String {
        let s = name.lowercased().map { $0.isLetter || $0.isNumber ? String($0) : "-" }.joined()
        let parts = s.split(separator: "-", omittingEmptySubsequences: true)
        return parts.isEmpty ? "tab" : parts.joined(separator: "-")
    }

    func addSplitTab(_ name: String, _ rule: SplitTabRule) throws -> String {
        let n = name.trimmingCharacters(in: .whitespacesAndNewlines)
        if n.isEmpty { throw CoreError.Invalid(field: "name", reasonWords: "a tab needs a name") }
        if tabNames.values.contains(where: { $0.caseInsensitiveCompare(n) == .orderedSame }) { throw CoreError.Invalid(field: "name", reasonWords: "a tab called “\(n)” exists already") }
        switch rule {
        case .builtin: throw CoreError.Invalid(field: "rule", reasonWords: "pick a rule: sender, domain, account, label or decision category")
        case .sender(let a): if !a.contains("@") || a.hasPrefix("@") || a.hasSuffix("@") { throw CoreError.Invalid(field: "rule", reasonWords: "a sender is a full address, like orders@example.com") }
        case .domain(let d): if d.contains("@") || !d.contains(".") { throw CoreError.Invalid(field: "rule", reasonWords: "a domain looks like example.com") }
        case .account(let a): if !accountIdsLocked().contains(a) { throw CoreError.NotFound(kind: "account", id: a) }
        case .label(let l): if !labelRows.contains(where: { $0.id == l }) { throw CoreError.NotFound(kind: "label", id: l) }
        case .decisionCategory(let c): if c.trimmingCharacters(in: .whitespaces).isEmpty { throw CoreError.Invalid(field: "rule", reasonWords: "name the decision category") }
        }
        var id = slug(n), k = 2
        while splitTabRows.contains(where: { $0.id == id }) { id = slug(n) + "-\(k)"; k += 1 }
        tabNames[id] = n
        splitTabRows.append(SplitTab(id: id, name: n, order: UInt32(splitTabRows.count), rule: rule, enabled: true))
        return id
    }

    func inboxRow(_ id: String) -> InboxRow? {
        guard let m = fxMessage(id) else { return nil }
        return InboxRow(messageId: id, threadId: threadIdOf(m), accountId: account[id] ?? m.account, senderName: m.sender, subject: m.subject,
                        preview: m.preview, receivedAtMs: receivedAt[id] ?? 0, unread: !read.contains(id), pinned: pinned[id] ?? false,
                        labelNames: (assigned[id] ?? []).compactMap { lid in labelRows.first { $0.id == lid }?.name }, attachmentCount: UInt32(m.attachments?.count ?? 0), threadCount: UInt32(m.thread_count ?? 1) + UInt32(sentCountsLocked()[threadIdOf(m)] ?? 0),   // a reply the owner sent joins the count (round 15: the row's stamp then changes and the reply arrow is read again)
                        senderAddress: m.from, decidedBy: filedBy[id]?[category[id] ?? m.tab ?? "primary"],   // delta 13: the store is the one source
                        reminderFired: threadReminderFiredLocked(threadIdOf(m)))   // gap-remind: a fired "remind me if no reply" marks its thread's rows
    }

    /// gap-remind: the thread's reminder fired (its time passed with no reply, the contract's `replyReminderState`): the real core's `reminder_fired`.
    func threadReminderFiredLocked(_ thread: String) -> Bool {
        replyReminders.keys.contains { reminderThreadLocked($0) == thread && replyReminderInView(state: reminderStateLocked($0)) }
    }

    public func counts() -> Counts { lock.withLock { countsLocked() } }

    /// unread = the fixture tab's unread figure minus the unread fixture messages that left the tab.
    func countsLocked() -> Counts {
        func tabCount(_ tabId: String) -> TabCounts {
            guard let t = fx.tabs.first(where: { $0.id == tabId }) else {
                // a tab with a rule: unread = its unread messages still in the inbox; nothing is waiting on a rule
                let n = messages.filter { $0.unread && !read.contains($0.id) && !gone($0.id) && listable($0.id) && belongs($0.id, toTab: tabId) }.count
                return TabCounts(tabId: tabId, unread: UInt32(n), waiting: 0)
            }
            let left = messages.filter { ($0.tab ?? "primary") == t.id && $0.unread && gone($0.id) && !inserted.contains($0.id) }.count
            let arrived = messages.filter { inserted.contains($0.id) && ($0.tab ?? "primary") == t.id && $0.unread && !read.contains($0.id) && !gone($0.id) }.count
            // a message the rules or the models re-filed moves its unread between the built-in tabs (delta 9)
            let moved = messages.filter { $0.unread && !read.contains($0.id) && !gone($0.id) && category[$0.id] != nil }
            let into = moved.filter { category[$0.id] == t.id && ($0.tab ?? "primary") != t.id }.count
            let out = moved.filter { ($0.tab ?? "primary") == t.id && category[$0.id] != t.id }.count
            return TabCounts(tabId: t.id, unread: UInt32(max(0, t.unread - left + into - out + arrived)), waiting: UInt32(t.waiting))
        }
        // delta 16: the views come first. Primary is the primary category's figure, All is the sum of EVERY category's figure
        // (pinned or not), so a message read in any category lowers it and All is never below Primary. Junk, Trash and the
        // mailboxes are not categories, so they are not in it.
        let everyCategory = (["primary"] + builtinLabels().map(\.id)).map { tabCount($0) }
        let views = [tabCount("primary"),
                     TabCounts(tabId: inboxViewId(view: .all), unread: everyCategory.reduce(0) { $0 + $1.unread }, waiting: everyCategory.reduce(0) { $0 + $1.waiting })]
        let tabs = views + splitTabRows.filter(\.enabled).map { tabCount($0.id) }
        var boxes: [String: UInt32] = [:]
        for b in fx.mailboxes { if let c = b.count { boxes[b.name.lowercased()] = UInt32(c) } }
        // All inboxes is the inbox's own unread, the All view's figure, as the store builds it (counts.rs `build`): the file's fixed number stayed at 198 while
        // the All tab said 228 after the seeded mail, and never moved when a mail was read
        boxes["all inboxes"] = views[1].unread
        boxes["snoozed", default: 0] += UInt32(snoozed.count)
        for box in [Mailbox.junk, .trash, .archive].map({ mailboxKey(mailbox: $0) }) { boxes[box] = nil }
        for (box, n) in shownMailboxCountsLocked() { boxes[box] = n }
        return Counts(tabs: tabs, mailboxes: boxes)
    }

    /// The fixture's body paragraphs as the reader reads them: HTML escaped into <p>, quoted text in a
    /// <blockquote>; the same text plain, paragraphs split by a blank line. Offsets are UTF-8 bytes.
    public func body(messageId: String) -> MessageBody? {
        guard let m = fxMessage(messageId), let paragraphs = m.body else { return nil }
        // `fixture offline on` (the same door as the outbox): the phone holds no body it did not download, so the body is withheld until `off`.
        if lock.withLock({ outboxHold.offline }) { return nil }
        func esc(_ s: String) -> String {
            s.replacingOccurrences(of: "&", with: "&amp;").replacingOccurrences(of: "<", with: "&lt;").replacingOccurrences(of: ">", with: "&gt;")
        }
        let quoted = m.quoted ?? []
        // attachments hook: "[[inline:NAME]]" and "[[inline-data:BASE64]]" in a corpus body are inline images (cid: / data:).
        var html = paragraphs.map { "<p>\(FixtureInlineImages.html(esc($0)))</p>" }.joined()
        var plain = paragraphs.joined(separator: "\n\n")
        var quoteOffset: UInt32?
        var ranges: [TextRange] = []
        if !quoted.isEmpty {
            quoteOffset = UInt32(html.utf8.count)
            html += "<blockquote>" + quoted.map { "<p>\(esc($0))</p>" }.joined() + "</blockquote>"
            plain += "\n\n"
            let start = UInt32(plain.utf8.count)
            plain += quoted.map { "> " + $0 }.joined(separator: "\n")
            ranges = [TextRange(start: start, end: UInt32(plain.utf8.count))]
        }
        // delta 26 (body-wire): the mail content engine's fields, as the real core reads them back for a fetched body. The fixture's own
        // classification: its paragraphs are the sender's words (shown), its quoted lines the reply history (collapsed, behind the fold);
        // a fixture mail is written (it reads in the app's colours). Segment offsets are UTF-8 bytes of `text`'s canonical text, here `plain`.
        let authored = paragraphs.joined(separator: "\n\n")
        let authoredHtml = paragraphs.map { "<p>\(FixtureInlineImages.html(esc($0)))</p>" }.joined()
        var segments = [BodySegment(kind: .newContent, text: authored, safeHtml: authoredHtml, confidence: 1.0, visibility: .show,
                                    reasons: ["no_history_signal"], start: 0, end: UInt32(authored.utf8.count))]
        var foldFrom: UInt32?
        if let start = ranges.first?.start {
            let quotedText = String(decoding: Array(plain.utf8)[Int(start)...], as: UTF8.self)
            segments.append(BodySegment(kind: .replyHistory, text: quotedText, safeHtml: "<blockquote>" + quoted.map { "<p>\(esc($0))</p>" }.joined() + "</blockquote>",
                                        confidence: 0.8, visibility: .collapse, reasons: ["quote_marks"], start: start, end: UInt32(plain.utf8.count)))
            foldFrom = 1
        }
        if lock.withLock({ designedForRig.contains(messageId) }) {
            // the rig's designed mail: the same words on a page of its own; the pictures keep their address in data-whoosh-src (never src), as the engine leaves them
            let page = "<table width=\"600\" style=\"background:#ffffff\"><tr><td style=\"padding:24px\"><img data-whoosh-src=\"https://pictures.example.test/hero.png\" width=\"552\" height=\"120\" alt=\"\">"
                + authoredHtml + "<img data-whoosh-src=\"https://pictures.example.test/foot.png\" width=\"120\" height=\"40\" alt=\"\"></td></tr></table>"
            return MessageBody(messageId: messageId, safeHtml: page, plainText: plain, trackersBlocked: 3, quoteOffset: nil, quotedRanges: [], segments: segments,
                               foldFrom: foldFrom, layout: .designed, layoutReasons: ["rig_designed"], displayHtml: page, writtenHtml: nil, bodyWarnings: [],
                               picturesBlocked: 2, pixelTrackers: 1)
        }
        return MessageBody(messageId: messageId, safeHtml: html, plainText: plain, trackersBlocked: UInt32(m.trackers_blocked ?? 0),
                           quoteOffset: quoteOffset, quotedRanges: ranges, segments: segments, foldFrom: foldFrom, layout: .written,
                           layoutReasons: ["no_design_signal"], displayHtml: html, writtenHtml: html, bodyWarnings: [],
                           picturesBlocked: 0, pixelTrackers: UInt32(m.trackers_blocked ?? 0))
    }

    /// unread = the fixture's own figure for the label plus the unread messages the owner labelled since
    /// (still in the inbox, not opened). Own count only; the sidebar rolls children up from parentId.
    public func labels() -> [MailLabel] { lock.withLock { labelsLocked() } }

    /// The fixture's smart folders in order, ids sf-1.. (the fixture holds no nesting yet: parentId is nil).
    public func smartFolders() -> [SmartFolder] { lock.withLock { smartFoldersLocked() } }

    func smartFoldersLocked() -> [SmartFolder] {
        folderRows
    }

    /// Accounts as the lists show them. unread = the fixture's figure minus that account's unread rows that
    /// left the inbox (the same derivation as the tab counts); health = the account's sync failure, if any.
    public func accounts() -> [AccountSummary] {
        let rows = accountsInFixtureOrder()
        return lock.withLock {
            guard let o = accountOrder else { return rows }
            return rows.sorted { (o.firstIndex(of: $0.id) ?? Int.max) < (o.firstIndex(of: $1.id) ?? Int.max) }
        }
    }

    func accountsInFixtureOrder() -> [AccountSummary] {
        lock.withLock {
            if noAccounts { return [] }
            let failing = Dictionary(uniqueKeysWithValues: syncStatesLocked().compactMap { s in s.lastError.map { (s.accountId, $0) } })
            return (fx.accounts.map { a in
                let left = messages.filter { $0.account == a.id && $0.unread && gone($0.id) }.count
                return AccountSummary(id: a.id, name: accountNames[a.id] ?? a.name, address: a.address, provider: MailProvider(words: a.provider), colour: settings.colours[a.id] ?? a.colour,
                                      iconId: settings.iconId(a.id, fallback: a.icon_id), unread: UInt32(max(0, (a.unread ?? 0) - left)), health: failing[a.id],
                                      group: a.group, paused: (a.paused ?? false) || pausedAccounts.contains(a.id), backfill: a.backfill,
                                      signature: accountSignatures[a.id] ?? "", iconFile: settings.iconFile(a.id))
            } + added.map { a in
                AccountSummary(id: a.id, name: accountNames[a.id] ?? a.address, address: a.address, provider: a.provider, colour: settings.colours[a.id] ?? "", iconId: settings.iconId(a.id, fallback: nil), unread: 0, health: nil, group: nil,
                               paused: pausedAccounts.contains(a.id), backfill: nil, signature: accountSignatures[a.id] ?? "", iconFile: settings.iconFile(a.id))
            }).filter { !removedAccounts.contains($0.id) }
        }
    }

    /// `fixture accounts none|all` (lane brandmark-r2): the core lists no account, or every account again.
    public func fixtureAccounts(none: Bool) {
        lock.withLock { noAccounts = none }
        emit()
    }

    /// `fixture corpus <path>`: a second fixture file's messages replace the rows (accounts, tabs and labels
    /// stay), so a check runs on its own corpus. Every per-message state starts over.
    public func loadCorpus(from url: URL) throws {
        let corpus: CorpusFile
        do { corpus = try JSONDecoder().decode(CorpusFile.self, from: Data(contentsOf: url)) }
        catch { throw CoreError.Invalid(field: "corpus", reasonWords: "\(url.lastPathComponent) is not a whoosh-search-corpus/1 file: \(error.localizedDescription)") }
        guard corpus.schema == "whoosh-search-corpus/1" else {
            throw CoreError.Invalid(field: "corpus", reasonWords: "\(url.lastPathComponent) is a \(corpus.schema) file; the fixture core reads whoosh-search-corpus/1")
        }
        let sync: [(StreamKind, EventSink)] = lock.withLock {
            messages = corpus.asMessages
            resetMessages()
            undo = []; consumedLast = false; entries = []
            agents.reset()
            // the corpus brings its own labels and smart folders; a file that lists none leaves none
            labelRows = []; labelRuleSets = [:]; entryChainIds = [:]; exitChains = [:]; flowCard = [:]; flowRejected = [:]; flowEdited = []; replyBookStore = nil; filedBy = [:]; category = [:]; vipSenders = [:]; rerunState = [:]; suggestionRows = []; snoozesOfThread = [:]; labelsOfSender = [:]; blockRows = []
            buildLabels(corpus.labels ?? [], parent: nil)
            pinnedFromCorpus = corpus.pinned_labels
            installBuiltInLabels()
            seedPlacementsLocked()
            folderRows = Self.folders(corpus.smart_folders ?? [])
            return sinks.values.filter { $0.0 == .syncStatus }
        }
        emit()
        // contracts delta 12: the subject travels in the embedder's title slot
        MeaningSearch.warm(lock.withLock { messages.map { (id: $0.id, title: Optional($0.subject), body: ($0.body ?? [$0.preview]).joined(separator: " ")) } })
        let attached = FixtureAttachments.book(from: (corpus.attachment_content ?? [:]).compactMap { name, e in e.entry.map { FixtureAttachmentSpec(fileName: name, message: $0.message, kind: $0.kind, state: $0.state, text: $0.text) } })
        lock.withLock { attachmentBook = attached }
        // the read files of this corpus are pieces of their mails for the meaning search (FixtureSearchBook.meaningReads); a corpus with none clears them
        FixtureSearchBook.corpusReads = (corpus.attachment_content ?? [:]).compactMap { name, e in
            guard let x = e.entry, x.state == "indexed", !x.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return nil }
            return FixtureSearchBook.AttachmentRead(messageId: x.message, name: name, text: x.text)
        }.sorted { ($0.messageId, $0.name) < ($1.messageId, $1.name) }
        let states = syncStates()
        Self.deliver(sync.flatMap { _, sink in states.map { (sink, CoreEvent.syncStateChanged(state: $0)) } })
    }

    /// Per-message state from `messages`, and each row's received time. Called with the lock held (or from init).
    /// `known`: the received times the caller made the rows from (the full-history fixture, launch-r3); nil derives them from each row.
    /// Only the launch corpus (init, `fixture reset`) carries the three seeded sent mails: a corpus loaded from a file, or the full-history one, is not the
    /// fixture file's own mail, so its counts stay its own (`index/message_count` reads exactly its messages).
    func resetMessages(receivedAt known: [String: Int64]? = nil) {
        let isLaunchCorpus = messages.count == fx.messages.count && messages.map(\.id) == fx.messages.map(\.id)
        if isLaunchCorpus, let a = fx.accounts.first {
            messages += FixtureSearchBook.sentSeed(account: a.id, from: a.address)   // sent mail for the people reads (never an inbox row: `listable`)
        }
        order = messages.map(\.id)
        starred = []; muted = []; spam = []
        repliedByMe = []; unansweredState = [:]
        inserted = []; archiveRefused = [:]
        archived = []; deleted = []; snoozed = [:]; pinned = [:]; read = []; account = [:]; assigned = [:]; receivedAt = [:]
        sentCountsStamp = -1; sentCountsMemo = [:]   // the thread rows' count of the owner's sent replies is read again from the restored mail (a sent reply must not survive a reset)
        buildContactCards()
        for m in messages {
            pinned[m.id] = m.pinned
            account[m.id] = m.account
            if !m.unread { read.insert(m.id) }
        }
        if let known { receivedAt = known; return }
        var seen: [String: Int] = [:]
        for m in messages {
            let k = seen[m.time, default: 0]; seen[m.time] = k + 1
            receivedAt[m.id] = Self.receivedAtMs(m, sameLabelIndex: k, now: clock.nowMs())
        }
    }

    /// The fixture writes times as the list shows them. An absolute `date` wins; "09:12" is today; "Yesterday"
    /// is yesterday; "Thu" is the latest past Thursday (a week ago when today is Thursday). Rows sharing a
    /// relative label get earlier times in list order (18:00 then 7 minutes apart; 12:00 for weekdays), so the
    /// list stays newest first. A label the rule does not know gives nil, served as 0.
    /// One parser for every corpus date (delta 13: the full-history fixture parses 300,000 of them; a formatter per row
    /// made that load take seconds). Only ever used with the core's lock held.
    nonisolated(unsafe) static let isoLocal: DateFormatter = {
        let f = DateFormatter(); f.locale = Locale(identifier: "en_US_POSIX"); f.timeZone = .current
        f.dateFormat = "yyyy-MM-dd'T'HH:mm:ss"
        return f
    }()

    static func receivedAtMs(_ m: FixtureFile.Message, sameLabelIndex k: Int, now nowMs: Int64) -> Int64? {
        var cal = Calendar(identifier: .gregorian); cal.timeZone = .current
        let now = Date(timeIntervalSince1970: Double(nowMs) / 1000)
        func ms(_ d: Date?) -> Int64? { d.map { Int64($0.timeIntervalSince1970 * 1000) } }
        if let iso = m.date { return ms(isoLocal.date(from: iso)) }
        let t = m.time.trimmingCharacters(in: .whitespaces)
        let hm = t.split(separator: ":")
        if hm.count == 2, let h = Int(hm[0]), let mi = Int(hm[1]) {
            return ms(cal.date(bySettingHour: h, minute: mi, second: 0, of: now))
        }
        let staggered = { (day: Date, hour: Int) -> Int64? in
            ms(cal.date(bySettingHour: hour, minute: 0, second: 0, of: day).map { $0.addingTimeInterval(Double(-7 * 60 * k)) })
        }
        if t == "Yesterday" { return cal.date(byAdding: .day, value: -1, to: now).flatMap { staggered($0, 18) } }
        let days = ["Sun": 1, "Mon": 2, "Tue": 3, "Wed": 4, "Thu": 5, "Fri": 6, "Sat": 7]
        if let wd = days[t] {
            let today = cal.component(.weekday, from: now)
            var back = (today - wd + 7) % 7
            if back == 0 { back = 7 }
            return cal.date(byAdding: .day, value: -back, to: now).flatMap { staggered($0, 12) }
        }
        return nil
    }

    static func labelRow(id: String, name: String, parent: MailLabel?) -> MailLabel {
        // a parent's path was built by labelPath, so it always parses; its names come back in order
        let up: [String]
        if let p = parent { up = (try? parseLabelPath(path: p.path)) ?? [p.name] } else { up = [] }
        let names = up + [name]
        return MailLabel(id: id, name: name, parentId: parent?.id, colour: nil, accountIds: [],
                         path: labelPath(names: names), depth: UInt8(names.count),
                         unread: 0, description: "", pinned: false, scope: .all, kind: .owner, order: 0,
                         entryEnabled: true, aiEnabled: true, exitEnabled: false, aiReads: defaultAiReads(), exitFallback: nil)
    }

    /// A single name matching exactly one label anywhere means that label; otherwise the path is read
    /// from the root, creating the missing levels when asked. The depth rule is the contract's
    /// (`parseLabelPath`, LABEL_DEPTH_MAX), so a sixth level is refused in its words.
    func resolveLabel(_ text: String, create: Bool) throws -> String {
        let names = try parseLabelPath(path: text)
        if names.count == 1 {
            let same = labelRows.filter { $0.name.caseInsensitiveCompare(names[0]) == .orderedSame }
            if same.count == 1 { return same[0].id }
        }
        var parent: MailLabel?
        for (i, n) in names.enumerated() {
            let path = labelPath(names: Array(names[...i]))
            if let found = labelRows.first(where: { $0.path.caseInsensitiveCompare(path) == .orderedSame }) {
                parent = found; continue
            }
            guard create else { throw CoreError.NotFound(kind: "label", id: path) }
            let row = Self.labelRow(id: "lbl-\(labelRows.count + 1)", name: n, parent: parent)
            let at = parent.flatMap { p in lastDescendantIndex(of: p.id) }.map { $0 + 1 } ?? labelRows.count
            labelRows.insert(row, at: at)
            parent = row
        }
        guard let leaf = parent else { throw CoreError.Invalid(field: "label", reasonWords: "a label needs a name") }
        return leaf.id
    }

    /// The index of the last row in the subtree under `id` (the parent itself when it has no children).
    func lastDescendantIndex(of id: String) -> Int? {
        guard var i = labelRows.firstIndex(where: { $0.id == id }) else { return nil }
        var inTree: Set<String> = [id]
        while i + 1 < labelRows.count, let p = labelRows[i + 1].parentId, inTree.contains(p) { i += 1; inTree.insert(labelRows[i].id) }
        return i
    }


    // MARK: label commands (delta 6)

    func labelDepthWords(_ path: String, _ depth: Int) -> String {
        "a label can be at most \(WhooshConstants.LABEL_DEPTH_MAX) levels deep; \"\(path)\" has \(depth)"
    }

    /// One name, trimmed; never a path, never empty.
    func singleLabelName(_ text: String) throws -> String {
        let n = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if n.contains(">") { throw CoreError.Invalid(field: "name", reasonWords: "a label name cannot hold \">\"; make the parent first") }
        if n.isEmpty { throw CoreError.Invalid(field: "name", reasonWords: "a label needs a name") }
        return n
    }

    func sameParent(_ a: String?, _ b: String?) -> Bool { a == b }

    func createLabel(_ name: String, parentId: String?) throws -> String {
        let n = try singleLabelName(name)
        var parent: MailLabel?
        if let pid = parentId {
            guard let p = labelRows.first(where: { $0.id == pid }) else { throw CoreError.NotFound(kind: "label", id: pid) }
            parent = p
        }
        let depth = (parent.map { Int($0.depth) } ?? 0) + 1
        let path = labelPath(names: ((parent.map { (try? parseLabelPath(path: $0.path)) ?? [$0.name] }) ?? []) + [n])
        if depth > WhooshConstants.LABEL_DEPTH_MAX { throw CoreError.Invalid(field: "label", reasonWords: labelDepthWords(path, depth)) }
        if labelRows.contains(where: { sameParent($0.parentId, parentId) && $0.name.caseInsensitiveCompare(n) == .orderedSame }) {
            throw CoreError.Invalid(field: "name", reasonWords: "\"\(n)\" exists already here")
        }
        var maxN = 0
        for l in labelRows { if l.id.hasPrefix("lbl-"), let k = Int(l.id.dropFirst(4)) { maxN = max(maxN, k) } }
        let row = Self.labelRow(id: "lbl-\(maxN + 1)", name: n, parent: parent)
        let at = parent.flatMap { lastDescendantIndex(of: $0.id) }.map { $0 + 1 } ?? labelRows.count
        labelRows.insert(row, at: at)
        return row.id
    }

    /// The label and everything under it, in tree order.
    func subtree(of id: String) -> [MailLabel] {
        guard let i = labelRows.firstIndex(where: { $0.id == id }), let last = lastDescendantIndex(of: id) else { return [] }
        return Array(labelRows[i...last])
    }

    /// Rewrites path and depth of `rows` (a subtree whose root now sits under `parent`), keeping tree order.
    func relabel(_ rows: [MailLabel], rootName: String, under parent: MailLabel?) -> [MailLabel] {
        var out: [MailLabel] = []
        var byId: [String: MailLabel] = [:]
        for (k, old) in rows.enumerated() {
            let up = old.parentId.flatMap { byId[$0] } ?? (k == 0 ? parent : nil)
            var fresh = Self.labelRow(id: old.id, name: k == 0 ? rootName : old.name, parent: up)
            fresh.colour = old.colour; fresh.accountIds = old.accountIds; fresh.unread = old.unread
            fresh.description = old.description; fresh.pinned = old.pinned; fresh.scope = old.scope; fresh.kind = old.kind; fresh.order = old.order
            if k == 0 { fresh.parentId = parent?.id }
            byId[old.id] = fresh
            out.append(fresh)
        }
        return out
    }

    func renameLabel(_ id: String, _ name: String) throws {
        guard let label = labelRows.first(where: { $0.id == id }) else { throw CoreError.NotFound(kind: "label", id: id) }
        try checkLabelRename(label: label)
        let n = try singleLabelName(name)
        if labelRows.contains(where: { $0.id != id && sameParent($0.parentId, label.parentId) && $0.name.caseInsensitiveCompare(n) == .orderedSame }) {
            throw CoreError.Invalid(field: "name", reasonWords: "\"\(n)\" exists already here")
        }
        let parent = label.parentId.flatMap { pid in labelRows.first { $0.id == pid } }
        let rows = subtree(of: id)
        let fresh = relabel(rows, rootName: n, under: parent)
        let start = labelRows.firstIndex(where: { $0.id == id }) ?? 0
        labelRows.replaceSubrange(start..<(start + rows.count), with: fresh)
    }

    func moveLabel(_ id: String, to parentId: String?) throws {
        guard let label = labelRows.first(where: { $0.id == id }) else { throw CoreError.NotFound(kind: "label", id: id) }
        if label.kind == .builtIn, parentId != nil { throw CoreError.Invalid(field: "parent", reasonWords: "\(label.name) is a built-in category: it stays at the top level") }
        var parent: MailLabel?
        if let pid = parentId {
            guard let p = labelRows.first(where: { $0.id == pid }) else { throw CoreError.NotFound(kind: "label", id: pid) }
            parent = p
        }
        let rows = subtree(of: id)
        if let p = parent, rows.contains(where: { $0.id == p.id }) {
            throw CoreError.Invalid(field: "parent", reasonWords: "\"\(label.name)\" cannot go inside itself")
        }
        if sameParent(label.parentId, parentId) { return }
        let newDepth = (parent.map { Int($0.depth) } ?? 0) + 1
        let deepest = (rows.map { Int($0.depth) }.max() ?? Int(label.depth)) - Int(label.depth) + newDepth
        if deepest > WhooshConstants.LABEL_DEPTH_MAX {
            let deepRow = rows.max { $0.depth < $1.depth } ?? label
            throw CoreError.Invalid(field: "label", reasonWords: labelDepthWords(labelPath(names: ((parent.map { (try? parseLabelPath(path: $0.path)) ?? [$0.name] }) ?? []) + [deepRow.path.components(separatedBy: " > ").dropFirst(Int(label.depth) - 1).joined(separator: " > ")]), deepest))
        }
        if labelRows.contains(where: { sameParent($0.parentId, parentId) && $0.name.caseInsensitiveCompare(label.name) == .orderedSame }) {
            throw CoreError.Invalid(field: "name", reasonWords: "\"\(label.name)\" exists already there")
        }
        let fresh = relabel(rows, rootName: label.name, under: parent)
        let start = labelRows.firstIndex(where: { $0.id == id }) ?? 0
        labelRows.removeSubrange(start..<(start + rows.count))
        let at = parent.flatMap { p in lastDescendantIndex(of: p.id) }.map { $0 + 1 } ?? labelRows.count
        labelRows.insert(contentsOf: fresh, at: at)
    }

    // MARK: contacts (delta 6)

    /// One card per sender address, in order of first appearance; ids ct-1.. Called with the lock held (or from init).
    func buildContactCards() {
        var cards: [Contact] = []
        for m in messages {
            let a = m.from.lowercased()
            if let i = cards.firstIndex(where: { $0.addresses.contains(a) }) {
                cards[i].mailCountPerAccount[m.account, default: 0] += 1
            } else {
                cards.append(Contact(id: "ct-\(cards.count + 1)", name: m.sender, addresses: [a], logoId: nil, mailCountPerAccount: [m.account: 1]))
            }
        }
        contactCards = cards
    }

    /// One entry per person: look-alike cards (the same name from several addresses) are shown together with every address;
    /// the fixture's `contacts` block names addresses a person has that no mail came from yet. `contact(address:)` and
    /// MergeContacts still work on the cards, so two look-alike cards stay mergeable.
    public func contacts() -> [Contact] { lock.withLock { contactsLocked() } }

    func contactsLocked() -> [Contact] {
        var people: [Contact] = []
        for card in contactCards {
            if let i = people.firstIndex(where: { $0.name.caseInsensitiveCompare(card.name) == .orderedSame }) {
                people[i].addresses += card.addresses.filter { !people[i].addresses.contains($0) }
                for (acc, n) in card.mailCountPerAccount { people[i].mailCountPerAccount[acc, default: 0] += n }
            } else { people.append(card) }
        }
        for decl in fx.contacts ?? [] {
            guard let i = people.firstIndex(where: { $0.name.caseInsensitiveCompare(decl.name) == .orderedSame }) else { continue }
            people[i].addresses += decl.addresses.map { $0.lowercased() }.filter { !people[i].addresses.contains($0) }
        }
        return people.enumerated().sorted {
            let a = $0.element.mailCountPerAccount.values.reduce(0, +), b = $1.element.mailCountPerAccount.values.reduce(0, +)
            return a != b ? a > b : $0.offset < $1.offset
        }.map(\.element)
    }

    /// The sender domains the typed letters start, with their people and mails, from the fixture mail (FixtureSearch.swift `domainsOfSenders`; SPEC A153).
    public func senderDomains(prefix: String) -> [SenderDomain] {
        let owner = Set(fx.accounts.map { $0.address.lowercased() })
        return lock.withLock { Self.domainsOfSenders(in: messages, deleted: deleted, owner: owner, prefix: prefix) }
    }

    public func contact(address: String) -> Contact? {
        let a = address.trimmingCharacters(in: .whitespaces).lowercased()
        return lock.withLock { contactCards.first { $0.addresses.contains(a) } }
    }

    public func contactMessages(contactId: String) -> [InboxRow] {
        lock.withLock {
            guard let c = contactCards.first(where: { $0.id == contactId }) else { return [] }
            return order.compactMap { id -> InboxRow? in
                guard let m = fxMessage(id), listable(id), c.addresses.contains(m.from.lowercased()) else { return nil }
                return inboxRow(id)
            }.sorted { $0.receivedAtMs > $1.receivedAtMs }
        }
    }

    func mergeContacts(keep: String, merge: String) throws {
        guard let k = contactCards.firstIndex(where: { $0.id == keep }) else { throw CoreError.NotFound(kind: "contact", id: keep) }
        guard let m = contactCards.firstIndex(where: { $0.id == merge }) else { throw CoreError.NotFound(kind: "contact", id: merge) }
        if k == m { throw CoreError.Invalid(field: "merge", reasonWords: "a card cannot be merged into itself") }
        let gone = contactCards[m]
        contactCards[k].addresses += gone.addresses.filter { !contactCards[k].addresses.contains($0) }
        for (acc, n) in gone.mailCountPerAccount { contactCards[k].mailCountPerAccount[acc, default: 0] += n }
        contactCards.remove(at: m)
    }

    // MARK: one message, and rule counts (delta 6)

    public func message(messageId: String) -> Message? {
        lock.withLock {
            if let m = fxMessage(messageId) {
                let owner = fx.accounts.first { $0.id == (account[messageId] ?? m.account) }?.address ?? ""
                let folder = deleted.contains(messageId) ? "Trash" : m.outgoing == true ? "Sent" : archived.contains(messageId) ? "Archive" : snoozed[messageId] != nil ? "Snoozed" : "Inbox"
                return Message(id: messageId, threadId: threadIdOf(m), accountId: account[messageId] ?? m.account, senderName: m.sender, fromAddress: m.from,
                               to: m.to ?? [owner], subject: m.subject, preview: m.preview, receivedAtMs: receivedAt[messageId] ?? 0, read: read.contains(messageId),
                               archived: archived.contains(messageId), folder: folder, attachmentNames: m.attachments ?? [], loginCode: m.login_code,
                               trackersBlocked: UInt32(m.trackers_blocked ?? 0),
                               listUnsubscribe: m.list_unsubscribe ?? [], listUnsubscribeOneClick: m.list_unsubscribe_one_click ?? false,
                               replyToName: Self.replyToName(sender: m.sender, body: m.body), cc: m.cc ?? [], bcc: m.bcc ?? [], isOutgoing: m.outgoing ?? false)
            }
            // a message of the design fixture's open thread that is not a row of the list
            guard let t = fx.open_thread, let tm = t.messages?.first(where: { $0.id == messageId }) else { return nil }
            let address = t.participants.compactMap { p -> String? in
                guard p.hasPrefix(tm.from), let a = p.firstIndex(of: "<"), let b = p.firstIndex(of: ">") else { return nil }
                return String(p[p.index(after: a)..<b])
            }.first ?? ""
            let owner = fx.accounts.first { $0.id == t.account }?.address ?? ""
            return Message(id: messageId, threadId: t.id, accountId: t.account, senderName: tm.from, fromAddress: address, to: [owner], subject: t.subject,
                           preview: "", receivedAtMs: Self.threadTimeMs(tm.time ?? "", now: clock.nowMs()) ?? 0, read: true, archived: false, folder: "Inbox",
                           attachmentNames: tm.attachments ?? [], loginCode: nil, trackersBlocked: 0, replyToName: nil)
        }
    }

    /// "Mon 14:02", "Today 09:12": a weekday name or Today, then a clock time. The latest past such weekday.
    static func threadTimeMs(_ label: String, now nowMs: Int64) -> Int64? {
        let parts = label.split(separator: " ").map(String.init)
        guard parts.count == 2 else { return nil }
        let hm = parts[1].split(separator: ":")
        guard hm.count == 2, let h = Int(hm[0]), let mi = Int(hm[1]) else { return nil }
        var cal = Calendar(identifier: .gregorian); cal.timeZone = .current
        let now = Date(timeIntervalSince1970: Double(nowMs) / 1000)
        var day = now
        if parts[0] != "Today" {
            let days = ["Sun": 1, "Mon": 2, "Tue": 3, "Wed": 4, "Thu": 5, "Fri": 6, "Sat": 7]
            guard let wd = days[parts[0]] else { return nil }
            var back = (cal.component(.weekday, from: now) - wd + 7) % 7
            if back == 0 { back = 7 }
            day = cal.date(byAdding: .day, value: -back, to: now) ?? now
        }
        return cal.date(bySettingHour: h, minute: mi, second: 0, of: day).map { Int64($0.timeIntervalSince1970 * 1000) }
    }

    /// The editor's count for a rule in words: the same reading as a saved folder's `smart_folder/<id>/count` (`ruleLiveIdsLocked`),
    /// so a rule typed in the editor and the folder saved from it show one number, and an archive moves both.
    public func ruleMatchCount(ruleWords: String) -> UInt32 {
        lock.withLock { UInt32(ruleLiveIdsLocked(ruleWords).count) }
    }

    /// True when the last answer of the latest Ask conversation cites the message.
    func answers(citing id: String) -> Bool {
        guard let c = agents.lastConversationId, let a = agents.answerFor(c) else { return false }
        return a.citations.contains { $0.messageId == id }
    }

    /// The fixture holds one signature: the quick reply's, "Assem Chammah · Nordlicht" for acc-04's address.
    public func signature(address: String) -> Signature? {
        let a = address.trimmingCharacters(in: .whitespaces).lowercased()
        // the account's own signature (SetSignature, delta 11) wins over the fixture's quick-reply one
        let own: Signature? = lock.withLock {
            guard let acc = fx.accounts.first(where: { $0.address.lowercased() == a })?.id ?? added.first(where: { $0.address == a })?.id,
                  let t = accountSignatures[acc] else { return nil }
            return Signature(address: address, text: t)
        }
        if let own { return own }
        guard let q = fx.open_thread?.quick_reply, q.from.lowercased() == a else { return nil }
        return Signature(address: q.from, text: q.signature)
    }

    /// The first name of the person who signed a mail an organisation sent: the line after a closing line that ends in
    /// a comma ("Best,"), at most five words, and not the sender's own name ("LHG Procurement" signs as LHG Procurement).
    static func replyToName(sender: String, body: [String]?) -> String? {
        guard let body, body.count >= 2, body[body.count - 2].hasSuffix(",") else { return nil }
        let line = body[body.count - 1].trimmingCharacters(in: .whitespacesAndNewlines)
        let words = line.split(whereSeparator: { $0 == " " || $0 == "," }).map(String.init)
        guard let first = words.first, words.count <= 5, first.first?.isUppercase == true, first.allSatisfy({ $0.isLetter }) else { return nil }
        return sender.lowercased().hasPrefix(first.lowercased()) ? nil : first
    }

    /// Entries whose undo window has passed, and scheduled ones whose time has come, are sent: state, tick and SendCompleted at the window's end. Lock held.
    func settleOutboxLocked() {
        let now = clock.nowMs()
        for i in entries.indices {
            // An undoable entry leaves when its window ends, a scheduled one at its time (the real outbox's Primary sends at the time),
            // one that waited for the network when the network is back. Once: the entry is `.sent` afterwards, so a second settle
            // finds nothing to send.
            let due: Int64?
            switch entries[i].state {
            case .undoable(let until): due = until
            case .scheduled(let at): due = at
            case .waitingForNetwork: due = outboxHold.offline ? nil : now
            default: due = nil
            }
            guard let at = due, now >= at else { continue }
            if outboxHold.offline {   // `fixture offline on`: due, but no path; not a failure (OutboxState.waitingForNetwork)
                entries[i].state = .waitingForNetwork(sinceMs: at)
                pending.append((.outbox, .outboxTick(entry: entries[i])))
            } else if let why = outboxHold.refusal {   // `fixture server-refuse <reason>`: the draft stays with the words and a Retry
                let failure = ActionFailed(accountName: accountName(entries[i].draft.fromAccountId), reasonWords: why, retryable: true)
                entries[i].state = .failed(failure: failure, attempts: 1)
                pending.append((.outbox, .outboxTick(entry: entries[i])))
            } else {
                entries[i].state = .sent(atMs: at)
                pending.append((.outbox, .outboxTick(entry: entries[i])))
                pending.append((.outbox, .sendCompleted(outboxId: entries[i].id, sentAtMs: at)))
                joinThreadLocked(entries[i], sentAtMs: at)
            }
        }
    }

    /// `fixture offline on|off` and `fixture server-refuse <reason>|off`: what the test engine's outbox does with a send that is due.
    /// `fixture reset` must put it back (FixtureReset.swift: `outboxHold = OutboxHold()`).
    public func fixtureOutbox(offline: Bool) { lock.withLock { outboxHold.offline = offline; settleOutboxLocked() } }
    public func fixtureServerRefuse(_ reason: String?) { lock.withLock { outboxHold.refusal = reason; settleOutboxLocked() } }

    /// Delta 15: a sent reply joins its thread, once: a message row from the sending account's address, outgoing, dated
    /// the send, holding the draft's words, never an inbox row (archived and "Sent"); a ThreadChanged event follows.
    /// A send that failed or was cancelled never reaches here (it never becomes `sent`).
    func joinThreadLocked(_ e: OutboxEntry, sentAtMs: Int64) {
        guard let replyTo = e.draft.replyToThreadId else { return }
        let thread = threadOf(replyTo)
        let id = "sent-\(e.id)"
        guard fxMessage(id) == nil else { return }
        let address = fx.accounts.first { $0.id == e.draft.fromAccountId }?.address ?? added.first { $0.id == e.draft.fromAccountId }?.address ?? ""
        let subject = e.draft.subject.isEmpty ? (fxMessage(replyTo)?.subject ?? "") : e.draft.subject
        var row = FixtureFile.Message(id: id, sender: address, from: address, account: e.draft.fromAccountId, subject: subject, preview: String(e.draft.body.prefix(160)),
                                      time: "", unread: false, pinned: false, attachments: nil, thread_count: 1, trackers_blocked: 0, tab: "primary",
                                      body: [e.draft.body], quoted: nil, date: nil, thread: thread)
        row.outgoing = true
        row.to = e.draft.to; row.cc = e.draft.cc; row.bcc = e.draft.bcc
        messages.append(row)
        order.append(id)
        account[id] = e.draft.fromAccountId
        receivedAt[id] = sentAtMs
        read.insert(id)
        archived.insert(id)   // out of the inbox for every reader that checks `gone`; d15b: `listable` keeps it out of Archive's list and count too
        if let t = threadLocked(thread) { pending.append((.thread, .threadChanged(thread: t))) }
    }

    /// The clock moved: settle the outbox and tell the listeners (`clock advance` calls this).
    func clockMoved() {
        settleKeyChecksAfterClock()
        lock.withLock { settleOutboxLocked() }
        emit()
    }

    public func notes(target: NoteTarget) -> [Note] { lock.withLock { noteRows.filter { $0.target == target } } }

    func threadExistsLocked(_ id: String) -> Bool {
        messages.contains { threadIdOf($0) == id } || fx.open_thread?.id == id
    }
    func messageIdKnownLocked(_ id: String) -> Bool {
        fxMessage(id) != nil || (fx.open_thread?.messages ?? []).contains { $0.id == id }
    }

    public func shortcutPreset() -> ShortcutPreset { lock.withLock { ShortcutPreset(preset: presetName, overrides: [:]) } }
    public func notchSetting() -> NotchSetting { lock.withLock { settings.notch } }
    public func notchAlerts() -> NotchAlerts { lock.withLock { settings.alerts } }
    public func notchKind(messageId: String) -> NotchKind { lock.withLock { notchKindLocked(messageId) } }
    /// A test copy's door for the notch's alert check: one mail is filed under a category tab ("updates", "promotions", "vips", "needs-reply"). The
    /// fixture's sample mail is all Primary and VIPs is not a label a command can file under, so a drive needs this for a mail of every kind.
    public func fileForRig(messageId: String, tab: String) { lock.withLock { category[messageId] = tab } }
    /// A test copy's door for the notch's full-mail check: this mail becomes a DESIGNED one (a page of its own, two remote pictures, one tracking
    /// pixel). Every sample mail of the fixture is written, so a drive needs this for a mail that has the "Full mail" button.
    public func designForRig(messageId: String) { lock.withLock { _ = designedForRig.insert(messageId) } }
    /// The kind of a fixture mail, by the contract's one rule (`notchKindOf`) over the fixture's own facts: its sign-in code, its tab (after a
    /// re-filing, the new one; the VIPs tab is the fixture's VIP fact, as in `FixtureLabels`), its list header. Primary for an id the fixture does not hold.
    func notchKindLocked(_ id: String) -> NotchKind {
        guard let m = fxMessage(id) else { return .primary }
        let tab = category[m.id] ?? m.tab
        return notchKindOf(facts: NotchFacts(hasSignInCode: !(m.login_code ?? "").trimmingCharacters(in: .whitespaces).isEmpty, vipSender: tab == "vips",
                                             tabId: tab, hasListHeader: !(m.list_unsubscribe ?? []).isEmpty))
    }
    public func notificationSettings() -> [NotificationSetting] { lock.withLock { accountIdsLocked().map { settings.notification($0) } } }
    /// `fixture fail-next notch-setting-save|notification-setting-save <reason>`: the next such save is refused in
    /// the reason's words. False for a kind the core does not know.
    @discardableResult
    /// Arms one refusal of the next `kind` action. "send" (delta 12): the next QuickSend, QuickSendFrom or Send fails with
    /// `reason` and the server `code` ("451 4.7.1"), as the real send path reports a refusing server.
    public func failNext(_ kind: String, reason: String, code: String? = nil) -> Bool { lock.withLock { settings.failNext(kind, reason: reason, code: code) } }

    /// Names where the provider answers for key checks (the journeys' stub on 127.0.0.1); an empty `url` removes it.
    public func setProviderEndpoint(_ provider: String, url: String) {
        lock.withLock { providerEndpoints[provider] = url.isEmpty ? nil : url }
    }

    /// `fixture reset` for the AI pane: keys and checks forgotten, providers off, agents the owner made removed. Built-in agents stay.
    public func resetAISettings() {
        lock.withLock {
            agents.keyChecks = [:]
            agents.credentialStored.subtract(FixtureSettingsBook.credentialIds)
            for i in agents.providers.indices where agents.providers[i].id != "local" {
                agents.providers[i].connected = false
                agents.providers[i].keychain = nil
            }
            agents.agents.removeAll { !$0.builtIn }
            if settings.decision != nil { settings.decision?.model = nil }   // the on-device model in use is the catalogue's first line again (SPEC A133)
        }
    }

    public func aiProviders() -> [AiProvider] { lock.withLock { settleKeyChecksLocked(); return agents.providers } }
    public func agentConfigs() -> [AgentConfig] { lock.withLock { agents.agents } }
    public func decisionConsent(provider: DecisionProviderKind) -> Bool { lock.withLock { decisionConsentKey(provider: provider).map { decisionConsentGiven.contains($0) } ?? false } }   // delta 23
    public func decisionModel() -> DecisionModel? { lock.withLock { settings.decision } }
    public func decisionPreview() -> [DecisionVerdict] { lock.withLock { settings.preview } }
    public func speechEngine() -> SpeechEngine { lock.withLock { settings.speech } }
    public func mcpGrants() -> [McpGrant] { lock.withLock { settings.mcpGrants() } }

    public func answer(conversationId: String) -> AskAnswer? { lock.withLock { agents.answerFor(conversationId) } }
    public func proposals(conversationId: String) -> [AgentProposal] { lock.withLock { agents.proposals(conversationId: conversationId) } }

    /// `fixture provider-fails <reason>`: off-device asks fail in the provider's words; nil clears it.
    public func setProviderFails(_ reason: String?) { lock.withLock { agents.providerFails = reason } }
    /// `fixture agent-allowed readMail,search`: replaces the built-in Ask agent's allowed actions. Unknown words refuse.
    @discardableResult
    public func setAgentAllowed(agentId: String = "inbox", words: String) -> Bool {
        let all: [String: AgentPermission] = ["readMail": .readMail, "search": .search, "draft": .draft, "label": .label, "sendWithConfirmation": .sendWithConfirmation]
        let parts = words.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
        let allowed = parts.compactMap { all[$0] }
        return lock.withLock {
            guard allowed.count == parts.count, let i = agents.agents.firstIndex(where: { $0.id == agentId }) else { return false }
            agents.agents[i].allowed = allowed
            return true
        }
    }

    public func thread(threadId: String) -> MailThread? { lock.withLock { threadLocked(threadId) } }

    func threadLocked(_ threadId: String) -> MailThread? {
        // delta 15: the replies the owner sent in this thread, oldest first, after the thread's own messages
        let sent = messages.filter { $0.outgoing == true && $0.thread == threadId }.sorted { (receivedAt[$0.id] ?? 0) < (receivedAt[$1.id] ?? 0) }
        // The design fixture's own open thread is served from its file, unless a loaded corpus brings rows of that very thread (the corpus
        // of J2 has a thr-01 of its own: msg-01, msg-01a, msg-01b): then the rows answer, as the engine answers from the mail it holds.
        if let t = fx.open_thread, t.id == threadId, !messages.contains(where: { $0.thread == threadId && $0.outgoing != true }) {
            return MailThread(id: t.id, accountId: t.account, subject: t.subject, participants: t.participants, messageIds: (t.messages ?? []).compactMap(\.id) + sent.map(\.id))
        }
        // A corpus thread (several rows sharing `thread`) is served from its rows (lead fix for the search round); a base
        // row (no `thread`) that a sent reply joined is its thread's first message
        var rows = messages.filter { $0.thread == threadId && $0.outgoing != true }
        if !sent.isEmpty, rows.isEmpty, let base = fxMessage(threadId), base.thread == nil { rows = [base] }
        if let first = rows.first {
            var seen: [String] = []
            for r in rows + sent where !seen.contains(r.sender) { seen.append(r.sender) }
            return MailThread(id: threadId, accountId: first.account, subject: first.subject, participants: seen, messageIds: rows.map(\.id) + sent.map(\.id))
        }
        guard let t = fx.open_thread else { return nil }
        return MailThread(id: t.id, accountId: t.account, subject: t.subject, participants: t.participants, messageIds: (t.messages ?? []).compactMap(\.id))
    }

    public func snoozedMessages() -> [Snooze] {
        lock.withLock { snoozed.filter { !deleted.contains($0.key) }.map { Snooze(messageId: $0.key, returnAtMs: $0.value) }.sorted { $0.returnAtMs != $1.returnAtMs ? $0.returnAtMs < $1.returnAtMs : $0.messageId < $1.messageId } }
    }

    public func splitTabRuleMatchCount(rule: SplitTabRule) -> UInt32 {
        lock.withLock {
            UInt32(order.filter { id in
                guard !gone(id), let m = fxMessage(id) else { return false }
                switch rule {
                case .builtin(let name): return (m.tab ?? "primary") == name
                case .sender(let a): return m.from.lowercased() == a.lowercased()
                case .domain(let d): return m.from.lowercased().hasSuffix("@" + d.lowercased())
                case .account(let a): return (account[id] ?? m.account) == a
                case .label(let l): return (assigned[id] ?? []).contains(l)
                case .decisionCategory(let c): return (m.tab ?? "primary") == c
                }
            }.count)
        }
    }

    /// The socket verb `fixture mcp-request <app> <tool>` calls this: an allowed app asks for something that needs a yes. Returns the request id.
    public func requestMcpTool(app: String, tool: String, words: String) throws -> String {
        let (id, sinks): (String, [EventSink]) = try lock.withLock {
            guard settings.mcpGrants().first(where: { $0.appName == app })?.allowed == true else { throw CoreError.Invalid(field: "app", reasonWords: "\(app) has no grant on the MCP server, so it cannot ask") }
            let id = newId("mcp-req")
            mcpRequests[id] = (app, tool, "pending")
            return (id, self.sinks.values.filter { $0.0 == .agent }.map(\.1))
        }
        Self.deliver(sinks.map { ($0, CoreEvent.mcpApprovalNeeded(requestId: id, appName: app, toolName: tool, requestWords: words)) })
        return id
    }

    public func oauthSignInState(accountDraftId: String) -> OAuthSignInState { lock.withLock { oauth.state(accountDraftId) } }

    /// `fixture oauth approve|refuse <address> [words]`: the fake authority answers the waiting sign-in (delta 14).
    public func answerOAuth(address: String, approve: Bool, words: String) throws { try lock.withLock { try oauth.answer(address, approve: approve, words: words) } }

    public func detectProvider(email: String) -> DetectionResult {
        let (result, targets): (DetectionResult, [EventSink]) = lock.withLock {
            (addAccount.detect(email), sinks.values.filter { $0.0 == .syncStatus }.map(\.1))
        }
        var number: UInt32 = 0
        Self.deliver(result.progress.flatMap { p -> [(EventSink, CoreEvent)] in
            number += 1
            return targets.map { ($0, CoreEvent.detectionProgress(address: email, number: number, progress: p)) }
        })
        return result
    }

    public func validateAccount(config: AccountConfig, password: SecretText) -> ValidationReport {
        lock.withLock { addAccount.validate(config, password: password.value) }
    }

    /// SPEC A87: `fixture refuse <cause id>`: the next sign-in check fails with that cause. False for an unknown id.
    public func armRefusal(_ causeId: String) -> Bool {
        if FixtureAddAccountBook.engineWords[causeId] != nil || causeId.hasPrefix("engine.") { lock.withLock { addAccount.armedEngineCode = causeId }; return true }
        let all: [RefusalCause] = [.basicAuthDisabled, .badPassword, .hostUnreachable, .tlsFailed, .smtpRefused, .unknown]
        guard let c = all.first(where: { refusalCauseId(cause: $0) == causeId }) else { return false }
        lock.withLock { addAccount.armedRefusal = c }
        return true
    }

    public func state(path: String) -> String? {
        if path == "fixture/digest" { return lock.withLock { stateDigestLocked().total } }   // probes-r1: one value over the whole fixture state (FixtureReset.swift)
        if path.hasPrefix("draft/current/") {      // lane richtext-r2: the draft on screen, read from the shared rich text session
            guard let v = RichTextFixture.draftValue(String(path.dropFirst("draft/current/".count))),
                  let d = try? JSONSerialization.data(withJSONObject: v, options: [.fragmentsAllowed]) else { return nil }
            return String(data: d, encoding: .utf8)
        }
        // fixture-r3: the Reminders view's rows (the contract's key is "reminders"; `mailboxNamed` lists the other boxes)
        if path == "mailbox/reminders/rows" || path == "mailbox/reminders/count" {
            return lock.withLock { settleOutboxLocked(); return String(mailboxRowsLocked(.reminders, accountIds: nil).count) }
        }
        if path.hasPrefix("people/") { return peopleState(path: path) }   // SearchSuggest.swift PeopleRead
        if let hit = stateDelta9(path) { return hit.value }
        if path.hasPrefix("smart_folder/"), path.hasSuffix("/rule"), path.count > "smart_folder//rule".count {
            // a folder name may hold dots and spaces: read the middle whole
            let name = String(path.dropFirst("smart_folder/".count).dropLast("/rule".count))
            let rule = lock.withLock { smartFoldersLocked().first { $0.name.caseInsensitiveCompare(name) == .orderedSame || $0.id == name }?.ruleWords }
            guard let rule, let d = try? JSONSerialization.data(withJSONObject: rule, options: [.fragmentsAllowed]) else { return nil }
            return String(data: d, encoding: .utf8)
        }
        if path.hasPrefix("detection_cache/"), path.hasSuffix("/source") {
            // a domain holds dots: read the middle whole
            let domain = String(path.dropFirst("detection_cache/".count).dropLast("/source".count)).lowercased()
            guard let word = lock.withLock({ addAccount.cache[domain] }), let d = try? JSONSerialization.data(withJSONObject: word, options: [.fragmentsAllowed]) else { return nil }
            return String(data: d, encoding: .utf8)
        }
        if let served = stateComposerServed(path) { return served.value }   // J3: ai_job/last/*, server/<address>/inbox/count (FixtureCoreComposer.swift)
        if let served = stateReplyToServed(path) { return served.value }   // the Reply-To paths both engines serve (FixtureReplyTo.swift)
        if let served = stateTestsServed(path) { return served.value }   // swift-tests lane: paths the Swift seam tests read (extension at the end of this file)
        let p = path.split(whereSeparator: { $0 == "/" || $0 == "." }).map(String.init)
        let value: Any? = lock.withLock { () -> Any? in
            settleOutboxLocked()
            switch (p.first, p.count) {
            case ("message", 3):
                switch p[2] {
                case "archived": return archived.contains(p[1])
                case "read": return read.contains(p[1])
                case "account": return account[p[1]]
                case "snoozed": return snoozed[p[1]] != nil
                case "deleted": return deleted.contains(p[1])
                case "starred": return starred.contains(p[1])
                case "unsubscribed": return listsLeft[p[1]] != nil || unsubscribed.contains(p[1])
                case "unsubscribe_method": return unsubscribeMethodLocked(p[1]).map { m -> String in
                    switch m { case .oneClick: return "one_click"; case .mail: return "mail"; case .link: return "link" } }
                case "muted": return muted.contains(p[1])
                case "spam": return spam.contains(p[1])
                default: return nil
                }
            case ("counts", 2):
                let c = countsLocked()
                if let t = c.tabs.first(where: { $0.tabId == p[1] }) { return Int(t.unread) }
                return c.mailboxes[p[1]].map { Int($0) }
            case ("device_link", 2) where p[1] == "id": return deviceLink.id
            case ("device_link", 2) where p[1] == "token": return deviceLink.token
            case ("device_link", 2) where p[1] == "expires_in_s": return deviceLink.expiresInS
            case ("settings_sync", 2) where p[1] == "records_carried": return deviceLink.recordsCarried
            // The design fixture stands for the iPhone after the scan (J4.10, J4.11): one paired Mac, last synced 2 minutes ago (the fixture's own
            // `states.sync.line` "synced 2 min ago"). `settings_sync/error` answers only while `fixture sync broken` is on (absent otherwise: nothing is failing to sync).
            case ("settings_sync", 2) where p[1] == "error": return syncBroken ? Self.syncBrokenReason : nil
            case ("settings_sync", 2) where p[1] == "devices": return [Self.pairedMacName]
            case ("settings_sync", 2) where p[1] == "last_sync_ms": return clock.nowMs() - Self.pairedMacSyncAgeMs
            // Accounts whose credential did not arrive with the scan: the iPhone would have to ask for their password. Same fact as `account_credential/<id>/stored`.
            case ("iphone", 2) where p[1] == "password_prompts": return accountIdsLocked().filter { !credentialCarriedLocked($0) }.count
            case ("account", 3) where p[2] == "address":
                return added.first { $0.id == p[1] }?.address ?? fx.accounts.first { $0.id == p[1] }?.address
            case ("account", 3) where p[2] == "provider":
                if let a = added.first(where: { $0.id == p[1] }) { return "\(a.provider)" }
                return fx.accounts.first { $0.id == p[1] }.map { "\(MailProvider(words: $0.provider))" }
            case ("account", 2) where p[1] == "count": return noAccounts ? 0 : fx.accounts.count + added.count
            case ("account", 3) where addAccount.configValue(p[1], p[2]) != nil: return addAccount.configValue(p[1], p[2])
            case ("detection", 3) where p[1] == "last": return addAccount.lastValue(p[2])
            case ("detection", 2) where p[1] == "count": return addAccount.detectionCount
            case ("validation", 3) where p[1] == "last": return addAccount.validationValue(p[2])
            case ("index", 2) where p[1] == "message_count": return messages.filter { !deleted.contains($0.id) }.count
            case ("snooze", 3) where p[2] == "return_ms": return snoozed[p[1]]
            case ("reply_reminder", 3) where p[2] == "at_ms": return replyReminders[p[1]]
            // fired: the time has passed and no reply arrived in its thread by then (`replyReminderState`; `fixture reply-arrived` is a reply)
            case ("reply_reminder", 3) where p[2] == "fired": return replyReminders[p[1]].map { _ in replyReminderInView(state: reminderStateLocked(p[1])) }
            case ("smart_folder", 3) where p[2] == "parent": return folderRows.first { $0.id == p[1] }?.parentId
            case ("smart_folder", 3) where p[2] == "rule": return folderRows.first { $0.id == p[1] }?.ruleWords
            // delta 14: what the folder's saved words find (J2), the list's row ids (message ids) newest first, and its count
            // (the number of those rows) and unread, all from the same rows: an archive takes a row out of all three
            case ("smart_folder", 3) where p[2] == "threads":
                guard folderRows.contains(where: { $0.id == p[1] }) else { return nil }
                return ruleLiveIdsLocked(folderRows.first { $0.id == p[1] }?.ruleWords ?? "")
            case ("smart_folder", 3) where p[2] == "count":
                guard folderRows.contains(where: { $0.id == p[1] }) else { return nil }
                return ruleLiveIdsLocked(folderRows.first { $0.id == p[1] }?.ruleWords ?? "").count
            case ("smart_folder", 3) where p[2] == "unread":
                guard folderRows.contains(where: { $0.id == p[1] }) else { return nil }
                return ruleLiveIdsLocked(folderRows.first { $0.id == p[1] }?.ruleWords ?? "").filter { !read.contains($0) }.count
            case ("draft", 2) where p[1] == "count": return draftRows.count
            case ("draft", 3):
                guard let d = draftRows[p[1]] else { return nil }
                switch p[2] { case "from_account_id": return d.account; case "to": return d.to; case "cc": return d.cc; case "subject": return d.subject; case "body": return d.body; case "reply_to_thread_id": return d.thread; default: return nil }
            case ("mcp_request", 3) where p[2] == "state": return mcpRequests[p[1]]?.state
            case ("mcp_request", 3) where p[2] == "results": return mcpResults[p[1]] ?? []   // delta 13: subjects, after approval
            case ("mcp_request", 2) where p[1] == "pending_count": return mcpRequests.values.filter { $0.state == "pending" }.count
            case ("label_removed", 3): return removedByUser.contains("\(threadOf(p[1]))|\(p[2])")   // label_removed/<message>/<label>
            case ("pin", 2): return pinned[p[1]] ?? false
            // a built-in label a mail is filed under (À répondre, by the label's flow) lives in `category` here; the real engine keeps it in its one
            // label_assignment table (label_cmds.rs:380), so its name is listed too (main104 J5.10: the kept mail was never seen held)
            // (only a built-in label its flow filed the mail under: a category from a tab move stays out, as every reader of this path before expects)
            case ("label_assignment", 2):
                let flowFiled = category[p[1]].flatMap { c in flowCard[p[1]]?[c] != nil ? [c] : nil } ?? []
                return ((assigned[p[1]] ?? []) + flowFiled).compactMap { lid in labelRows.first { $0.id == lid }?.name }
            case ("label", 3) where p[2] == "path": return labelRows.first { $0.id == p[1] }?.path
            case ("label", 3) where p[2] == "depth": return labelRows.first { $0.id == p[1] }.map { Int($0.depth) }
            case ("labels", 2) where p[1] == "count": return labelRows.count
            case ("label", 3) where p[2] == "name": return labelRows.first { $0.id == p[1] }?.name
            case ("label", 3) where p[2] == "parent": return labelRows.first { $0.id == p[1] }?.parentId
            case ("smart_folder", 2) where p[1] == "count": return smartFoldersLocked().count
            case ("contact", 2) where p[1] == "count": return contactCards.count
            case ("contacts", 2) where p[1] == "count": return contactsLocked().count
            case ("search", 2) where p[1] == "recent": return searchBook.recent
            case ("search", 3) where p[1] == "recent" && p[2] == "count": return searchBook.recent.count
            case ("search", _) where p[1] == "last": return searchBook.value(p)
            case ("contact", 3) where p[2] == "addresses": return contactCards.first { $0.id == p[1] }?.addresses
            case ("undo_entry", 2) where p[1] == "count": return undo.count
            case ("undo_entry", 2) where p[1] == "consumed_last": return consumedLast
            case ("outbox_entry", 2) where p[1] == "count": return entries.count
            // A thread's sends, oldest first, as their state words (quickreply round 4: the thread's outgoing cards).
            case ("outbox_thread", 2): return entries.filter { $0.draft.replyToThreadId == p[1] }.map { e -> String in
                switch e.state { case .undoable: return "undoable"; case .sent: return "sent"; case .cancelled: return "cancelled"
                                 case .sending: return "sending"; case .scheduled: return "scheduled"; case .failed: return "failed"
                                 case .late: return "late"; case .waitingForNetwork: return "waiting_for_network" } }
            case ("outbox_entry", 3) where p[2] == "state":
                guard let i = Int(p[1]), i < entries.count else { return nil }
                switch entries[i].state {
                case .undoable(let until): return clock.nowMs() >= until ? "sent" : "undoable"
                case .scheduled: return "scheduled"
                case .sending: return "sending"
                case .sent: return "sent"
                case .failed: return "failed"
                case .cancelled: return "cancelled"
                case .late: return "late"
                case .waitingForNetwork: return "waiting"
                }
            case ("notch_alerts", 2):
                let a = settings.alerts
                switch p[1] {
                case "vip": return notchAlertStyleWord(style: a.vip)
                case "needs_reply": return notchAlertStyleWord(style: a.needsReply)
                case "primary": return notchAlertStyleWord(style: a.primary)
                case "updates": return notchAlertStyleWord(style: a.updates)
                case "promotions": return notchAlertStyleWord(style: a.promotions)
                case "sign_in_codes": return notchAlertStyleWord(style: a.signInCodes)
                case "stay": return notchStayWord(stay: a.stay)
                case "sound": return a.sound
                case "full_mail_loads_pictures": return a.fullMailLoadsPictures
                default: return nil
                }
            case ("notch_kind", 2): return notchKindWord(kind: notchKindLocked(p[1]))
            case ("notch_setting", 2):
                let n = settings.notch
                switch p[1] {
                case "show": return n.listShows == .latest ? "latest" : "unread"
                case "enabled": return n.show
                case "buttons": return n.buttons.isEmpty ? "none" : n.buttons
                case "hidden_in_screen_share": return n.hiddenInScreenShare
                case "follow_focus": return n.followFocus
                default: return nil
                }
            case ("notification_setting", 2) where p[1] == "off":
                return accountIdsLocked().filter { !settings.notification($0).enabled }.sorted()
            case ("notification_setting", 2) where p[1] == "delivery":
                return "\(settings.commonDelivery(accountIdsLocked()))"
            case ("notification_setting", 3) where accountIdsLocked().contains(p[1]):
                let s = settings.notification(p[1])
                switch p[2] {
                case "enabled": return s.enabled
                case "delivery": return "\(s.delivery ?? .both)"
                default: return nil
                }
            case ("account_icon", 2):
                guard accountIdsLocked().contains(p[1]) else { return nil }
                return settings.iconWord(p[1], fallback: fx.accounts.first { $0.id == p[1] }?.icon_id)
            case ("account", 3) where p[2] == "colour":
                return settings.colours[p[1]] ?? fx.accounts.first { $0.id == p[1] }?.colour
            case ("account_credential", 3) where p[2] == "stored":
                return accountIdsLocked().contains(p[1]) ? true : nil
            case ("sync_state", 3):
                guard let s = syncStatesLocked().first(where: { $0.accountId == p[1] }) else { return nil }
                switch p[2] {
                case "healthy": return s.lastError == nil
                case "history_fetched": return s.historyFetched
                case "messages_indexed": return Int(s.messagesIndexed)
                case "status": return s.providerStatusWords
                case "error": return s.lastError?.reasonWords
                case "last_synced_ms": return s.lastSyncedMs
                // the first screen of an inbox is there once the account signed in; a failing account never had one
                case "first_screen_at": return s.lastError == nil ? (addedAt[p[1]] ?? bornMs) : nil
                default: return nil
                }
            case ("shortcut_preset", 2) where p[1] == "name": return presetName
            case ("note", 2) where p[1] == "count": return noteRows.count
            case ("note", 3) where p[2] == "text": return noteRows.first { $0.id == p[1] }?.text
            case ("note", 3) where p[2] == "target":
                switch noteRows.first(where: { $0.id == p[1] })?.target {
                case .thread(let t)?: return "thread:\(t)"
                case .message(let m)?: return "message:\(m)"
                case nil: return nil
                }
            case ("account", 2) where p[1] == "order": return (accountOrder ?? accountIdsLocked())
            case ("account", 3) where p[2] == "password_stored": return accountIdsLocked().contains(p[1]) ? true : nil
            case ("device_link", 2) where p[1] == "payload_keys":
                // read off the real code text (PairingToken.qrPayload), never a constant: each ':' part is named by what it holds
                return deviceLink.token.map { t -> [String] in
                    let text = PairingToken(value: t, expiresAtMs: 0).qrPayload
                    let scheme = "whoosh-pair:v1:"
                    guard text.hasPrefix(scheme) else { return ["scheme"] }
                    let parts = text.dropFirst(scheme.count).split(separator: ":", omittingEmptySubsequences: false)
                    let named = parts.map { part -> String in
                        if !part.isEmpty, part.allSatisfy({ $0.isASCII && $0.isHexDigit }) { return "token" }
                        if part.contains("@") { return "address" }
                        if part.lowercased().contains("password") { return "password" }
                        return "other"
                    }
                    return Array(Set(named)).sorted()
                }
            case ("agent_conversation", 3) where p[1] == "last":
                let conv = agents.lastConversationId.flatMap { id in agents.conversations.first { $0.id == id } }
                switch p[2] {
                case "steps": return agents.lastSteps
                case "chunks": return agents.lastChunks
                case "turns": return conv?.turns.count ?? 0
                case "id": return agents.lastConversationId
                case "quoted_message": return agents.lastQuoted
                case "awaiting_consent": return agents.awaitingConsent
                default: return nil
                }
            case ("agent_conversation", 2) where p[1] == "count": return agents.conversations.count
            case ("agent_proposal", 3) where p[1] == "last":
                guard let i = agents.stored.indices.last else { return p[2] == "state" ? "none" : nil }
                switch p[2] {
                case "state": return agents.stateWord(i)
                case "id": return agents.stored[i].proposal.id
                case "items": return agents.stored[i].ids
                default: return nil
                }
            case ("agent_proposal", 2) where p[1] == "count": return agents.stored.count
            case ("agent_consent", 2): return agents.provider(p[1]).map { $0.mailConsentAtMs != nil }
            case ("provider_request", 2) where p[1] == "count": return FixtureProviderRequests.count   // hid-ai F2 (FixtureDecide.swift)
            case ("decision_teaching", 2) where p[1] == "examples": return FixtureTeaching.examples   // hid-ai F5: DECISION-MODEL.md v2 §3
            case ("decision_teaching", 2) where p[1] == "refits": return FixtureTeaching.refits
            case ("decision_teaching", 3) where p[2] == "corrections": return FixtureTeaching.count(p[1])
            case ("decision_teaching", 3) where p[2] == "shared": return FixtureTeaching.shared(p[1])
            case ("provider_credential", 3) where p[2] == "stored":
                settleKeyChecksLocked()
                return FixtureSettingsBook.credentialIds.contains(p[1]) ? agents.credentialStored.contains(p[1]) : nil
            case ("ai_provider", 3) where p[2] == "key": return nil      // a key is never read back
            case ("ai_provider", 3) where p[2] == "connected": settleKeyChecksLocked(); return agents.provider(p[1])?.connected
            case ("ai_provider", 3) where p[2] == "state": return aiProviderStateLocked(p[1], "state")       // the real core's two-step answer
            case ("ai_provider", 3) where p[2] == "refusal": return aiProviderStateLocked(p[1], "refusal")
            case ("ai_provider", 3) where p[2] == "models": return aiProviderStateLocked(p[1], "models")
            case ("speech_engine", 3) where p[1] == "chosen" && p[2] == "state": return settings.speechState
            case ("agent_config", 3) where p[2] == "allowed":
                guard let a = agents.agents.first(where: { $0.id == p[1] || $0.name.caseInsensitiveCompare(p[1]) == .orderedSame }) else { return nil }
                return a.allowed.map { "\($0)" }
            case ("decision_model", 2) where p[1] == "provider":
                return settings.decision.map { "\($0.provider)" }
            case ("decision_model", 2) where p[1] == "key": return nil   // the Jev key lives in the Keychain only
            case ("decision_model", 2) where p[1] == "model": return settings.decision?.model
            case ("decision_preview", 2) where p[1] == "applied": return settings.previewApplied
            case ("decision_preview", 2) where p[1] == "count": return settings.preview.count
            case ("decision_preview", 2) where p[1] == "placed": return settings.previewApplied ? settings.placedBy.count : 0      // as the real engine: what Apply wrote
            case ("decision_preview", 2) where p[1] == "words": return settings.previewWords
            case ("mcp_grant", 2): return settings.mcpApps.contains(p[1]) ? settings.mcpAllowedNow(p[1]) : nil
            case ("mcp_server", 2) where p[1] == "on": return settings.mcpOn
            case ("label_assignment", 3) where p[2] == "count":
                let ids = labelRows.filter { $0.id == p[1] || $0.name.caseInsensitiveCompare(p[1]) == .orderedSame }.map(\.id)
                return assigned.values.filter { $0.contains { ids.contains($0) } }.count
            case ("split_tab", 3) where inboxViewFromId(id: p[1]) != nil:   // delta 16: a view answers like a tab of the strip, so the list's own reads keep working
                guard let v = inboxViewFromId(id: p[1]) else { return nil }
                switch p[2] {
                case "name": return inboxViewName(view: v)
                case "order": return inboxTabsLocked().firstIndex { $0.id == p[1] }
                case "enabled": return true
                case "count": return order.filter { !gone($0) && belongs($0, toTab: p[1]) }.count
                case "unread": return countsLocked().tabs.first { $0.tabId == p[1] }.map { Int($0.unread) }
                case "rule": return "view:\(p[1])"
                default: return nil
                }
            case ("split_tab", 3) where p[2] == "name": return tabNames[p[1]]
            case ("split_tab", 3) where p[2] == "order": return splitTabRows.firstIndex { $0.id == p[1] }
            case ("split_tab", 3) where p[2] == "enabled": return splitTabRows.first { $0.id == p[1] }?.enabled
            case ("split_tab", 3) where p[2] == "count":
                guard splitTabRows.contains(where: { $0.id == p[1] }) else { return nil }
                return order.filter { !gone($0) && belongs($0, toTab: p[1]) }.count
            case ("split_tab", 3) where p[2] == "unread":
                guard splitTabRows.contains(where: { $0.id == p[1] }) else { return nil }
                return countsLocked().tabs.first { $0.tabId == p[1] }.map { Int($0.unread) }
            case ("split_tab", 3) where p[2] == "rule":
                switch splitTabRows.first(where: { $0.id == p[1] })?.rule {
                case .builtin(let n)?: return "builtin:\(n)"
                case .sender(let a)?: return "sender:\(a)"
                case .domain(let d)?: return "domain:\(d)"
                case .account(let a)?: return "account:\(a)"
                case .label(let l)?: return "label:\(l)"
                case .decisionCategory(let c)?: return "category:\(c)"
                case nil: return nil
                }
            case ("split_tab", 2) where p[1] == "count": return splitTabRows.count
            case ("snooze", 3) where p[2] == "return_local_time":
                guard let ms = snoozed[p[1]] else { return nil }
                let f = DateFormatter(); f.dateFormat = "HH:mm"
                return f.string(from: Date(timeIntervalSince1970: Double(ms) / 1000))
            default: return nil
            }
        }
        guard let value else { return nil }
        guard let data = try? JSONSerialization.data(withJSONObject: value, options: [.fragmentsAllowed]) else { return nil }
        return String(data: data, encoding: .utf8)
    }

    public func subscribe(stream: StreamKind, sink: EventSink) -> UInt64 {
        let id: UInt64 = lock.withLock { let id = nextSub; nextSub += 1; sinks[id] = (stream, sink); return id }
        let first: [CoreEvent]
        switch stream {
        case .inbox: first = [.inboxChanged(tabId: "primary", rows: inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)))]
        case .counts: first = [.countsChanged(counts: counts())]
        case .syncStatus: first = syncStates().map { CoreEvent.syncStateChanged(state: $0) }
        case .outbox: first = outbox().map { CoreEvent.outboxTick(entry: $0) }
        case .thread, .search, .agent: first = []
        }
        Self.deliver(first.map { (sink, $0) })
        return id
    }

    /// Events reach sinks in commit order: on the main thread, synchronously when the caller is on it,
    /// else through the main queue (serial, first in first out).
    static func deliver(_ work: [(EventSink, CoreEvent)]) {
        guard !work.isEmpty else { return }
        let run = { for (sink, event) in work { sink.onEvent(event: event) } }
        if Thread.isMainThread { run() } else { DispatchQueue.main.async(execute: run) }
    }

    public func outbox() -> [OutboxEntry] {
        lock.withLock {
            settleOutboxLocked()
            return entries.filter {
                switch $0.state {
                case .undoable(let until): return clock.nowMs() < until
                case .sent, .cancelled: return false
                default: return true
                }
            }
        }
    }

    public func syncStates() -> [SyncState] {
        lock.withLock { syncStatesLocked() }
    }

    func syncStatesLocked() -> [SyncState] {
        let base = fx.accounts.enumerated().map { i, a -> SyncState in
            let failing = a.id == failingAccountId
            // `fixture indexing <n>` (contracts delta 12): past the first n accounts, the first index is still being built
            if !failing, let n = indexedAccounts, i >= n {
                return SyncState(accountId: a.id, historyFetched: false, messagesIndexed: 0, lastError: nil, providerStatusWords: "indexing your mail", lastSyncedMs: nil)
            }
            return SyncState(accountId: a.id, historyFetched: !failing, messagesIndexed: 0,
                             lastError: failing ? ActionFailed(accountName: a.name, reasonWords: "needs you to sign in again", retryable: true) : nil,
                             providerStatusWords: failing ? "needs you to sign in again" : "synced", lastSyncedMs: failing ? nil : clock.nowMs())
        }
        // an account the owner added: signed in, the first screen still loading (derived, never stored)
        return base + added.map { a in
            SyncState(accountId: a.id, historyFetched: false, messagesIndexed: 0, lastError: nil, providerStatusWords: "fetching the inbox", lastSyncedMs: nil)
        }
    }

    /// `fixture indexing <n>|all` (contracts delta 12): the first `n` of the fixture's accounts, in their listed order, are
    /// indexed and the others are still loading (history not fetched yet, status "indexing your mail"); nil (`all`) puts every
    /// account back to indexed. Read through `syncStates()` and the sync-status stream, which gets one event per account.
    /// A failing account stays failing whatever n is. n below 0 or above the account count is refused in words.
    public func setIndexing(accounts n: Int?) throws {
        let states: [SyncState] = try lock.withLock {
            if let n, n < 0 || n > fx.accounts.count {
                throw CoreError.Invalid(field: "indexing", reasonWords: "pick a number of accounts from 0 to \(fx.accounts.count), or all")
            }
            indexedAccounts = n
            return syncStatesLocked()
        }
        let targets = lock.withLock { sinks.values.filter { $0.0 == .syncStatus }.map(\.1) }
        Self.deliver(targets.flatMap { sink in states.map { (sink, CoreEvent.syncStateChanged(state: $0)) } })
    }

    /// `fixture reauth <accountId>`: the failing account signs in again and becomes healthy.
    public func reauth(accountId: String) -> Bool {
        let changed: SyncState? = lock.withLock {
            guard failingAccountId == accountId else { return nil }
            failingAccountId = nil
            return syncStatesLocked().first { $0.accountId == accountId }
        }
        guard let s = changed else { return false }
        let targets = lock.withLock { sinks.values.filter { $0.0 == .syncStatus }.map(\.1) }
        Self.deliver(targets.map { ($0, CoreEvent.syncStateChanged(state: s)) })
        return true
    }

    /// `fixture device-link scan`: the paired device's first request reaches this copy (DeviceLinkModel.scan: it redeems the token, deletes it from the keychain and
    /// marks it consumed, so `device_link/token` reads nil). On a test copy a phone scan happens in ANOTHER process, so the rig cannot reach it any other way.
    func simulateDeviceLinkScan() {
        lock.withLock { _ = deviceLink.scan() }
    }

    public func unsubscribe(subscription: UInt64) { lock.withLock { sinks[subscription] = nil } }

    func emit() {
        let (targets, extra): ([(StreamKind, EventSink)], [(StreamKind, CoreEvent)]) = lock.withLock {
            defer { pending.removeAll() }
            return (Array(sinks.values), pending)
        }
        let counts: Counts = counts()
        let rows: [InboxRow] = inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil))
        var work: [(EventSink, CoreEvent)] = []
        for (kind, sink) in targets {
            switch kind {
            case .inbox: work.append((sink, CoreEvent.inboxChanged(tabId: "primary", rows: rows)))
            case .counts: work.append((sink, CoreEvent.countsChanged(counts: counts)))
            default: break
            }
            for (k, e) in extra where k == kind { work.append((sink, e)) }
        }
        Self.deliver(work)
    }
}

/// Time for the fixture core; the socket verb `clock advance <seconds>` moves it.
public final class FixtureClock: Clock, @unchecked Sendable {
    let lock = NSLock()
    var offsetMs: Int64 = 0
    /// Set by the fixture core: called after every move, so an undo window that ended sends its message.
    var onAdvance: (() -> Void)?
    public init() {}
    public func nowMs() -> Int64 { lock.withLock { Int64(Date().timeIntervalSince1970 * 1000) + offsetMs } }
    /// `fixture reset`: the launch clock again (no `clock advance` kept).
    func restore() { lock.withLock { offsetMs = 0 } }
    public func utcOffsetS() -> Int32 { Int32(TimeZone.current.secondsFromGMT()) }
    /// The largest move one `clock advance` takes: ten years. Beyond it, or not finite, is refused.
    public static let maxAdvanceS: Double = 10 * 365 * 24 * 3600
    @discardableResult
    public func advance(seconds: Double) -> Bool {
        guard seconds.isFinite, abs(seconds) <= Self.maxAdvanceS else { return false }
        let moved: Bool = lock.withLock {
            let (sum, overflow) = offsetMs.addingReportingOverflow(Int64(seconds * 1000))
            if overflow { return false }
            offsetMs = sum
            return true
        }
        if moved { onAdvance?() }
        return moved
    }
}

// MARK: fixture-r3, `fixture reply-arrived`

extension FixtureCore {
    /// `fixture reply-arrived <message, draft or outbox id>`: the person answers. A new unread message lands in that thread
    /// (an inbox row, counted by the tab and the thread), and the thread's first reply time is recorded, so the reminder reads
    /// answered once the clock passes its time (`reminderStateLocked`). Returns the new message id and the reminder's state word.
    @discardableResult
    public func fixtureReplyLands(id: String) throws -> (messageId: String, state: String) {
        let out: (String, String) = try lock.withLock {
            let key = replyReminders[id] != nil ? reminderThreadLocked(id) : threadKeyLocked(id)
            let inThread = order.compactMap { fxMessage($0) }.filter { threadKeyLocked($0.id) == key }
            guard let first = inThread.first(where: { $0.outgoing != true }) ?? inThread.first ?? fxMessage(key) else {
                throw CoreError.NotFound(kind: "thread", id: id)
            }
            let subject = first.subject.lowercased().hasPrefix("re:") ? first.subject : "Re: \(first.subject)"
            let body = "Thanks for your note. I will come back to you."
            let rid = newId("reply")
            let acc = account[first.id] ?? first.account
            let row = FixtureFile.Message(id: rid, sender: first.sender, from: first.from, account: acc, subject: subject, preview: String(body.prefix(160)),
                                          time: "", unread: true, pinned: false, attachments: nil, thread_count: 1, trackers_blocked: 0, tab: first.tab ?? "primary",
                                          body: [body], quoted: nil, date: nil, thread: key)
            messages.insert(row, at: 0)
            order.insert(rid, at: 0)
            account[rid] = acc
            receivedAt[rid] = clock.nowMs()
            pinned[rid] = false
            inserted.insert(rid)
            if replyArrivedAt[key] == nil { replyArrivedAt[key] = clock.nowMs() }
            if let t = threadLocked(key) { pending.append((.thread, .threadChanged(thread: t))) }
            let state = replyReminderStateKey(state: reminderIdLocked(thread: key).map { reminderStateLocked($0) } ?? .noReminder)
            return (rid, state)
        }
        emit()
        return out
    }
}

// MARK: state paths the Swift seam tests read (lane swift-tests, 2026-11-11)
// Each path below answers from state the fixture already holds, never from a constant written to match a test.
extension FixtureCore {
    struct TestsServed { let value: String? }

    /// nil = not one of these paths. `account_icon/limit`: the upload limit of the ledger (docs/CONSTANTS.md ACCOUNT_ICON_MAX_BYTES, the value
    /// FixtureSettings.setIcon enforces). `search_history/recent`: the recent searches the search book holds, newest first, as a JSON list
    /// (the same list as `search/recent`; Tests/Integration/SearchSeamTests.swift reads it under this name). `voice_profile/built_at_ms`:
    /// when the voice was last analysed, read from the replies book (the J3.3 draft reads that profile). `decision_model/provider`: "rules" while no decision model is saved. `smart_folder/<name or id>/exists`: whether a saved smart folder
    /// of that name (case-blind) or id is in the list (`smart_folder/count` counts the same list).
    func stateTestsServed(_ path: String) -> TestsServed? {
        func json(_ v: Any) -> String? { (try? JSONSerialization.data(withJSONObject: v, options: [.fragmentsAllowed])).flatMap { String(data: $0, encoding: .utf8) } }
        if path.hasPrefix("smart_folder/"), path.hasSuffix("/exists"), path.count > "smart_folder//exists".count {   // the name may hold dots and spaces: read the middle whole
            let name = String(path.dropFirst("smart_folder/".count).dropLast("/exists".count))
            return TestsServed(value: lock.withLock { json(smartFoldersLocked().contains { $0.name.caseInsensitiveCompare(name) == .orderedSame || $0.id == name }) })
        }
        switch path {
        case "decision_model/provider":   // none saved yet: the rules decide (FixtureSettings.swift `decision?.provider ?? .rules`); a saved model answers through the existing case
            return lock.withLock { settings.decision == nil ? TestsServed(value: json("rules")) : nil }
        case "account_icon/limit": return TestsServed(value: json(WhooshConstants.ACCOUNT_ICON_MAX_BYTES))
        case "search_history/recent": return TestsServed(value: lock.withLock { json(searchBook.recent) })
        case "voice_profile/built_at_ms": return TestsServed(value: lock.withLock { replyBookLocked().voice.analysedAtMs.flatMap { json($0) } })
        default: return nil
        }
    }
}

extension FixtureCore {
    /// `fixture search-hold on|off` (a test copy only): while on, a search is answered in two steps and the second waits; `off` sends it (the ranked list).
    public func fixtureSearchHold(_ on: Bool) {
        let release: (String, [SearchHit])? = lock.withLock {
            searchBook.hold = on
            guard !on, let h = searchBook.held else { return nil }
            searchBook.held = nil; searchBook.stageOverride = nil
            return (h.query, h.hits)
        }
        guard let (q, hits) = release else { return }
        let targets = lock.withLock { sinks.values.filter { $0.0 == .search }.map(\.1) }
        Self.deliver(targets.map { ($0, CoreEvent.searchResults(query: q, hits: hits)) })
    }

    /// `fixture search-refuse <words>|off`: every search is refused with these words until off.
    public func fixtureSyncBroken(_ on: Bool) { lock.withLock { syncBroken = on } }
    public func fixtureSearchRefuse(_ words: String?) { lock.withLock { searchBook.refusal = words } }
}
