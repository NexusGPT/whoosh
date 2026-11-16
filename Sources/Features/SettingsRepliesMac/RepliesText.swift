import Foundation

/// The facts grid of a reply filter card: the contract's own cells (`messageFactsInGridOrder`, `factStates`, `setFact`), so the eight facts read
/// and write exactly as the label inspector's do.
enum RepliesFacts {
    static var order: [MessageFact] { messageFactsInGridOrder() }

    static func state(_ conditions: [RuleCondition], _ fact: MessageFact) -> FactState {
        factStates(conditions: conditions).first { $0.fact == fact }?.state ?? .any
    }
    static func set(_ conditions: [RuleCondition], _ fact: MessageFact, _ state: FactState) -> [RuleCondition] {
        setFact(conditions: conditions, fact: fact, state: state)
    }
    /// The From row of a rule (the « Expéditeur » field); "" when there is none.
    static func sender(_ conditions: [RuleCondition]) -> String {
        for c in conditions { if case .from(let w) = c { return w } }
        return ""
    }
    /// « Étiquettes À répondre, Clients · Liste de diffusion : non »: what a card keeps, in one line, for the rows of Brouillon automatique.
    static func summary(_ rule: RuleSet, labels: String?) -> String {
        var parts: [String] = []
        if let labels { parts.append(labels) }
        let set = factStates(conditions: rule.conditions).filter { $0.state != .any }
            .map { Words.text("%@: %@", FlowFacts.title($0.fact), $0.state == .yes ? Words.text("yes") : Words.text("no")) }   // round 24: the language's own colon
        if !set.isEmpty { parts.append(set.joined(separator: ", ")) }
        let s = sender(rule.conditions)
        if !s.isEmpty { parts.append(RepliesWords.senderIs(s)) }
        return parts.isEmpty ? RepliesWords.everything : parts.joined(separator: " · ")
    }
}

extension ReplySource {
    /// Delta 24: the server no longer holds this mail folder; the row is off and keeps its last count.
    var isGone: Bool { if case .gone = progress { return true } else { return false } }
}

/// The variables of the prompts as the owner reads them: `{{message}}` is stored, « @message » is shown and typed. The words come from the
/// catalogue keys the contract names (`reply.var.<id>.name`, `.meaning`); a key not yet in the catalogue shows the id.
enum RepliesVariables {
    static func name(_ v: ReplyPromptVariable) -> String { Words.text(v.nameKey, default: v.id) }
    static func meaning(_ v: ReplyPromptVariable) -> String { Words.text(v.meaningKey, default: "") }

    /// `{{id}}` → « @nom ».
    static func show(_ text: String, _ vars: [ReplyPromptVariable]) -> String {
        vars.reduce(text) { t, v in t.replacingOccurrences(of: promptPlaceholder(id: v.id), with: "@" + name(v)) }
    }
    /// « @nom » → `{{id}}`, the longest names first (« @extraits trouvés » before a shorter name it starts with).
    static func store(_ text: String, _ vars: [ReplyPromptVariable]) -> String {
        vars.sorted { name($0).count > name($1).count }.reduce(text) { t, v in t.replacingOccurrences(of: "@" + name(v), with: promptPlaceholder(id: v.id)) }
    }

    /// One piece of a prompt line: words, or a variable chip.
    enum Piece: Hashable { case words(String), variable(String) }

    /// A stored line cut into words and chips, for the page that reads like the board (`{{id}}` as a blue « @nom » chip).
    static func pieces(_ line: String, _ vars: [ReplyPromptVariable]) -> [Piece] {
        var out: [Piece] = []
        var rest = Substring(line)
        while let open = rest.range(of: "{{"), let close = rest[open.upperBound...].range(of: "}}") {
            if open.lowerBound > rest.startIndex { out.append(.words(String(rest[..<open.lowerBound]))) }
            let id = String(rest[open.upperBound..<close.lowerBound])
            out.append(.variable(vars.first { $0.id == id }.map(name) ?? id))
            rest = rest[close.upperBound...]
        }
        if !rest.isEmpty { out.append(.words(String(rest))) }
        return out
    }
}
