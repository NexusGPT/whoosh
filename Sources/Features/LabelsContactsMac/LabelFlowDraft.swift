import Foundation

/// A chain as the core stores it, before it is a draft: its number, how its conditions combine, the conditions.
struct DecodedChain: Equatable, Sendable {
    var id: UInt32
    var mode: MatchMode
    var conditions: [RuleCondition]
}

/// One chain of a label's Entry or Exit card: the eight facts as a three-state grid (`facts`, grid order), the text rows under the
/// grid, and how they combine (`matchMode`: all of them, or any one). The facts are the contract's cells (`factCondition`); a fact
/// left on "any" has no condition. `chainId` is the core's number, nil until the chain is written.
struct ChainDraft: Identifiable, Equatable, Sendable {
    /// the facts in the order the board draws them (contract: `message_facts_in_grid_order`)
    static let order: [MessageFact] = messageFactsInGridOrder()

    var id = UUID()
    var chainId: UInt32?
    var facts: [FactState] = Array(repeating: .any, count: ChainDraft.order.count)
    var rows: [RuleDraft] = [RuleDraft(kind: .from), RuleDraft(kind: .subject)]
    var matchMode: MatchMode = .all

    init() {}
    init(rows: [RuleDraft]) { self.rows = rows }

    /// The chain the core holds: the grid from `factStates`, the rows from `rowConditions` (a kind's values share one row).
    init(_ d: DecodedChain) {
        chainId = d.id; matchMode = d.mode
        facts = factStates(conditions: d.conditions).map(\.state)
        var out: [RuleDraft] = []
        for c in rowConditions(conditions: d.conditions) {
            guard let k = RuleKind(c) else { continue }
            let neg = RuleKind.isNegated(c)
            if let i = out.firstIndex(where: { $0.kind == k && $0.negated == neg }) { out[i].values.append(RuleKind.value(of: c)) }
            else { var r = RuleDraft(kind: k, values: [RuleKind.value(of: c)]); r.negated = neg; out.append(r) }
        }
        rows = out
    }

    var conditions: [RuleCondition] {
        zip(Self.order, facts).compactMap { factCondition(fact: $0, state: $1) } + rows.flatMap(\.conditions)
    }
    var ruleSet: RuleSet { RuleSet(conditions: conditions, matchMode: matchMode) }
    var isEmpty: Bool { conditions.isEmpty }
    var setFacts: Int { facts.filter { $0 != .any }.count }

    /// What counts as the same chain when the draft is compared with the saved one (the view ids are not part of it).
    var signature: [String] {
        facts.map { "\($0)" } + ["mode:\(matchMode)"] + rows.filter { !$0.isEmpty }.map { $0.signature.joined(separator: "\u{1}") }
    }

    /// Changed cells, changed or new or removed rows, a changed mode (when the chain says anything).
    func changes(from saved: ChainDraft) -> Int {
        var n = zip(facts, saved.facts).filter { $0 != $1 }.count
        let mine = rows.filter { !$0.isEmpty }.map(\.signature), was = saved.rows.filter { !$0.isEmpty }.map(\.signature)
        n += mine.filter { !was.contains($0) }.count + was.filter { !mine.contains($0) }.count
        if matchMode != saved.matchMode && !(isEmpty && saved.isEmpty) { n += 1 }
        return n
    }

    /// Text typed but not yet a value counts as a value once the chain is written; empty rows go.
    func settled() -> ChainDraft {
        var c = self
        c.rows = c.rows.compactMap { r in var r = r; r.values = r.allValues; r.pending = ""; return r.isEmpty ? nil : r }
        return c
    }
}

/// What the AI reads of a message (the AI card's ticks); the contract's `AiReads` without a second spelling.
struct FlowReads: Equatable, Sendable {
    var subject = true
    var body = true
    /// « Reste du fil »: the earlier messages of the thread, cut to the model's size limit (the core does the cutting). Off by default.
    var restOfThread = false
    var attachments = false
    init() {}
    init(_ r: AiReads) { subject = r.subject; body = r.body; restOfThread = r.restOfThread; attachments = r.attachments }
    var core: AiReads { AiReads(subject: subject, body: body, restOfThread: restOfThread, attachments: attachments) }
}

/// The three cards of a label, as the owner edits them: the switches, what the AI reads, where the exit filter sends the rest, and the
/// chains beyond the first of the Entry card (`others`, alternatives) and the Exit card's own (`exits`; the card edits the first).
struct FlowCards: Equatable, Sendable {
    var entryOn = true
    var aiOn = true
    var exitOn = false
    var reads = FlowReads()
    /// a label id, "primary" or "junk"; nil = not chosen yet
    var fallback: String?
    var others: [ChainDraft] = []
    var exits: [ChainDraft] = []

    init() {}
    init(saved l: MailLabel, others o: [DecodedChain], exits e: [DecodedChain]) {
        entryOn = l.entryEnabled; aiOn = l.aiEnabled; exitOn = l.exitEnabled
        reads = FlowReads(l.aiReads); fallback = l.exitFallback
        others = o.map(ChainDraft.init); exits = e.map(ChainDraft.init)
    }

    /// One change per switch, per tick, for the fallback, and per chain that is new, gone or different.
    func changes(from s: FlowCards) -> Int {
        var n = [entryOn != s.entryOn, aiOn != s.aiOn, exitOn != s.exitOn, reads.subject != s.reads.subject, reads.body != s.reads.body,
                 reads.restOfThread != s.reads.restOfThread, reads.attachments != s.reads.attachments, fallback != s.fallback].filter { $0 }.count
        for (mine, was) in [(others, s.others), (exits, s.exits)] {
            let a = mine.filter { !$0.isEmpty }.map { "\($0.chainId.map(String.init) ?? "new")|" + $0.signature.joined(separator: "\u{2}") }
            let b = was.filter { !$0.isEmpty }.map { "\($0.chainId.map(String.init) ?? "new")|" + $0.signature.joined(separator: "\u{2}") }
            n += a.filter { !b.contains($0) }.count + b.filter { !a.contains($0) }.count
        }
        return n
    }
}
