import Foundation

/// One row of "Preview on my last 20 messages", as the page draws it.
struct PreviewRow: Identifiable, Equatable {
    let id: String
    let sender: String
    let subject: String
    let outcome: String
    let entry: Int
    let decidedBy: String
    let differs: Bool
}

/// The preview asks the core (`Command.previewDecisionList`): the page sends its list as it stands now (every entry, ticked or not, with its question as edited)
/// and draws the rows the core keeps (`decisionPreview()`): who decided in words, the sender and the subject of the mail, the tab or answer. The sample mail the page
/// used to carry lives only in the test engine (Sources/CoreBridge/FixtureDecisionPreview.swift).
enum DecisionPreview {
    /// The page's list as the core's command carries it, in the page's order (an unticked entry is sent and decides nothing, so its number still counts).
    static func entries(of page: DecisionPage) -> [PreviewEntry] {
        page.entries.map { e in
            let kind: PreviewEntryKind = switch e.kind { case .rule: .rule; case .model: .model; case .default: .otherwise }
            return PreviewEntry(kind: kind, enabled: e.enabled, matchKey: e.kind == .rule ? (e.match ?? "") : "", outcome: e.outcome,
                                question: e.kind == .model ? e.text : "", trust: e.trust.map { UInt32(max(0, $0)) })
        }
    }

    /// The core's verdicts as the rows the page draws (entry 0 = no entry answered).
    static func rows(from verdicts: [DecisionVerdict]) -> [PreviewRow] {
        verdicts.map { PreviewRow(id: $0.messageId, sender: $0.sender, subject: $0.subject, outcome: $0.outcome, entry: Int($0.entry ?? 0), decidedBy: $0.decidedBy, differs: $0.differs) }
    }
}
