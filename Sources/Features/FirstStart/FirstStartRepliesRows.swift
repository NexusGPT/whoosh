import SwiftUI

/// The socket's view of step 8: `rows mac-first-start.replies` (the three choices with their options and what is chosen), and one click door
/// per option (`click mac-first-start.replies.<row>.<option>`), which sets the choice as the pop-up does. Doors answer false off step 8, as for a person.
@MainActor
enum FirstStartRepliesRows {
    private static let P = "\(FirstStartFeature.X).replies"
    private static var m: FirstStartModel { .shared }
    private static var on: Bool { m.active && !m.skipped && m.step == FirstStartModel.repliesStep }

    static func rows() -> [ControlRow] {
        guard on else { return [] }
        return RepliesStepRows.choices().map { c in var r: ControlRow = c; r["ax"] = "\(P).\(c["id"] as? String ?? "")"; return r }
    }

    static func clicks() -> [String: (AppModel) -> Bool] {
        let r = RepliesStepModel.shared
        var t: [String: (AppModel) -> Bool] = [:]
        for c in RepliesDraftChoice.allCases { t["\(P).draft.\(c.rawValue)"] = { _ in guard on else { return false }; r.draft = c; return true } }
        for c in RepliesSearchChoice.allCases { t["\(P).search.\(c.rawValue)"] = { _ in guard on else { return false }; r.search = c; return true } }
        for c in RepliesVoiceChoice.allCases { t["\(P).voice.\(c.rawValue)"] = { _ in guard on else { return false }; r.voice = c; return true } }
        return t
    }

    /// the card's own row at step 8: the same keys the coach cards give, plus the three choices
    static func card() -> ControlRow {
        ["ax": "\(FirstStartFeature.X).card", "step": m.step, "kicker": RepliesStepWords.kicker, "title": RepliesStepWords.title, "body": RepliesStepWords.body,
         "count": FirstStartWords.stepOf(m.step), "dots": FirstStartModel.stepCount, "dot_current": m.step, "back_enabled": true, "choices": RepliesStepRows.choices()]
    }
}
