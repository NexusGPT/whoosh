import Foundation

/// One feature, as the router sees it: who it is, whether it holds the keys, and what its own text port says to a string.
struct DictationTarget {
    let id: String
    let focused: Bool
    let take: @MainActor (String) -> Bool
}

/// Where finished text goes: the field that holds the keys, through the shell's focused-field text port (`FeatureModule.typeText`,
/// the same door as the socket's `type`), asked in the shell's focus order.
///  - A field that owns its text (the quick reply's draft) takes it by `typeText`.
///  - The composer observes `Notification.Name.dictationText` and inserts at its caret (it has no `typeText` yet: a finding in the lane note).
///  - The search, the palette and the labels names keep their text in the shared buffer `AppModel.typed` while they hold the keys; they take
///    nothing by `typeText`, so a focused one of them gets the text appended to that buffer, exactly as `type` does.
/// Nothing holds the keys: nothing is typed and the answer is false, so the line can say so.
@MainActor
enum DictationInsert {
    static let composerId = "composermac"
    static let sharedBufferFields: Set<String> = ["searchmac", "palettemac", "labelscontactsmac"]

    /// The routing rule, with no app in it.
    static func route(_ text: String, _ targets: [DictationTarget], composer: (String) -> Void, typed: (String) -> Void) -> Bool {
        for t in targets {
            if t.id == composerId, t.focused { composer(text); return true }
            if t.take(text) { return true }
            if t.focused, sharedBufferFields.contains(t.id) { typed(text); return true }
        }
        return false
    }

    @discardableResult
    static func deliver(_ text: String, _ m: AppModel) -> Bool {
        let targets = Registrations.byFocus(m).map { f in DictationTarget(id: f.id, focused: f.hasFocus(m), take: { f.typeText($0, m) }) }
        return route(text, targets,
                     composer: { NotificationCenter.default.post(name: .dictationText, object: nil, userInfo: ["text": $0]) },
                     typed: { m.typed += $0 })
    }
}
