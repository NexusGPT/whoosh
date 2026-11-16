import Foundation

/// The shortcuts lane's registration: one key closure per known chord, one click target per command, the
/// preset and usage rows, and the palette's command list. Presets: Superhuman (default), Gmail, Apple Mail,
/// GotEmail, your own. Everything dispatches through ShortcutEngine and CommandRegistry.
@MainActor
public enum ShortcutsFeature: FeatureModule {
    public static let id = "shortcuts"
    public static let surfaces = ["mac-shortcuts"]

    private static var engine: ShortcutEngine { .shared }

    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        var out: [String: (AppModel) -> Bool] = [:]
        for chord in engine.knownChords.union(Presets.pressable) {
            out[chord] = { m in
                if ShortcutEngine.shared.press(chord, m) { return true }
                // a screen that wrote the same key with its modifiers the other way round still gets it
                for alt in ChordSpelling.alternates(of: chord) { if Registrations.key(alt)?(m) == true { return true } }
                return false
            }
        }
        return out
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        var out: [String: (AppModel) -> Bool] = [:]
        for c in Commands.all {
            let id = c.id
            out["mac-shortcuts.action.\(id)"] = { m in ShortcutEngine.shared.click(id, m) }
        }
        // The palette line "Show all keys" (below) carries no key in a preset without a show-keys key; the palette then
        // answers a click on its slug, and the line opens the key list, never the search.
        out["mac-palette.result.show-all-keys"] = { m in ShortcutEngine.shared.click("keys", m) }
        for p in engine.allPresets {
            let id = p.id
            out["mac-shortcuts.preset.\(id)"] = { _ in ShortcutEngine.shared.setPreset(id) }
        }
        return out
    }

    public static func paletteCommands() -> [(title: String, key: String)] {
        Commands.all.filter(\.inPalette).map { (title: $0.title, key: engine.chord(for: $0.id) ?? "") }
            + [(title: "Show all keys", key: engine.chord(for: "keys") ?? "")]   // listed before Trainer's line of the same title, which hard-coded ⇧? (the search in Superhuman)
    }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        var out: [String: (AppModel) throws -> [ControlRow]] = [
            "mac-shortcuts.preset": { _ in bindingRows(ShortcutEngine.shared.preset) },
            "mac-shortcuts.usage": { _ in usageRows() },
            "mac-shortcuts.audit": { _ in auditRows() },
            "mac-shortcuts.map": { _ in mapRows(ShortcutEngine.shared.preset) },
            "mac-shortcuts.dispatch": { _ in dispatchRows() },
            "mac-shortcuts.notice": { _ in noticeRows() },
        ]
        for p in engine.allPresets {
            let id = p.id
            out["mac-shortcuts.preset.\(id)"] = { _ in
                guard let p = ShortcutEngine.shared.named(id) else { return [] }
                return [summaryRow(p)] + bindingRows(p)
            }
            out["mac-shortcuts.map.\(id)"] = { _ in ShortcutEngine.shared.named(id).map(mapRows) ?? [] }
        }
        return out
    }

    private static func bindingRows(_ p: KeyPreset) -> [ControlRow] {
        p.bindings.enumerated().map { i, b in
            ["ax": "mac-shortcuts.preset.\(p.id).\(i)", "key": b.chord, "action": Commands.title(b.command) ?? b.command, "command": b.command]
        }
    }

    /// Every way into a command in the preset: its row, a second key, a context meaning, a sequence. `status` is
    /// live, stub (`wired_by` names the lane that owes the handler) or coming-soon (`feature` names what is missing).
    private static func mapRows(_ p: KeyPreset) -> [ControlRow] {
        p.ways.enumerated().map { i, e in
            let c = Commands.command(e.command)
            let status = c?.comingSoon != nil ? "coming-soon" : (c?.wiredBy != nil ? "stub" : "live")
            return ["ax": "mac-shortcuts.map.\(p.id).\(i)", "key": e.key, "kind": e.kind, "command": e.command, "action": c?.title ?? e.command,
                    "status": status, "wired_by": c?.wiredBy ?? "", "feature": c?.comingSoon ?? ""]
        }
    }

    /// The last keys the engine saw, oldest first: what each reached and what came of it.
    private static func dispatchRows() -> [ControlRow] {
        ShortcutEngine.shared.log.enumerated().map { i, d in
            ["ax": "mac-shortcuts.dispatch.\(i)", "n": d.n, "key": d.chord, "command": d.command, "resolved": d.resolved, "outcome": d.outcome]
        }
    }

    /// The one-line notice a coming-soon key left (none when the last key left none).
    private static func noticeRows() -> [ControlRow] {
        ShortcutEngine.shared.notice.map { [["ax": "mac-shortcuts.notice.0", "text": $0]] } ?? []
    }

    /// Row 0 of a preset region: the keys other screens ask about by name.
    private static func summaryRow(_ p: KeyPreset) -> ControlRow {
        ["ax": "mac-shortcuts.preset.\(p.id).summary", "preset": p.id, "name": p.name, "current": p.id == ShortcutEngine.shared.presetId,
         "palette": p.chord(for: "palette") ?? "", "search": p.map["⌘K"] == "search" ? "⌘K" : (p.chord(for: "search") ?? ""), "keys": p.chord(for: "keys") ?? "",
         "archive": p.chord(for: "archive") ?? ""]
    }

    private static func usageRows() -> [ControlRow] {
        let u = ShortcutEngine.shared.store.usage()
        return Commands.all.map { c in
            ["ax": "mac-shortcuts.usage.\(c.id)", "action": c.id, "mouse_uses": u[c.id]?.mouse ?? 0, "key_uses": u[c.id]?.key ?? 0]
        }
    }

    /// Per preset: duplicates and unregistered commands. Both must be 0 for every preset.
    private static func auditRows() -> [ControlRow] {
        ShortcutEngine.shared.allPresets.map { p in
            ["ax": "mac-shortcuts.audit.\(p.id)", "preset": p.id, "keys": p.bindings.count,
             "duplicates": p.duplicates.count, "unregistered": p.unregistered.count,
             "no_handler": Set((p.bindings + p.alsoKeys + p.contextKeys).map(\.command) + p.sequences.map(\.command)).filter { CommandRegistry.handler($0) == nil }.count]
        }
    }
}
