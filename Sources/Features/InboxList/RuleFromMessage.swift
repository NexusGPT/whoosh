import Foundation

/// J5.9 "Create a rule from this message": Settings > Decisions on the Which tab page, a new rule first in its list, its condition the sender of the
/// row, the rule editor on it (`AISettingsStore.ruleDraft`, which the row `mac-settings-ai.decisions.rule-editor` reads). Reached from the row's context
/// menu, the palette line of the same words and the door `mac-main.list.row.<n>.create-rule`; no key (the owner: no complex shortcuts), so the menu and the palette show it.
@MainActor
enum RuleFromMessage {
    static let page = "which-tab"
    static var title: String { Words.text("Create a rule from this message") }

    /// The focused row's sender becomes the rule. False when no row is focused or the mail names no sender address.
    static func run(_ m: AppModel) -> Bool {
        guard let row = m.shownRow else { return false }
        let address = row.senderAddress.trimmingCharacters(in: .whitespaces)
        guard !address.isEmpty else { return false }
        let store = AISettingsStore.shared
        AISetupDoors.openSettings(.decisions)
        store.section = .page(page)
        store.addRule(page: page)
        guard let draft = store.ruleDraft else { return false }
        store.update(page: draft.page, entry: draft.entry) { $0.text = "from " + address; $0.match = "from:" + address }
        return true
    }

    /// The row at `index` is focused first, as every row control does.
    static func run(_ m: AppModel, row index: Int) -> Bool {
        guard m.rows.indices.contains(index) else { return false }
        m.focusIndex = index
        return run(m)
    }
}
