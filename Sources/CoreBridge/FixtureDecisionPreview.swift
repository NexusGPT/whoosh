import Foundation

/// `Command.previewDecisionList` in the test engine (gap-preview-core): the SAME contract as the real core (core/crates/engine/src/decision_preview.rs): the list given is the
/// list used (an unticked entry decides nothing, the entry numbers still count it), at most DECISION_PREVIEW_N rows, each with who decided in words, the sender and subject,
/// and the tab or answer; nothing is written (only the rows kept for `decisionPreview()` and the sentence). The mail is the INTERIM sample below, the only place it lives
/// (it moved here from Settings > Decisions: the page asks the core and draws what comes back). The sample model answers per page; a model entry whose question is blank
/// asks nothing and answers nothing (an edited question changes the rows that way, and only that way).
struct FixtureSampleMessage {
    let id: String
    let sender: String
    let subject: String
    var vip = false, list = false, me = false, code = false, reply = false
    let today: String
    let model: [String: (String, Int)]
}

enum FixtureDecisionPreview {
    /// The sentence a fixture preview says (the page shows it where the real core's own sentence would go).
    static let sampleWords = "Preview data is a sample until the decision engine lands."

    static let sample: [FixtureSampleMessage] = [
        .init(id: "m01", sender: "Linea Supply", subject: "Quote for the Q1 print run", vip: true, today: "Primary", model: ["which-tab": ("Primary", 93), "needs-reply": ("yes", 81)]),
        .init(id: "m02", sender: "GitHub", subject: "[whoosh] Pull request #42", list: true, today: "Updates", model: ["which-tab": ("Updates", 97)]),
        .init(id: "m03", sender: "Running Belgium", subject: "Brussels 20k: bib pickup times", today: "Primary", model: ["which-tab": ("Promotions", 84), "needs-reply": ("no", 90)]),
        .init(id: "m04", sender: "Jonas Peeters", subject: "Can we move Thursday to 15:00?", reply: true, today: "Primary", model: ["which-tab": ("Primary", 95), "needs-reply": ("yes", 92)]),
        .init(id: "m05", sender: "Me", subject: "Notes to self", me: true, today: "Primary", model: ["which-tab": ("Primary", 70)]),
        .init(id: "m06", sender: "Service security", subject: "Your verification code is 482913", code: true, today: "Updates", model: ["which-tab": ("Updates", 91), "sign-in-codes": ("yes", 99)]),
        .init(id: "m07", sender: "Service team", subject: "Welcome aboard", today: "Primary", model: ["which-tab": ("Updates", 72)]),
        .init(id: "m08", sender: "HostCloud", subject: "Invoice 2026-11 is available", today: "Primary", model: ["which-tab": ("Updates", 91)]),
        .init(id: "m09", sender: "Bart Tenders", subject: "Tender AQP: questions for Friday", reply: true, today: "Primary", model: ["which-tab": ("Primary", 88), "needs-reply": ("yes", 77)]),
        .init(id: "m10", sender: "Provanta", subject: "RFP 2026: pricing annex", reply: true, today: "Primary", model: ["which-tab": ("Primary", 90), "needs-reply": ("yes", 86), "labels": ("Work › Clients › Provanta › RFP 2026", 82)]),
        .init(id: "m11", sender: "Marie Lefèvre", subject: "Dinner on Saturday?", vip: true, reply: true, today: "Primary", model: ["which-tab": ("Primary", 96)]),
        .init(id: "m12", sender: "Figma", subject: "New comment on Whoosh / Settings", list: true, today: "Updates", model: ["which-tab": ("Updates", 89)]),
        .init(id: "m13", sender: "Zalando", subject: "−30 % this weekend only", list: true, today: "Promotions", model: ["which-tab": ("Promotions", 98)]),
        .init(id: "m14", sender: "LinkedIn", subject: "You appeared in 9 searches", list: true, today: "Social", model: ["which-tab": ("Social", 94)]),
        .init(id: "m15", sender: "Karim Haddad", subject: "Signed contract attached", vip: true, today: "Primary", model: ["which-tab": ("Primary", 97), "needs-reply": ("no", 64)]),
        .init(id: "m16", sender: "Bank", subject: "Sign in from a new device", code: true, today: "Primary", model: ["which-tab": ("Updates", 86), "sign-in-codes": ("yes", 88)]),
        .init(id: "m17", sender: "Nordlicht", subject: "Re: delivery window", reply: true, today: "Primary", model: ["which-tab": ("Primary", 79), "needs-reply": ("yes", 74)]),
        .init(id: "m18", sender: "Calendar", subject: "Weekly digest", list: true, today: "Updates", model: ["which-tab": ("Updates", 93)]),
        .init(id: "m19", sender: "Provanta Billing", subject: "Your invoice is ready", today: "Primary", model: ["which-tab": ("Updates", 90), "labels": ("Work › Clients › Provanta › Billing", 91)]),
        .init(id: "m20", sender: "Sofie Claes", subject: "Photos from the weekend", today: "Primary", model: ["which-tab": ("Primary", 83), "needs-reply": ("no", 70)]),
    ]

    static func matches(_ key: String, _ m: FixtureSampleMessage) -> Bool {
        switch key {
        case "vip": m.vip
        case "list": m.list
        case "me": m.me
        case "code": m.code
        case "reply": m.reply
        case let s where s.hasPrefix("from:"): m.sender.lowercased().contains(s.dropFirst(5))
        case let s where s.hasPrefix("subject:"): m.subject.lowercased().contains(s.dropFirst(8))
        default: false
        }
    }

    /// First ticked entry that answers wins. A model entry answers only at or above its trust, with a question, and with the engine on; an unticked entry is skipped.
    static func rows(page: String, entries: [PreviewEntry], engineOn: Bool, limit: Int = Int(WhooshConstants.DECISION_PREVIEW_N)) -> [DecisionVerdict] {
        sample.prefix(limit).map { m in
            for (i, e) in entries.enumerated() where e.enabled {
                let n = i + 1
                switch e.kind {
                case .rule where matches(e.matchKey, m):
                    // the Labels page files under a node of the tree: the row names the node path, not only the entry
                    return row(m, page, e.outcome, n, page == "labels" ? "entry \(n) · rule · \(e.outcome)" : "entry \(n) · rule")
                case .model where engineOn && !e.question.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty:
                    if let (answer, conf) = m.model[page], conf >= Int(e.trust ?? 100) {
                        return row(m, page, answer, n, String(format: "entry %d · the model said %@ (%.2f)", n, answer, Double(conf) / 100))
                    }
                case .otherwise:
                    return row(m, page, e.outcome, n, "entry \(n) · otherwise")
                default: break
                }
            }
            return DecisionVerdict(messageId: m.id, entry: nil, outcome: "—", differs: false, decidedBy: "no entry answered", sender: m.sender, subject: m.subject)
        }
    }

    private static func row(_ m: FixtureSampleMessage, _ page: String, _ outcome: String, _ n: Int, _ by: String) -> DecisionVerdict {
        DecisionVerdict(messageId: m.id, entry: UInt32(n), outcome: outcome, differs: page == "which-tab" && outcome != m.today, decidedBy: by, sender: m.sender, subject: m.subject)
    }
}

extension FixtureCore {
    /// Called with the lock held, like every `*Locked` of the test engine. An unknown page is refused in words, as the real core does.
    func previewDecisionListLocked(page: String, entries: [PreviewEntry]) throws -> CommandOutcome {
        let known = ["which-tab", "needs-reply", "sign-in-codes", "vips", "notify-me", "labels"]
        guard known.contains(page) else { throw CoreError.Invalid(field: "page_id", reasonWords: "there is no decision page “\(page)”") }
        let engineOn = (settings.decision?.provider ?? .local) != .rules
        settings.keepPreview(FixtureDecisionPreview.rows(page: page, entries: entries, engineOn: engineOn), words: FixtureDecisionPreview.sampleWords, nowMs: clock.nowMs())
        return CommandOutcome(undoEntryId: nil, createdId: nil)
    }
}
