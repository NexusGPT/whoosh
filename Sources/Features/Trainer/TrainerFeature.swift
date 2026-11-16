import Foundation

/// The trainer lane's registrations: the hint after a mouse action, the keys overlay (the show-keys key), the
/// hints switch. Views are in this folder (TrainerHintView, TrainerOverlayView) for the shell to mount.
@MainActor
public enum TrainerFeature: FeatureModule {
    public static let id = "trainer"
    public static let surfaces: [String] = ["mac-trainer"]
    static var model: TrainerModel { .shared }

    /// The actions a list row offers with the mouse; each ends in the engine as a mouse use, then the hint.
    static let rowActions = ["archive", "snooze", "label", "pin"]

    /// The one call a screen makes when its button (not its key) ran an action. The inbox list calls it from the
    /// row's action buttons instead of `CommandRegistry.handler(action)`.
    @discardableResult
    public static func mouseUsed(_ command: String, _ m: AppModel) -> Bool { model.mouseUsed(command, m) }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        [
            "mac-trainer.hint": { m in
                guard let h = model.visibleHint(m) else { return [] }
                return [["ax": "mac-trainer.hint", "text": h.text, "key": h.chord, "action": h.verb, "command": h.command,
                         "key_uses": h.usesAtShow, "stop_after": TrainerModel.stopAfter]]
            },
            "mac-trainer.overlay.keys": { m in
                guard model.isOpen(m) else { return [] }
                return model.keyRows.enumerated().map { i, r in
                    ["ax": "mac-trainer.overlay.keys.\(i)", "key": r.chord, "action": r.action, "command": r.command, "used": r.used]
                }
            },
            "mac-trainer.settings": { _ in [["ax": "mac-trainer.settings", "hints": model.hintsOn ? "on" : "off"]] },
        ]
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        var t: [String: (AppModel) -> Bool] = [
            "mac-trainer.hint.dismiss": { _ in
                guard model.shown != nil else { return false }
                model.dismiss(); return true
            },
            "mac-trainer.overlay.close": { model.close($0) },
            "mac-trainer.hints.on": { _ in model.hintsOn = true; return true },
            "mac-trainer.hints.off": { _ in model.hintsOn = false; return true },
            // the two palette lines the trainer lists (paletteCommands below): a feature that owns a line with no key answers `mac-palette.result.<slug>` (PaletteMacCatalog)
            "mac-palette.result.turn-shortcut-hints-off": { _ in model.hintsOn = false; return true },
            "mac-palette.result.turn-shortcut-hints-on": { _ in model.hintsOn = true; return true },
        ]
        for i in 0..<AppModel.shared.rows.count {
            for a in rowActions {
                t["mac-main.list.row.\(i).\(a)"] = { m in
                    guard m.rows.indices.contains(i) else { return false }
                    m.focusIndex = i
                    return model.mouseUsed(a, m)
                }
            }
        }
        #if os(macOS)
        t["mac-trainer.stage.light"] = { _ in TrainerStage.shared.show(dark: false) }
        t["mac-trainer.stage.dark"] = { _ in TrainerStage.shared.show(dark: true) }
        t["mac-trainer.stage.wide"] = { _ in TrainerStage.shared.show(size: .wide) }
        t["mac-trainer.stage.narrow"] = { _ in TrainerStage.shared.show(size: .narrow) }
        #endif
        return t
    }

    public static func paletteCommands() -> [(title: String, key: String)] {
        // "Show all keys" is listed by ShortcutsFeature.paletteCommands() (its key follows the preset); listed here too it showed twice.
        [("Turn shortcut hints off", ""), ("Turn shortcut hints on", "")]
    }
}

/// The show-keys key arrives through the shortcuts lane's preset (CommandRegistry): this screen provides "keys".
extension TrainerFeature: CommandProviding {
    public static func commands() -> [String: (AppModel) -> Bool] {
        ["keys": { model.toggle($0) }]
    }
}
