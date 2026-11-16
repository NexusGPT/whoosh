import Foundation

/// One chain of a rule card, as words: "File when", how its lines combine, and one line per fact or text rule.
struct PhoneChainRead: Equatable, Sendable, Identifiable {
    let id: Int
    let lead: String
    let mode: String?
    let lines: [String]
}

/// One of the three cards of a label as the phone shows it (read only). The names are the Mac's: Entry rules, AI sorting, Exit filter.
struct PhoneCardRead: Equatable, Sendable, Identifiable {
    var id: String { key }
    let key: String
    let title: String
    let subtitle: String
    let count: String
    let on: Bool
    let chains: [PhoneChainRead]
    let lines: [String]
    let note: String?
    var allLines: [String] { chains.flatMap(\.lines) + lines }
}

/// A label as its page shows it.
struct PhoneLabelRead: Equatable, Sendable {
    let id: String
    let name: String
    let path: String
    let depth: Int
    let colour: String?
    let meaning: String
    let pinned: Bool
    let scope: String
    let unread: Int
    let mail: Int
    let cards: [PhoneCardRead]
}

/// Reads one label from the core: the label row, its chains and cards (the Mac inspector's own read), its three counts.
/// Nothing is stored here.
@MainActor
enum PhoneLabelReader {
    static func read(_ id: String, _ m: AppModel) -> PhoneLabelRead? {
        guard let l = m.core.labels().first(where: { $0.id == id }) else { return nil }
        let d = LabelInspectorModel.shared.readDraft(l)
        let counts = FlowCounts(m.core.state(path: "label/\(id)/flow/counts"))
        let scope: String
        switch l.scope {
        case .all: scope = Words.text("All mailboxes")
        case .account(let a): scope = AccountBook.shared.account(a)?.name ?? a
        }
        return PhoneLabelRead(id: l.id, name: l.name, path: l.path, depth: Int(l.depth), colour: l.colour, meaning: l.description,
                              pinned: l.pinned, scope: scope, unread: Int(l.unread), mail: mail(id, m).count,
                              cards: [entry(d, counts, m), ai(d, counts, l.id, m), exit(d, counts, m)])
    }

    /// Every message that carries the label, across tabs and accounts, newest first.
    static func mail(_ id: String, _ m: AppModel) -> [InboxRow] {
        var seen = Set<String>()
        return m.tabIds.flatMap { m.core.inbox(query: InboxQuery(tabId: $0, accountId: nil, labelId: id)) }
            .filter { seen.insert($0.messageId).inserted }
            .sorted { $0.receivedAtMs > $1.receivedAtMs }
    }

    private static func chain(_ c: ChainDraft, lead: String, index: Int) -> PhoneChainRead {
        var lines: [String] = []
        for (fact, state) in zip(ChainDraft.order, c.facts) where state != .any {
            lines.append("\(FlowFacts.title(fact)): \(Words.text(state == .yes ? "yes" : "no"))")
        }
        for r in c.rows where !r.isEmpty {
            lines.append("\(r.kind.title) \(r.allValues.joined(separator: " \(Words.text("or")) "))")
        }
        let mode = lines.count > 1 ? Words.text(c.matchMode == .all ? "All of these" : "Any of these") : nil
        return PhoneChainRead(id: index, lead: lead, mode: mode, lines: lines)
    }

    private static func entry(_ d: LabelDraft, _ c: FlowCounts?, _ m: AppModel) -> PhoneCardRead {
        let chains = ([d.entry] + d.flow.others).enumerated().map { chain($1, lead: Words.text("File when"), index: $0) }.filter { !$0.lines.isEmpty }
        return PhoneCardRead(key: "entry", title: Words.text("Entry rules"), subtitle: d.flow.aiOn ? Words.text("picks the mails the AI reads") : Words.text("these mails go straight in"),
                             count: c.map { Words.text("picks %lld", $0.entry) } ?? "", on: d.flow.entryOn,
                             chains: chains, lines: chains.isEmpty ? [Words.text("No rule yet")] : [], note: nil)
    }

    private static func ai(_ d: LabelDraft, _ c: FlowCounts?, _ id: String, _ m: AppModel) -> PhoneCardRead {
        // the picked mails the AI has not ruled on yet (`waiting_for_ai` of the same read); only while picks outnumber accepts, so the read is rare
        let waiting = (d.flow.aiOn && (c.map { $0.entry > $0.ai } ?? false)) ? ((StoredRules.object(m.core.state(path: "label/\(id)/flow/counts")) as? [String: Any])?["waiting_for_ai"] as? NSNumber)?.intValue ?? 0 : 0
        let r = d.flow.reads
        let names = [(r.subject, "Subject"), (r.body, "Body"), (r.restOfThread, "Rest of the thread"), (r.attachments, "Attachments")]
            .filter { $0.0 }.map { Words.text($0.1) }
        let reads = "\(Words.text("The AI reads")): \(names.joined(separator: ", "))"
        return PhoneCardRead(key: "ai", title: Words.text("AI sorting"), subtitle: Words.text("from the description"),
                             count: c.map { LabelsWords.accepted($0.ai, waiting: waiting) } ?? "", on: d.flow.aiOn, chains: [], lines: [reads],
                             note: d.flow.aiOn ? AiRefusal.words(m.core.state(path: "label/\(id)/flow/ai_refusal")) : nil)
    }

    private static func exit(_ d: LabelDraft, _ c: FlowCounts?, _ m: AppModel) -> PhoneCardRead {
        let chains = (d.flow.exits.first.map { [chain($0, lead: Words.text("Keep when"), index: 0)] } ?? []).filter { !$0.lines.isEmpty }
        var lines: [String] = []
        if let to = place(d.flow.fallback, m) { lines.append("\(Words.text("The others go to")) \(to)") }
        return PhoneCardRead(key: "exit", title: Words.text("Exit filter"), subtitle: Words.text("what the AI sorted here must pass"),
                             count: c.map { Words.text("keeps %lld of %lld", $0.kept, d.flow.aiOn ? $0.ai : $0.entry) } ?? "", on: d.flow.exitOn,
                             chains: chains, lines: lines, note: nil)
    }

    private static func place(_ id: String?, _ m: AppModel) -> String? {
        guard let id, !id.isEmpty else { return nil }
        if id == "primary" { return Words.text("Primary") }
        if id == "junk" { return Words.text("Spam") }
        return m.core.labels().first { $0.id == id }?.name
    }
}
