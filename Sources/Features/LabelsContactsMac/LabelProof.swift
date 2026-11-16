import Foundation

/// How a message moves when the rules change: it joins the label, leaves it, or stays. Pane 2 draws it as a 2 px bar at the row's left edge.
enum ProofEdge: String, Sendable { case join, leave, stay }

/// One message in pane 2 while the inspector is open: who, what, and the small grey tag naming the rule kinds that caught it.
struct ProofRow: Identifiable, Equatable, Sendable {
    var id: String
    var senderName: String
    var subject: String
    var accountId: String
    var receivedAtMs: Int64
    var unread: Bool
    var threadCount: UInt32
    /// "Domain · Subject"; on a leaving row "was Account"
    var tag: String
    /// the tag names the rule a leaving row WAS caught by
    var wasTag = false
    /// the word the first subject rule matched, drawn with a faint blue highlight
    var highlight: String?
    var edge: ProofEdge?
}

/// The live proof of a draft against the mail the core holds: the count (the core's own match-count read), a count per rule,
/// the matching rows, and, for a label that exists, who would join, leave or stay on a re-run. Read-only; nothing here writes.
struct LabelProof: Equatable, Sendable {
    var count = 0
    var unread = 0
    var newestMs: Int64?
    /// one count per rule row, in order; the numbers beside the rules
    var perRule: [Int] = []
    var rows: [ProofRow] = []
    var join: [ProofRow] = []
    var leave: [ProofRow] = []
    var stay: [ProofRow] = []
    var unaffected = 0
    /// a preview exists only when the rules or meaning of a saved label moved
    var hasPreview = false
    /// the count before the keystroke that started the current read ("was 4")
    var was: Int?
    /// what the engine says a re-run of the SAVED label does (state `label/<l>/rerun/preview`); nil for an unsaved draft or an older core
    var engine: EnginePreview?

    /// The figures the line and the offer show: the engine's when it has spoken, else the rows the rules would catch.
    var joinCount: Int { joinByRule + (joinByAi ?? 0) }
    /// Who joins through an entry rule: the engine's `by_rule`, else the rows the rules catch (the same rows the list draws as "join").
    var joinByRule: Int { engine?.byRule ?? join.count }
    /// Who joins through the AI: only the engine knows (after Save); an unsaved draft's preview is the rules alone, so nil = not known, never 0.
    var joinByAi: Int? { engine?.byAi }
    var leaveCount: Int { engine?.leave ?? leave.count }
    var stayCount: Int { engine?.stay ?? stay.count }

    static let empty = LabelProof()
    var isEmpty: Bool { count == 0 && rows.isEmpty }
}

/// `label/<l>/rerun/preview`: what a re-run of the saved label does, in the foot's terms, from the one computation the offer and the run also use.
/// `aiWords` is the sentence when the AI card is on and no model can answer ("The AI cannot sort yet: no model on this Mac."); `byAi` is then 0.
struct EnginePreview: Equatable, Sendable {
    var byRule = 0, byAi = 0, leave = 0, stay = 0, beyond = 0
    var aiWords: String?

    init?(_ json: String?) {
        guard let o = StoredRules.object(json) as? [String: Any], o["by_rule"] != nil else { return nil }
        func n(_ k: String) -> Int { (o[k] as? NSNumber)?.intValue ?? 0 }
        byRule = n("by_rule"); byAi = n("by_ai"); leave = n("leave"); stay = n("stay"); beyond = n("beyond")
        if let w = o["ai_words"] as? String, !w.isEmpty { aiWords = w }
    }
}

/// One message as both the rule engine and the list know it.
struct ProofCorpusItem: Sendable {
    var facts: MessageFacts
    var row: InboxRow
}

/// The thread facts of one message, from the messages of its thread: what the core's SQL reads (`Store::OWNER_WROTE_EARLIER_SQL`,
/// `Store::LATER_IN_THREAD_SQL`), written once here for the proof's corpus.
enum ProofThreadFacts {
    struct Mate: Sendable, Equatable { var id: String; var at: Int64; var mine: Bool }

    /// Sans réponse: nothing newer in the thread (inbound message). Déjà répondu par moi: I wrote EARLIER in the thread (owner ruling 2026-11-10).
    static func of(id: String, at: Int64, thread: [Mate]) -> (unanswered: Bool, repliedByMe: Bool) {
        let unanswered = !thread.contains { $0.id != id && ($0.at > at || ($0.at == at && $0.id > id)) }
        let repliedByMe = thread.contains { $0.mine && $0.id != id && ($0.at < at || ($0.at == at && $0.id < id)) }
        return (unanswered, repliedByMe)
    }
}

enum LabelProofCompute {
    /// Reads the corpus once: every row the inbox query serves, each with the message body and recipients the rules can test, and the
    /// facts of the grid a row and its thread give: Cc, list header, Sans réponse and Déjà répondu par moi (from the thread's own messages;
    /// the core says which are mine, `Message.isOutgoing`). Favori, Expéditeur VIP and Je suis en copie need the account addresses, the VIP
    /// list and the star, which a row does not carry: the core says which messages have them (`coreIds`), so the counts and the rows' membership come from
    /// the core and these facts, equal to the core's, name the rule kinds and facts on a row's tag and feed the re-run plan.
    @MainActor
    static func corpus(_ app: AppModel) -> [ProofCorpusItem] {
        let core = app.core
        let rows = LabelsContactsSource.shared.allRows(core, tabs: app.tabIds)
        var threads: [String: [ProofThreadFacts.Mate]] = [:]
        func mates(of r: InboxRow) -> [ProofThreadFacts.Mate] {
            guard r.threadCount > 1 else { return [] }
            if let known = threads[r.threadId] { return known }
            let found: [ProofThreadFacts.Mate] = (core.thread(threadId: r.threadId)?.messageIds ?? []).compactMap { id in
                guard let m = core.message(messageId: id) else { return nil }
                return ProofThreadFacts.Mate(id: id, at: m.receivedAtMs, mine: m.isOutgoing)
            }
            threads[r.threadId] = found
            return found
        }
        // Favori, Expéditeur VIP and Je suis en copie are not on a row: the core answers them (one read each, the ids of the messages where the fact
        // holds), so a rule on them is judged here, for a row's tag and for the re-run plan, on what the core knows.
        let starred = coreIds(core, .starred), vip = coreIds(core, .vipSender), copied = coreIds(core, .ccMe)
        return rows.map { r in
            let m = core.message(messageId: r.messageId)
            let body = core.body(messageId: r.messageId)?.plainText ?? ""
            let to = m?.to ?? [], cc = m?.cc ?? []
            let thread = ProofThreadFacts.of(id: r.messageId, at: r.receivedAtMs, thread: mates(of: r))
            let facts = MessageFacts(messageId: r.messageId, accountId: r.accountId, fromName: r.senderName, fromAddress: r.senderAddress,
                                     to: to, cc: cc, subject: r.subject, body: body.isEmpty ? r.preview : body,
                                     hasAttachment: r.attachmentCount > 0, listHeader: (m?.listUnsubscribe.isEmpty ?? true) ? "" : "list",
                                     unread: r.unread, receivedAtMs: r.receivedAtMs,
                                     unanswered: thread.unanswered, starred: starred.contains(r.messageId), vipSender: vip.contains(r.messageId), ccMe: copied.contains(r.messageId),
                                     repliedByMe: thread.repliedByMe)
            return ProofCorpusItem(facts: facts, row: r)
        }
    }

    /// The ids of the inbound messages where one fact holds, from the core (`ruleSetMatches` on that fact alone).
    static func coreIds(_ core: WhooshCore, _ fact: MessageFact) -> Set<String> {
        guard let c = factCondition(fact: fact, state: .yes) else { return [] }
        return Set(core.ruleSetMatches(ruleSet: RuleSet(conditions: [c], matchMode: .all), limit: UInt32.max))
    }

    /// The names of the facts of a chain's grid that hold for this message (« Is starred », « VIP sender »...): the tag a row caught by a fact alone carries.
    static func factWords(_ facts: [FactState], of item: ProofCorpusItem) -> [String] {
        zip(ChainDraft.order, facts).compactMap { fact, state in
            guard state != .any, let c = factCondition(fact: fact, state: state),
                  ruleSetMatches(rules: RuleSet(conditions: [c], matchMode: .all), facts: item.facts) else { return nil }
            return FlowFacts.title(fact)
        }
    }

    /// The ids of the corpus messages `set` matches: the core's own read (`ruleSetMatches`, `smart_inbox::corpus::rule_set_matching_ids`),
    /// which knows all eight facts of the grid (starred, VIP, in copy, answered, waiting, list, attachment, unread) from the store. ONE
    /// function, so the proof never judges a fact it does not have. The core lists the inbound INBOX messages; the proof keeps those the
    /// pane shows (`corpus`).
    static func matchingIds(core: WhooshCore, set: RuleSet, corpus: [ProofCorpusItem]) -> Set<String> {
        guard !set.conditions.isEmpty else { return [] }
        return Set(core.ruleSetMatches(ruleSet: set, limit: UInt32.max))
    }

    /// The proof of `draft`. `saved` (a label that exists) adds the join, leave and stay groups, `filed` the ids carrying it now.
    static func compute(core: WhooshCore, corpus: [ProofCorpusItem], draft: LabelDraft, saved: LabelDraft?, filed: Set<String>, written: String? = nil) -> LabelProof {
        var p = LabelProof()
        let set = draft.ruleSet
        let read = core.ruleSetCount(ruleSet: set)
        p.count = Int(read.count); p.unread = Int(read.unread); p.newestMs = read.newestAtMs
        let rules = draft.rules
        // the numbers beside the rules are the core's own count of each rule (it knows every fact), not a Swift re-judgement
        p.perRule = rules.map { r in r.isEmpty ? 0 : Int(core.ruleSetCount(ruleSet: r.ruleSet).count) }
        let byId = Dictionary(corpus.map { ($0.facts.messageId, $0) }, uniquingKeysWith: { a, _ in a })
        func row(_ item: ProofCorpusItem, edge: ProofEdge? = nil, against: [RuleDraft]? = nil, againstFacts: [FactState]? = nil, was: Bool = false) -> ProofRow {
            let hit = (against ?? rules).filter { !$0.isEmpty && ruleSetMatches(rules: $0.ruleSet, facts: item.facts) }
            var kinds: [String] = []
            for r in hit where !kinds.contains(r.kind.tag) { kinds.append(r.kind.tag) }
            kinds += factWords(againstFacts ?? draft.entry.facts, of: item).filter { !kinds.contains($0) }
            let r = item.row
            return ProofRow(id: r.messageId, senderName: r.senderName, subject: r.subject, accountId: r.accountId, receivedAtMs: r.receivedAtMs,
                            unread: r.unread, threadCount: r.threadCount,
                            tag: was ? Words.text("was %@", kinds.joined(separator: " · ")) : kinds.joined(separator: " · "),
                            wasTag: was, highlight: highlight(in: r.subject, rules: hit), edge: edge)
        }
        if !set.conditions.isEmpty {
            let hit = matchingIds(core: core, set: set, corpus: corpus)
            p.rows = corpus.filter { hit.contains($0.facts.messageId) }.map { row($0) }
        }
        if let saved, draft.filingMoved(from: saved) {
            let plan = planRerun(rules: set, hasDescription: !draft.meaning.trimmingCharacters(in: .whitespaces).isEmpty,
                                 corpus: corpus.map(\.facts), filed: Array(filed))
            p.hasPreview = true
            p.join = plan.file.compactMap { byId[$0] }.map { row($0, edge: .join) }
            p.leave = plan.unfile.compactMap { byId[$0] }.map { row($0, edge: .leave, against: saved.rules, againstFacts: saved.entry.facts, was: true) }
            p.stay = plan.keep.compactMap { byId[$0] }.map { row($0, edge: .stay) }
            p.unaffected = max(0, corpus.count - p.join.count - p.leave.count - p.stay.count)
            // The rules are in the core (Save was pressed): its one computation is the truth. Unsaved edits it cannot know: the rows above
            // are then only what the RULES would catch (never the AI's part), and the note under them says so.
            if let id = written, let e = EnginePreview(core.state(path: "label/\(id)/rerun/preview")) {
                p.engine = e
                p.join = Array(p.join.prefix(e.byRule)); p.leave = Array(p.leave.prefix(e.leave)); p.stay = Array(p.stay.prefix(e.stay))
                p.unaffected = max(0, corpus.count - e.byRule - e.byAi - e.leave - e.stay)
            }
        }
        return p
    }

    /// The first word of a subject-contains rule that the subject carries (case-insensitive), as it is spelt in the subject.
    static func highlight(in subject: String, rules: [RuleDraft]) -> String? {
        for r in rules where r.kind == .subject {
            for w in r.allValues {
                if let range = subject.range(of: w, options: [.caseInsensitive, .diacriticInsensitive]) { return String(subject[range]) }
            }
        }
        return nil
    }
}
