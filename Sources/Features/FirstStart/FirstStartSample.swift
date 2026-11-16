import Foundation

/// The sample mail of the illustrations: the shared fixture's first page (docs/design/fixtures/mac-main-sample.json), as the
/// boards list it. It is data, not interface words: it stays as written, and the stage says "Sample mail" beside it.
struct FirstStartLine: Identifiable, Equatable {
    let id: String
    let sender: String
    let count: String
    let subject: String
    let time: String
    let unread: Bool
}

enum FirstStartSample {
    /// id | sender | thread count | subject | time | unread. A text table, read once: it is sample data, not interface words, so no translation key.
    private static let table = """
    m1|Linea Supply|3|Quote for the Q1 print run: 12,000 units|09:12|1
    m2|Marie Lefèvre|2|Final logo files + brand book v3|08:47|0
    m3|Ton Verheyden|5|Re: Clarification points, section 4.2|08:31|1
    m4|Hetzner-style host||Invoice 2026-11 is available|08:05|1
    m5|Nina Okafor|2|Workshop on 19 Nov: agenda draft|07:58|1
    m6|Apple Developer||Your app has been approved for TestFlight|07:40|0
    m7|Jonas Pieters|2|Can we move Thursday to 15:00?|07:21|1
    m8|Sophie Chammah||Photos from Sunday|Yesterday|0
    m9|GitHub||[whoosh] Pull request #42: Rust core: IMAP IDLE for 30 accounts|Yesterday|1
    m10|LHG Procurement|4|Cloud Service Agreement: countersigned copy|Yesterday|0
    m11|Stripe-style billing||Payout of 8,420.00 EUR sent|Yesterday|0
    m12|Karim Haddad|6|Term sheet v2, redlines|Thu|1
    m13|Sailing club||Winter storage: your slot is 17B|Thu|0
    m14|Elena Rossi|2|Intro: Dario (ex-Linear) on design systems|Wed|0
    m15|Running Belgium||Brussels 20k: bib pickup times|Wed|0
    m16|Bart De Smet||Tender AQP: question round closes Friday|Tue|1
    """
    static let lines: [FirstStartLine] = table.split(separator: "\n").compactMap { row in
        let f = row.split(separator: "|", omittingEmptySubsequences: false).map(String.init)
        guard f.count == 6 else { return nil }
        return FirstStartLine(id: f[0], sender: f[1], count: f[2], subject: f[3], time: f[4], unread: f[5] == "1")
    }

    /// the two label chips and the rule of step 4's sample lines: names of the user's own labels and rule, sample data like the mail
    private static let names = """
    Updates
    Invoices
    GitHub
    """.split(separator: "\n").map(String.init)
    static var chipUpdates: String { names[0] }
    static var chipInvoices: String { names[1] }
    static var ruleName: String { names[2] }
    static func line(_ id: String) -> FirstStartLine? { lines.first { $0.id == id } }

    /// The sample index of the step-2 try-it: meaning, not words (2b `MEANING`, ten rules). First rule that matches wins.
    static let meaning: [(pattern: String, id: String)] = [
        ("boat|sail|harbou?r|marina|yacht", "m13"), ("invoice|bill|pay|server|host", "m4"), ("logo|brand|design", "m2"),
        ("print|quote|price|brochure", "m1"), ("meet|agenda|workshop|thursday", "m5"), ("money|payout|stripe|euro", "m11"),
        ("run|race|marathon|20k", "m15"), ("contract|legal|agreement", "m10"), ("app|testflight|ios", "m6"), ("photo|picture|family", "m8"),
    ]

    /// The line a typed word means, or nil. Two letters at least (2b `meaning`).
    static func match(_ typed: String) -> FirstStartLine? {
        let q = typed.lowercased().trimmingCharacters(in: .whitespaces)
        guard q.count >= 2 else { return nil }
        for rule in meaning where q.range(of: rule.pattern, options: .regularExpression) != nil { return line(rule.id) }
        return nil
    }
}
