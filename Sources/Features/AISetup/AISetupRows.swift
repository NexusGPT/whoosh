import Foundation

/// The socket's view of the AI setup: `rows ai-setup.status` (one row per need), `rows ai-setup.step`, and the doors the checks use.
/// The paste doors take a fixed fixture key (fixture core only, the way the import step's file doors do); a real key never passes here.
@MainActor
enum AISetupRows {
    static let P = "ai-setup"
    private static var s: AISetupStatus { .shared }
    private static var step: AISetupStep { .shared }
    static let fixtureKey = "fixture-key"
    /// What the last press of a mark's Fix or Cancel did, in a word (`rows ai-setup.mark.last`): the mark's controls have no other visible outcome the socket can read
    /// (Fix opens a Settings pane or starts a download, Cancel stops one).
    private static var last: ControlRow?

    static func status() -> [ControlRow] {
        s.refresh()
        return AINeed.allCases.map { n in
            let x = s.status(n)
            var r: ControlRow = ["ax": "\(P).status.\(n.rawValue)", "need": n.rawValue, "name": name(n), "state": x.state.rawValue, "reason": x.reason, "words": x.words,
                                 "door_pane": x.door?.pane ?? "", "door_control": x.door?.control ?? "", "door_title": x.door?.title ?? "", "usable": x.usable]
            if let p = x.progress { r["progress"] = p }
            return r
        }
    }

    /// The places that carry a mark, and whether each shows now (they follow the needs): the Settings sidebar rows, the reader's summary card, the foot's ⌘J entry, the search list.
    static func places() -> [ControlRow] {
        s.refresh()
        func unmet(_ n: [AINeed]) -> Bool { n.contains { !s.status($0).usable } }
        let panes: [ControlRow] = [SettingsPane.decisions, .ai, .replies].map { ["ax": "\(P).place.settings-sidebar.\($0.id)", "place": "settings-sidebar.\($0.id)", "shown": unmet($0.aiNeeds)] }
        return panes + [["ax": "\(P).place.reader-summary", "place": "reader-summary", "shown": unmet([.writing])],
                        ["ax": "\(P).place.foot-reply-with-ai", "place": "foot-reply-with-ai", "shown": unmet([.writing]), "words": AISetupWords.replyWithAI],
                        ["ax": "\(P).place.search-meaning", "place": "search-meaning", "shown": unmet([.meaning])]]
    }

    /// What every line says, in both sentences, as the view draws it: `need`, `sentence` (status|consequence), `words`, `button`, `shown`.
    static func marks() -> [ControlRow] {
        s.refresh()
        return AINeed.allCases.flatMap { n -> [ControlRow] in
            let x = s.status(n)
            return [("status", MissingSetupSentence.status), ("consequence", .consequence)].map { name, kind in
                ["ax": "\(P).mark.\(n.rawValue).\(name)", "need": n.rawValue, "sentence": name, "shown": !x.usable, "working": x.state == .working,
                 "words": x.usable ? "" : AISetupMarkText.words(n, kind, x), "button": x.usable || x.state == .working ? "" : AISetupMarkText.button(n, kind, x),
                 "progress_words": x.state == .working ? x.words : "", "cancel": x.state == .working && n == .decision ? AISetupWords.cancel : ""]
            }
        }
    }

    static func stepRows() -> [ControlRow] {
        s.refresh()
        return [["ax": "\(P).step", "done": step.done, "can_continue": step.canContinue, "skipped_all": step.skippedAll,
                 "skipped": AINeed.allCases.filter { step.skippedNeeds.contains($0) }.map(\.rawValue), "marks_on": step.marksOn.map(\.rawValue)]]
    }

    static func name(_ n: AINeed) -> String {
        switch n { case .writing: return AISetupWords.writingName; case .decision: return AISetupWords.decisionName; case .meaning: return AISetupWords.meaningName }
    }

    static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        ["\(P).status": { _ in status() }, "\(P).places": { _ in places() }, "\(P).marks": { _ in marks() }, "\(P).step": { _ in stepRows() },
         "\(P).mark.last": { _ in last.map { [$0] } ?? [] }]
    }

    static func clickTargets() -> [String: (AppModel) -> Bool] {
        var t: [String: (AppModel) -> Bool] = [
            "\(P).reset": { _ in step.reset(); s.reset(); return true },
            "\(P).skip": { _ in step.skipAll(); return true },
            "\(P).download": { _ in step.download(); return true },
            "\(P).jev.consent": { _ in step.useJev(key: "", consent: true); return true },
        ]
        for n in AINeed.allCases {
            for st in ["missing", "working", "ready", "clear"] { t["\(P).preview.\(n.rawValue).\(st)"] = { _ in s.setPreview(n, st) } }   // fixture core only: a drawing's state for the hidden rig
        }
        for n in AINeed.allCases { t["\(P).skip.\(n.rawValue)"] = { _ in step.skip(n); return true } }
        // The mark's own buttons (welcome sweep F-m): drawn while the need is unmet and not working (Fix) or while the decision model downloads (Cancel); each runs what the
        // button runs (AISetupDoors) and says what it did in `mark.last`.
        for n in AINeed.allCases {
            t["\(P).mark.\(n.rawValue).fix"] = { _ in
                let x = s.status(n)
                guard !x.usable, x.state != .working else { return false }
                let result: String
                switch n {
                case .writing: result = "settings:ai"
                case .meaning: result = "meaning-download"
                case .decision: result = ["laya-missing", "laya-failed"].contains(x.reason) ? "laya-download" : "settings:decisions"
                }
                last = ["ax": "\(P).mark.last", "need": n.rawValue, "action": "fix", "result": result]
                AISetupDoors.fix(n); return true
            }
        }
        t["\(P).mark.decision.cancel"] = { _ in
            guard s.status(.decision).state == .working else { return false }
            last = ["ax": "\(P).mark.last", "need": "decision", "action": "cancel", "result": "cancel-requested"]
            AISetupDoors.cancel(.decision); return true
        }
        for id in ["anthropic", "openai", "gemini", "openrouter"] {
            t["\(P).paste-key.\(id)"] = { _ in guard CoreHost.shared.kind == .fixture else { return false }; step.saveKey(provider: id, fixtureKey); return true }
        }
        t["\(P).paste-key.jev"] = { _ in guard CoreHost.shared.kind == .fixture else { return false }; step.useJev(key: fixtureKey, consent: false); return true }
        return t
    }
}
