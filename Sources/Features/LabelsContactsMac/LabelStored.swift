import Foundation

/// The reads of stored conditions: the store's JSON ({"kind":"from","words":".."}, {"kind":"fact","fact":"unread","wanted":true}) as the
/// contract's `RuleCondition`. An unknown kind or fact key is skipped here, never guessed (the core refuses it in words when it writes).
enum StoredRules {
    static func condition(_ c: [String: Any]) -> RuleCondition? {
        let words = c["words"] as? String ?? ""
        switch c["kind"] as? String {
        case "from": return .from(words: words)
        case "to": return .to(words: words)
        case "cc": return .cc(words: words)
        case "domain": return (c["domain"] as? String).map { .domain(domain: $0) }
        case "subject": return .subjectContains(words: words)
        case "body": return .bodyContains(words: words)
        case "list_header": return .listHeader(words: words)
        case "account": return (c["account_id"] as? String).map { .account(accountId: $0) }
        case "has_attachment": return .hasAttachment
        case "not":   // the negated twin: its field's key (the positive kind's word) and its value
            guard let key = c["field"] as? String, let field = RuleKind.field(key: key), let value = c["value"] as? String else { return nil }
            return .not(field: field, value: value)
        case "fact":
            guard let key = c["fact"] as? String, let fact = messageFactFromKey(key: key), let wanted = c["wanted"] as? Bool else { return nil }
            return .fact(fact: fact, wanted: wanted)
        default: return nil
        }
    }

    static func conditions(_ list: Any?) -> [RuleCondition] { (list as? [[String: Any]] ?? []).compactMap(condition) }
    static func mode(_ word: Any?) -> MatchMode { (word as? String) == "all" ? .all : .any }

    static func object(_ json: String?) -> Any? {
        guard let json, let data = json.data(using: .utf8) else { return nil }
        return try? JSONSerialization.jsonObject(with: data)
    }
}

/// `label/<id>/chains/<entry|exit>`: one object per chain (`chain_id`, `match`, `conditions`, ...), in chain order.
enum StoredChains {
    static func decode(_ json: String?) -> [DecodedChain] {
        (StoredRules.object(json) as? [[String: Any]] ?? []).compactMap { o in
            guard let id = (o["chain_id"] as? NSNumber)?.uint32Value else { return nil }
            return DecodedChain(id: id, mode: StoredRules.mode(o["match"]), conditions: StoredRules.conditions(o["conditions"]))
        }
        .sorted { $0.id < $1.id }
    }
}

/// `label/<id>/flow/counts`: the four figures of the foot, from the filings (the core derives them; nothing is counted twice).
struct FlowCounts: Equatable, Sendable {
    var entry = 0, ai = 0, kept = 0, sent = 0

    init() {}
    init?(_ json: String?) {
        guard let o = StoredRules.object(json) as? [String: Any] else { return nil }
        func n(_ k: String) -> Int { (o[k] as? NSNumber)?.intValue ?? 0 }
        entry = n("caught_by_entry"); ai = n("sorted_by_ai"); kept = n("kept_by_exit"); sent = n("sent_to_fallback")
    }

    /// The messages the label holds, in the pipeline's reading (SPEC A140): with the exit filter on, those it kept; else the AI's accepts when the AI card
    /// is on, else the mails step 1 picked (it files them alone). `caught_by_entry` is what step 1 PICKED, not what the label holds.
    func held(aiOn: Bool, exitOn: Bool) -> Int { exitOn ? kept : (aiOn ? ai : entry) }
}
