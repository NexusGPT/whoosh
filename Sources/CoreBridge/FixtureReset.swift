import Foundation

/// `fixture reset` for the core (probes-r1): the fixture core goes back to exactly its state at launch, and
/// `fixture/digest` reads one value over all of it, so a check can prove that. Every stored property of `FixtureCore`
/// is either reset in `restoreSeedLocked` or named in `FixtureDigest.skipped` with its reason; the digest reads the
/// properties by reflection, so a property added later and not reset shows as a changed part (check
/// `fixture-reset-restores-the-launch-state`, checks/probes-check.py).
extension FixtureCore {
    /// Everything the owner or a check can have changed goes back to the seed: mail (archived, deleted, junk, starred,
    /// muted, snoozed, pinned, read, moved), labels and their flows, suggestions, blocks, notes, split tabs, drafts,
    /// outbox, undo stack, reply drafts and reminders, AI settings (agents, providers, keys), notch and notification
    /// settings, account edits (added, removed, renamed, paused, reordered, icons, colours), the corpus (the served list
    /// returns to the fixture file's mail), the sign-in drafts, the import, recent searches, MCP requests and the clock.
    public func restoreSeed() {
        clock.restore()
        lock.withLock { restoreSeedLocked() }
        emit()
        let states = syncStates()
        let sync: [(StreamKind, EventSink)] = lock.withLock { sinks.values.filter { $0.0 == .syncStatus } }
        Self.deliver(sync.flatMap { _, sink in states.map { (sink, CoreEvent.syncStateChanged(state: $0)) } })
    }

    func restoreSeedLocked() {
        failingAccountId = "acc-18"
        newInstall = false
        attachmentTransfers = [:]; landedAttachments = []; decisionConsentGiven = [];   // delta 23 (added by other lanes since round 3)
        designedForRig = []                                                             // the notch's full-mail check: a mail marked designed is written again after a reset
        messages = fx.messages
        messageById = Self.index(messages)
        folderRows = []; listsLeft = [:]; listsLeftAt = [:]; filingRefused = [:]   // triage-keys: the unsubscribe date and the refused filings go back to launch
        replyReminders = [:]; replyReminderThread = [:]; replyArrivedAt = [:]
        draftRows = [:]; draftBcc = [:]; sentFromDraft = [:]; draftSavedAt = [:]; draftFileRows = [:]
        indexedAccounts = nil
        mcpRequests = [:]; mcpQueries = [:]; mcpResults = [:]
        contactCards = []; tabNames = [:]
        added = []; addedAt = [:]; accountOrder = nil; accountNames = [:]; pausedAccounts = []; removedAccounts = []
        aiSetAside = [:]; noAccounts = false; accountSignatures = [:]; lastRemoval = nil; removedByUser = []
        presetName = "Superhuman"
        noteRows = []; splitTabRows = []
        undo = []; consumedLast = false; entries = []; pending = []
        nextId = 1
        labelRows = []; labelRuleSets = [:]; entryChainIds = [:]; exitChains = [:]; flowCard = [:]; flowRejected = [:]; flowEdited = []
        replyBookStore = nil; filedBy = [:]; category = [:]; rerunState = [:]
        vipSenders = [:]   // vip-mark: a sender marked in one check is not a VIP in the next (probes-check reset-covers-every-stored-property, 16 Nov)
        suggestionRows = []; snoozesOfThread = [:]; labelsOfSender = [:]; blockRows = []
        unsubscribed = []; pinnedFromCorpus = nil; defaultPins = []
        providerEndpoints = [:]
        agents = FixtureAgentBook(); settings = FixtureSettingsBook(); oauth = FixtureOAuthBook()
        importBook = FixtureImportBook(); addAccount = FixtureAddAccountBook(); searchBook = FixtureSearchBook()
        ruleMatchMemo = [:]
        syncBroken = false
        outboxHold = OutboxHold()   // `fixture offline` and `fixture server-refuse` end with the reset
        aiJobNote = nil
        attachmentBook = nil
        // the same seeding as `init`, in the same order
        resetMessages()
        for t in fx.tabs { tabNames[t.id] = t.name }
        buildLabels(fx.labels ?? [], parent: nil)
        installBuiltInLabels()
        seedPlacementsLocked()
        seedFlowMessagesLocked()
        for a in fx.accounts { if let sig = a.signature, !sig.isEmpty { accountSignatures[a.id] = sig } }
        folderRows = Self.folders(fx.smart_folders ?? [])
    }

    /// One value over the whole fixture state: `total`, and one hash per stored property (`parts`), so a mismatch names
    /// the class of state that did not come back. Called with the lock held.
    func stateDigestLocked() -> (total: String, parts: [String: String]) {
        var parts: [String: String] = [:]
        for child in Mirror(reflecting: self).children {
            guard let label = child.label, !FixtureDigest.skipped.contains(label), !label.contains("lazy_storage") else { continue }
            if label == "messages" { parts[label] = FixtureDigest.hash("\(messages.count):" + messages.map(\.id).joined(separator: ",")); continue }
            var seen = Set<ObjectIdentifier>()
            parts[label] = FixtureDigest.hash(FixtureDigest.canon(child.value, &seen))
        }
        let total = FixtureDigest.hash(parts.keys.sorted().map { "\($0)=\(parts[$0] ?? "")" }.joined(separator: ";"))
        return (total, parts)
    }

    public func stateDigest() -> (total: String, parts: [String: String]) { lock.withLock { stateDigestLocked() } }
}

enum FixtureDigest {
    /// Stored properties the digest leaves out, and why.
    static let skipped: Set<String> = [
        "lock", "clock", "fx", "sourceURL", "bornMs",   // identity of the core, not state
        "messageById", "receivedAt",                    // derived from `messages` and the clock
        "sinks", "nextSub",                             // the app's subscriptions, kept across a reset
        "deviceLink",                                   // the Move-accounts QR link: a Keychain-backed token, not mail state (not reset)
        "ruleMatchMemo",                                // a cache of rule matches derived from the mail set: a read fills it, so it is not state
    ]

    /// FNV-1a over the UTF-8 bytes: the same text gives the same 16 hex digits in every process.
    static func hash(_ text: String) -> String {
        var h: UInt64 = 0xcbf29ce484222325
        for b in text.utf8 { h = (h ^ UInt64(b)) &* 0x100000001b3 }
        return String(format: "%016llx", h)
    }

    /// A canonical text of a value: dictionaries and sets sorted, so two equal states give equal text.
    static func canon(_ v: Any, _ seen: inout Set<ObjectIdentifier>) -> String {
        let m = Mirror(reflecting: v)
        switch m.displayStyle {
        case .optional:
            guard let c = m.children.first else { return "nil" }
            return "some(" + canon(c.value, &seen) + ")"
        case .collection:
            return "[" + m.children.map { canon($0.value, &seen) }.joined(separator: ",") + "]"
        case .set:
            return "set[" + m.children.map { canon($0.value, &seen) }.sorted().joined(separator: ",") + "]"
        case .dictionary:
            let items = m.children.map { c -> String in
                let kv = Array(Mirror(reflecting: c.value).children)
                guard kv.count == 2 else { return canon(c.value, &seen) }
                return canon(kv[0].value, &seen) + "=>" + canon(kv[1].value, &seen)
            }.sorted()
            return "{" + items.joined(separator: ",") + "}"
        case .class:
            if let o = v as? AnyObject, !seen.insert(ObjectIdentifier(o)).inserted { return "<seen>" }
            return "class(" + fields(m, &seen) + ")"
        case .struct, .tuple:
            return "(" + fields(m, &seen) + ")"
        case .enum:
            let name = String(describing: v).prefix { $0 != "(" }
            return name + "(" + fields(m, &seen) + ")"
        default:
            return "\(v)"
        }
    }

    private static func fields(_ m: Mirror, _ seen: inout Set<ObjectIdentifier>) -> String {
        m.children.map { ($0.label ?? "_") + ":" + canon($0.value, &seen) }.joined(separator: ",")
    }
}
