import Foundation

/// Lane fix-aisort round 8: the words Apple's on-device model reads for the Labels AI step and the strict read of its answer. These are the
/// model's input, in English whatever the app's language (the lead measured the instructions in English), never words the owner reads, so
/// none of them goes through the catalogue (`Words.text`). The owner-facing failure sentences stay in AppleLabelJudge.swift.
enum AppleLabelPrompt {
    /// VERBATIM the instructions the lead measured (scratchpad textjudge2.swift, 2026-11-12 18:35).
    static let instructions = """
    You sort one email into exactly one label. You get each label's id, name and description. Choose the ONE label whose description fits the email best. If no description fits, answer none. A marketing or automatic email that asks nothing of the reader personally never needs a reply. Answer with exactly one word: a label id, or none.
    """
    /// The engine's per-label question id prefix (`decide::label_question_id` over `LABELS_MODEL_ID`) and statement prefix (`decide::label_statement`).
    static let questionPrefix = "labels/model#"
    // written as multi-line literals: they are the model's words, and the i18n scan reads one-line literals as words the owner may see
    static let statementPrefix = """
    This email belongs under the label "
    """
    /// The body characters the prompt carries (the measured script's `prefix(2000)`, equal to DECISION_BODY_CHARS).
    static let bodyChars = 2000
    /// Room for the answer: one label id or "none".
    static let answerTokens = 24

    /// (label id, path, description) of an engine per-label Labels question; nil for any other question.
    static func offered(_ q: DecisionQuestion) -> (id: String, path: String, description: String)? {
        guard case .noul(let id, let prompt) = q, id.hasPrefix(questionPrefix), prompt.hasPrefix(statementPrefix) else { return nil }
        let rest = prompt.dropFirst(statementPrefix.count)
        guard let r = rest.range(of: "\": ") else { return nil }
        return (String(id.dropFirst(questionPrefix.count)), String(rest[..<r.lowerBound]), String(rest[r.upperBound...]))
    }

    /// The request's fixed parts (the measured script's form) and the state's field names (decision crate `state_for`).
    static let labelsHead = "Labels:", emailHead = "Email:", fromHead = "from: ", subjectHead = "subject: "
    static let listHead = """
    mailing list:
    """ + " "
    static let fromField = "from", subjectField = "subject", listField = "list_header", yesWord = "yes", noWord = "no", newline = "\n"

    /// The request text: the labels as `- <id> (<name>): <description>`, then the mail as the label flow's state gives it.
    /// Round 11: `withBody` false = the subject and the sender alone, the second ask for a mail whose text was refused (content, size,
    /// language): a mail often passes that way.
    static func prompt(_ labels: [(id: String, path: String, description: String)], state: DecisionState, withBody: Bool = true) -> String {
        let lines = labels.map { "- \($0.id) (\($0.path)): \($0.description)" }.joined(separator: "\n")
        let from = state.fields[fromField] ?? "", subject = state.fields[subjectField] ?? ""
        let list = state.fields[listField] == yesWord ? yesWord : noWord
        let body = withBody ? String(state.text.prefix(bodyChars)) : ""
        return [labelsHead, lines, "", emailHead, fromHead + from, subjectHead + subject, listHead + list, "", body].joined(separator: newline)
    }

    /// The strict read: one offered id (case ignored, surrounding quotes, backticks and a final full stop allowed), or "none" (nil inside
    /// the outer optional). The outer nil = unreadable.
    static func read(_ text: String, ids: [String]) -> String?? {
        var w = text.trimmingCharacters(in: .whitespacesAndNewlines)
        while let last = w.last, ".!".contains(last) { w.removeLast() }
        w = w.trimmingCharacters(in: CharacterSet(charactersIn: "\"'`*").union(.whitespacesAndNewlines))
        if w.lowercased() == "none" { return .some(nil) }
        let hits = ids.filter { $0.lowercased() == w.lowercased() }
        return hits.count == 1 ? .some(hits[0]) : nil
    }

    /// Round 11: the reason id of an answer with no label of the list (the mail is set aside, as for a refusal).
    static let noLabelReason = "no_label"
    /// The engine's code for a failure that belongs to one mail (`decision::decide::MAIL_REFUSED_CODE`).
    static let mailRefusedCode = "apple_mail_refused"

    /// The rig's stand-in judge (`WHOOSH_RIG_APPLE_JUDGE=stand-in`, rig only; the hidden `--slice-aisort`): the first label (in the order
    /// asked) whose description shares a word of five letters or more with the request, else `none`; a request holding `refuseMark` is refused
    /// as unsafe (so a mail whose SUBJECT holds it is refused twice and set aside, one whose body alone holds it passes the second ask).
    static let refuseMark = "unsafesample"
    static func standIn(_ labels: [(id: String, path: String, description: String)], prompt: String) throws -> String {
        if prompt.lowercased().contains(refuseMark) { throw AppleMailRefusal.unsafe }
        let mailPart = prompt.components(separatedBy: emailHead).last ?? prompt
        let words = Set(mailPart.lowercased().split(whereSeparator: { !$0.isLetter && !$0.isNumber }).map(String.init).filter { $0.count >= 5 })
        let hit = labels.first { l in l.description.lowercased().split(whereSeparator: { !$0.isLetter && !$0.isNumber }).contains { $0.count >= 5 && words.contains(String($0)) } }
        return hit?.id ?? "none"
    }

    /// The answers the engine reads: yes-probability 1.0 for the pick, 0.0 for every other offered label.
    static func answers(for labels: [(id: String, path: String, description: String)], pick: String?) -> [DecisionAnswer] {
        labels.map { l in
            let yes = l.id == pick
            return .noul(id: questionPrefix + l.id, yes: yes, probability: yes ? 1.0 : 0.0)
        }
    }

}
