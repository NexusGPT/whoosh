import Foundation

/// The rows and click doors of the Réponses step, registered by WelcomeMacFeature next to the import step's (ids under `mac-welcome-accounts.replies.`).
/// A door sets a choice exactly as the pop-up does; `apply` is Continue's write, in place, so a read right after it sees the core's answer.
@MainActor enum RepliesStepRows {
    private static let P = "\(WelcomeMacFeature.W).replies"
    private static var m: RepliesStepModel { RepliesStepModel.shared }

    /// the three choices, each with the ids of its options, the chosen id and its words
    static func choices() -> [[String: Any]] {
        [["id": "draft", "title": RepliesStepWords.draftRow, "caption": "", "chosen": m.draft.rawValue, "words": RepliesStepWords.draft(m.draft),
          "options": RepliesDraftChoice.allCases.map { ["id": $0.rawValue, "words": RepliesStepWords.draft($0)] }],
         ["id": "search", "title": RepliesStepWords.searchRow, "caption": RepliesStepWords.searchCaption, "chosen": m.search.rawValue, "words": RepliesStepWords.search(m.search),
          "options": RepliesSearchChoice.allCases.map { ["id": $0.rawValue, "words": RepliesStepWords.search($0)] }],
         ["id": "voice", "title": RepliesStepWords.voiceRow, "caption": RepliesStepWords.voiceCaption, "chosen": String(m.voice.rawValue), "words": RepliesStepWords.voice(m.voice.rawValue),
          "options": RepliesVoiceChoice.allCases.map { ["id": String($0.rawValue), "words": RepliesStepWords.voice($0.rawValue)] }]]
    }

    static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        [
            "\(P).choices": { _ in choices().map { c in var r: ControlRow = c; r["ax"] = "\(P).\(c["id"] as? String ?? "")"; return r } },
            "\(P).applied": { _ in
                [["ax": "\(P).applied", "applied": m.applied, "writes": m.written.map(\.name), "refusal": m.refusal,
                  "folders": m.written.compactMap { w -> String? in if case let .source(a, f, on) = w { return "\(a)/\(f)=\(on)" } else { return nil } }]]
            },
        ]
    }

    static func clickTargets() -> [String: (AppModel) -> Bool] {
        var t: [String: (AppModel) -> Bool] = [
            "\(P).apply": { _ in m.apply(); return m.applied },
            "\(P).reset": { _ in m.reset(); return true },
        ]
        for c in RepliesDraftChoice.allCases { t["\(P).draft.\(c.rawValue)"] = { _ in m.draft = c; return true } }
        for c in RepliesSearchChoice.allCases { t["\(P).search.\(c.rawValue)"] = { _ in m.search = c; return true } }
        for c in RepliesVoiceChoice.allCases { t["\(P).voice.\(c.rawValue)"] = { _ in m.voice = c; return true } }
        return t
    }
}
