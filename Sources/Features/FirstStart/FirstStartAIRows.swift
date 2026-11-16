import SwiftUI

/// The socket's view of step 7 (AI): `rows mac-first-start.ai` (the three lines with their state, words, progress and the actions on screen) and one click
/// door per action (`click mac-first-start.ai.<line>.<action>`); the doors answer false off step 7, as for a person. The key doors paste a fixed fixture key
/// (fixture core only); a real key never passes through the socket.
@MainActor
enum FirstStartAIRows {
    private static let P = "\(FirstStartFeature.X).ai"
    private static var m: FirstStartModel { .shared }
    private static var on: Bool { m.active && !m.skipped && m.step == FirstStartModel.aiStep }
    private static var step: AISetupStep { .shared }
    private static var status: AISetupStatus { .shared }

    private static func actions(_ n: AINeed) -> [String] {
        let x = status.status(n)
        switch n {
        case .decision:
            if x.state == .ready { return [] }
            if x.state == .working { return ["cancel"] }
            switch step.jev { case .key: return ["jev-key"]; case .consent: return ["jev-allow"]; default: return ["use-jev", "download"] }
        case .writing: return x.state == .ready ? [] : ["key"]
        case .meaning: return x.state == .ready || x.state == .working ? [] : ["download"]
        }
    }

    static func rows() -> [ControlRow] {
        guard on else { return [] }
        status.refresh()
        return AINeed.allCases.map { n in
            let x = status.status(n)
            let id = ["decision": "sorting", "writing": "writing", "meaning": "meaning"][n.rawValue] ?? n.rawValue
            var r: ControlRow = ["ax": "\(P).\(id)", "id": id, "need": n.rawValue, "state": x.state.rawValue, "reason": x.reason, "words": x.words, "detail": x.detail,
                                 "actions": actions(n), "jev": step.jev.rawValue, "met": x.usable]
            if let p = x.progress { r["progress"] = p }
            return r
        }
    }

    static func card() -> ControlRow {
        ["ax": "\(FirstStartFeature.X).card", "step": m.step, "kicker": AISetupWords.stepKicker, "title": AISetupWords.stepTitle, "body": AISetupWords.stepBody,
         "count": FirstStartWords.stepOf(m.step), "dots": FirstStartModel.stepCount, "dot_current": m.step, "back_enabled": true,
         "stage": AISetupWords.stageTitle, "rings": AINeed.allCases.map { status.status($0).usable }]
    }

    static func clicks() -> [String: (AppModel) -> Bool] {
        let key = AISetupRows.fixtureKey
        // The rig's `fixture provider-endpoint` (a pasted key's check, the Laya weights) is heard by AIFixture's observers, which the AI Settings feature starts
        // when ITS click table is first built. The registry builds a feature's table only for a click no earlier feature owns, and every step-7 click
        // is owned here, so on this screen the observers never started and the endpoint verb reached nobody. Fixture core only; the call is idempotent.
        if CoreHost.shared.kind == .fixture { AIFixture.listen(AISettingsStore.shared) }
        func door(_ id: String, fixtureOnly: Bool = false, _ run: @escaping () -> Void) -> (String, (AppModel) -> Bool) {
            ("\(P).\(id)", { _ in guard on, !fixtureOnly || CoreHost.shared.kind == .fixture else { return false }; run(); return true })
        }
        return Dictionary(uniqueKeysWithValues: [
            door("sorting.download") { step.download() },
            door("sorting.cancel") { AISetupDoors.cancel(.decision) },
            door("sorting.use-jev") { step.openJev() },
            door("sorting.jev-key", fixtureOnly: true) { step.submitJevKey(key) },
            door("sorting.jev-allow") { step.allowJev() },
            door("writing.key", fixtureOnly: true) { step.pasteWritingKey(key) },
            door("meaning.download") { AISetupDoors.fix(.meaning) },
        ])
    }
}
