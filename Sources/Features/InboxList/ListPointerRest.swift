import Foundation

/// The rig's pointer on a list row (board message-card-r1 state 7, "Hovered, action strip", on a row that is not the focused one). It rides the
/// notch's own verb (Sources/ControlSocket/FixtureVerbs.swift `fixture pointer-rest <ax-id>`), no second verb:
///   `fixture pointer-rest mac-main.list.row.<n>`  the pointer rests on the row at place n: the hover fill and the action strip, as after HOVER_REVEAL_MS
///   `fixture pointer-rest off`                    the pointer leaves the list (a rest on any other surface, the notch's included, leaves it too)
/// `fixture reset` takes the rest off as well (the reset's notification), so no state hands a hovered row to the next.
@MainActor
enum ListPointerRest {
    static let rowPrefix = "mac-main.list.row."
    private static var watchingReset = false

    enum Refusal: Error { case noRow(String) }

    /// Rests the pointer on the target. Returns the row's message id, or nil when the target is not a list row (the rest leaves the list).
    /// A target that names a list place holding no row is refused, never taken for "off".
    @discardableResult
    static func rest(_ m: AppModel, on target: String) throws -> String? {
        watchReset()
        var id: String?
        if target.hasPrefix(rowPrefix) {
            guard let n = Int(target.dropFirst(rowPrefix.count)), m.rows.indices.contains(n) else { throw Refusal.noRow(target) }
            id = m.rows[n].messageId
        }
        set(id)
        return id
    }

    private static func set(_ id: String?) {
        let state = InboxListState.shared
        if state.hovered != id { state.hovered = id }
        if state.rested != id { state.rested = id }
    }

    private static func watchReset() {
        guard !watchingReset else { return }
        watchingReset = true
        NotificationCenter.default.addObserver(forName: FixtureEvents.name("reset"), object: nil, queue: nil) { _ in
            MainActor.assumeIsolated { set(nil) }
        }
    }
}
