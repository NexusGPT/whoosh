import SwiftUI

/// What the socket reads and clicks (`rows mac-first-start.*`, `click mac-first-start.*`; docs/CONTROL-SOCKET.md). A click returns
/// false when the control is not on screen or is off, as for a person: the socket answers "did nothing".
@MainActor
enum FirstStartRows {
    static let X = FirstStartFeature.X
    static var m: FirstStartModel { .shared }

    struct Control { let id: String; let title: String; let key: String; let enabled: Bool }

    static func controls() -> [Control] {
        guard m.active else { return [] }
        if m.skipped { return [Control(id: "add-account", title: FirstStartWords.addAccount, key: "↩", enabled: true)] }
        var out: [Control] = []
        if m.step > 1 && m.step <= FirstStartModel.importStep { out.append(Control(id: "back", title: FirstStartWords.back, key: "⌘[", enabled: true)) }
        out.append(Control(id: "skip", title: FirstStartWords.skip, key: "esc", enabled: true))
        let goes = m.step == FirstStartModel.importStep ? FirstStartWords.finish : FirstStartWords.continueWord
        out.append(Control(id: "continue", title: goes, key: "↩", enabled: m.step != FirstStartModel.addStep || m.accountCount > 0))
        return out
    }

    static func sources() -> [String: (AppModel) throws -> [ControlRow]] {
        var t: [String: (AppModel) throws -> [ControlRow]] = [
            "\(X).state": { _ in state() },
            "\(X).card": { _ in card() },
            "\(X).controls": { _ in controls().map { ["ax": "\(X).\($0.id)", "id": $0.id, "title": $0.title, "key": $0.key, "enabled": $0.enabled] } },
            "\(X).dots": { _ in dots() },
            "\(X).lit": { _ in lit() },
            "\(X).keys": { _ in keysRows() },
            "\(X).search": { _ in search() },
            "\(X).launch": { _ in launch() },
            "\(X).geometry": { _ in geometry() },
            "\(X).replies": { _ in FirstStartRepliesRows.rows() },
            "\(X).ai": { _ in FirstStartAIRows.rows() },
        ]
        // the AI setup rows (status, places, marks, step) are served with this feature: one registry, the same in the first-start flow and in the main window
        t.merge(AISetupRows.rowSources()) { a, _ in a }
        return t
    }

    static func clicks() -> [String: (AppModel) -> Bool] {
        var t: [String: (AppModel) -> Bool] = [
            "\(X).continue": { _ in m.next() },
            "\(X).back": { _ in m.back() },
            "\(X).skip": { _ in m.skip() },
            "\(X).add-account": { _ in m.addAccountFromEmptyInbox() },
            "\(X).relaunch": { _ in m.relaunch() || true },   // the launch rule again; the answer is in `rows mac-first-start.launch`
            "mac-palette.result.show-the-quick-start": { _ in m.replay() },   // the palette's line (FirstStartFeature.paletteCommands)
        ]
        t.merge(FirstStartRepliesRows.clicks()) { a, _ in a }
        t.merge(FirstStartAIRows.clicks()) { a, _ in a }
        t.merge(AISetupRows.clickTargets()) { a, _ in a }
        return t
    }

    static func state() -> [ControlRow] {
        [["ax": "\(X).state", "active": m.active, "visible": m.visible, "step": m.step, "steps": FirstStartModel.stepCount, "skipped": m.skipped,
          "lifted": m.lifted, "finished": m.finished, "accounts": m.accountCount, "frozen": m.frozen, "reduce_motion": m.rigReduceMotion,
          "loop_s": FirstStartLook.loopS, "morph_ms": WhooshConstants.MOTION_MAX_MS, "veil_ms": WhooshConstants.MOTION_MAX_MS,
          "ring_ms": WhooshConstants.LAND_AURORA_MS, "entrance_ms": FirstStartLook.welcomeEntranceMs, "ring_showing": m.ringShowing]]
    }

    static func card() -> [ControlRow] {
        guard m.active, !m.skipped else { return [] }
        if m.step == FirstStartModel.aiStep { return [FirstStartAIRows.card()] }
        if m.step == FirstStartModel.repliesStep { return [FirstStartRepliesRows.card()] }
        if m.step > 5 {   // steps 6 and 8: the sheet is the step (the add-account card, the import panel); the flow only owns the count and the controls
            return [["ax": "\(X).card", "step": m.step, "sheet": m.step == FirstStartModel.addStep ? "add-account" : "import", "count": FirstStartWords.countOf(m.step),
                     "dots": FirstStartModel.stepCount, "dot_current": m.step, "back_enabled": true]]
        }
        let t = FirstStartCardText.of(m.step)
        var row: ControlRow = ["ax": "\(X).card", "step": m.step, "kicker": t.kicker, "title": t.title, "body": t.body, "count": FirstStartWords.stepOf(m.step),
                               "dots": FirstStartModel.stepCount, "dot_current": m.step, "back_enabled": m.step > 1]
        if m.step == 5 {   // the two optional parts, each with its switch off, and the line about Microsoft and Google (2b `.orow`, `.never`)
            row["optional"] = [["name": FirstStartWords.jevKey, "on": false], ["name": FirstStartWords.pushRelay, "on": false]]
            row["never"] = FirstStartWords.nothingSent
        }
        return [row]
    }

    static func dots() -> [ControlRow] {
        guard m.active, !m.skipped, m.step <= FirstStartModel.importStep else { return [] }
        return (1...FirstStartModel.stepCount).map { ["ax": "\(X).dots.\($0)", "n": $0, "current": $0 == m.step] }
    }

    static func lit() -> [ControlRow] {
        let on = FirstStartGeometry.lit(step: m.step, lifted: m.lifted || !m.active)
        return FirstStartGeometry.paneNames.map { ["ax": "\(X).lit.\($0)", "pane": $0, "lit": on.contains($0) || !m.visible] }
    }

    static func keysRows() -> [ControlRow] {
        guard m.active, m.step == 3 else { return [] }
        let k = m.keys
        return [["ax": "\(X).keys", "rows": k.rows.compactMap { FirstStartSample.line($0)?.sender }, "focus": k.focus, "started": k.started,
                 "last": k.last, "last_row": FirstStartSample.line(k.lastRow)?.sender ?? "", "caps": ["E", "H", "C", "/", "J", "K"]]]
    }

    static func search() -> [ControlRow] {
        guard m.active, m.step == 2 else { return [] }
        let hit = FirstStartSample.match(m.tryText)
        return [["ax": "\(X).search", "typed": m.tryText, "match": hit?.sender ?? "", "subject": hit?.subject ?? "",
                 "why": hit.map { FirstStartWords.foundByMeaning(m.tryText, $0.sender) } ?? (m.tryText.count >= 2 ? FirstStartWords.noSample(m.tryText) : "")]]
    }

    static func launch() -> [ControlRow] {
        [["ax": "\(X).launch", "accounts": m.accountCount, "finished": m.finished,
          "would_show": FirstStartModel.shouldShow(accounts: m.accountCount, finished: m.finished), "flag": FirstStartModel.finishedKey]]
    }

    static func geometry() -> [ControlRow] {
        let g = FirstStartGeometry(size: m.windowSize)
        func r(_ c: CGRect) -> [String: Any] { ["x": Double(c.minX), "y": Double(c.minY), "w": Double(c.width), "h": Double(c.height)] }
        return [["ax": "\(X).geometry", "window": ["w": Double(g.size.width), "h": Double(g.size.height)], "card": r(g.card(step: m.step)),
                 "welcome_card": r(g.welcomeCard), "medallion": Double(g.medallion), "halo": Double(g.haloSize), "compact": g.compact]]
    }
}
