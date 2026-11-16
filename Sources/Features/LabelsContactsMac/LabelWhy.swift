import Foundation

/// « Pourquoi ici ? » for the message in focus, read from the core (state `message/<id>/why/<label id>`, `message/<id>/flow_rejection`):
/// which card placed it, by which chain, and which conditions held. The core keeps no sentence; the words are made here, from the keys.
enum LabelWhy {
    static func lines(core: WhooshCore, label: String?, message: String?) -> [String] {
        guard let label, let mid = message, let m = core.message(messageId: mid) else { return [Words.text("Select a message in the list to see why it is here.")] }
        let head = m.subject.isEmpty ? m.senderName : m.subject
        if let o = StoredRules.object(core.state(path: "message/\(mid)/why/\(label)")) as? [String: Any] { return [head, placed(o, core: core, message: mid)] }
        if let r = StoredRules.object(core.state(path: "message/\(mid)/flow_rejection")) as? [String: Any], (r["ai_label_id"] as? String) == label {
            return [head, Words.text("The exit filter of this label sent it to %@.", name(r["sent_to"] as? String, core))]
        }
        return [head, Words.text("This label does not hold this message.")]
    }

    /// "Entry rules · chain 1 · Attachment: yes", "Is unread · AI: yes", or the fallback's and the hand's own sentence.
    private static func placed(_ o: [String: Any], core: WhooshCore, message: String) -> String {
        let card = o["card"] as? String ?? ""
        switch card {
        case "entry": return parts(Words.text("Entry rules"), o)
        case "ai": return viaAi(o, tail: nil)
        case "exit": return viaAi(o, tail: parts(Words.text("Exit filter"), o))
        case "fallback":
            let by = (StoredRules.object(core.state(path: "message/\(message)/flow_rejection")) as? [String: Any])?["ai_label_id"] as? String
            return Words.text("Sent here by the exit filter of %@", name(by, core))
        default: return Words.text("Placed by hand")
        }
    }

    /// SPEC A140: the steps that placed a mail the AI accepted: step 1's held conditions, then the AI's answer, then the exit filter's part when it kept it:
    /// "Is unread · AI: yes". `entry` is step 1's own reading (its chain and the conditions that held), read when the label has an entry chain that picked the mail.
    /// A mail the exit filter kept with the AI off keeps the plain exit sentence.
    private static func viaAi(_ o: [String: Any], tail: String?) -> String {
        guard (o["ai"] as? String) == "yes" else { return tail ?? Words.text("AI sorting") }
        var out: [String] = []
        if let e = o["entry"] as? [String: Any] { out += StoredRules.conditions(e["matched"]).map(words) }
        // SPEC A133: a mail an on-device model other than Laya accepted names that model (`decided_by` "model:<catalogue id>")
        if let by = o["decided_by"] as? String, by.hasPrefix("model:") {
            out.append(Words.text("AI (%@): yes", DecisionModelCatalogue.entry(id: String(by.dropFirst("model:".count))).name))
        } else {
            out.append(Words.text("AI: yes"))
        }
        if let tail { out.append(tail) }
        return out.joined(separator: " · ")
    }

    private static func parts(_ card: String, _ o: [String: Any]) -> String {
        var out = [card]
        if let n = (o["chain_id"] as? NSNumber)?.intValue { out.append(Words.text("chain %lld", n + 1)) }
        out += StoredRules.conditions(o["matched"]).map(words)
        return out.joined(separator: " · ")
    }

    /// One held condition in words: a fact as "Name: yes|no", a text condition as "Kind: value".
    static func words(_ c: RuleCondition) -> String {
        if let w = conditionFact(condition: c) { return Words.text("%@: %@", FlowFacts.title(w.fact), w.wanted ? Words.text("yes") : Words.text("no")) }
        guard let k = RuleKind(c) else { return "" }
        return Words.text("%@: %@", RuleKind.isNegated(c) ? Words.text("not %@", k.tag) : k.tag, RuleKind.value(of: c))
    }

    private static func name(_ id: String?, _ core: WhooshCore) -> String {
        guard let id else { return "" }
        if id == "primary" { return Words.text("Primary") }
        if id == "junk" { return Words.text("Spam") }
        return core.labels().first { $0.id == id }?.name ?? id
    }
}

/// The eight facts' names, in the owner's language (grid order = `ChainDraft.order`).
enum FlowFacts {
    static func title(_ f: MessageFact) -> String {
        switch f {
        case .unread: Words.text("Is unread")
        case .unanswered: Words.text("Unanswered")
        case .hasAttachment: Words.text("Attachment")
        case .starred: Words.text("Is starred")
        case .vipSender: Words.text("VIP sender")
        case .ccMe: Words.text("I am in copy")
        case .mailingList: Words.text("Mailing list")
        case .repliedByMe: Words.text("Already answered by me")
        }
    }
}
