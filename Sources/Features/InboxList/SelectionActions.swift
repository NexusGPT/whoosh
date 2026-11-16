import Foundation

/// Selection (Superhuman K2): X toggles the focused row, ⇧J and ⇧K extend down and up, Esc clears, ⌘A selects from
/// the focused row down, ⌘⇧A selects every row. Under a filter these act on the rows the filter keeps.
@MainActor
enum SelectionActions {
    static var state: InboxListState { .shared }

    /// X: the focused row joins or leaves the selection.
    static func toggle(_ m: AppModel) -> Bool {
        guard let id = m.focusedMessageId else { return false }
        state.toggle(id)
        return true
    }

    /// ⇧J / ⇧K: the selection grows by the focused row and the next one the list shows.
    static func extend(_ m: AppModel, _ delta: Int) -> Bool {
        guard let id = m.focusedMessageId else { return false }
        state.add(id)
        guard ListFilters.move(m, delta) || m.moveFocus(delta), let next = m.focusedMessageId else { return false }
        state.add(next)
        return true
    }

    /// ⌘A: from the focused row to the end of the list.
    static func fromHere(_ m: AppModel) -> Bool {
        let rows = ListFilters.shown(m).filter { $0.0 >= m.focusIndex }
        guard !rows.isEmpty else { return false }
        state.select(rows.map(\.1.messageId))
        return true
    }

    /// ⌘⇧A: every row.
    static func all(_ m: AppModel) -> Bool {
        let rows = ListFilters.shown(m)
        guard !rows.isEmpty else { return false }
        state.select(rows.map(\.1.messageId))
        return true
    }

    /// Esc (and the clear-selection command): the selection first, then the filter. False when neither is open.
    static func clear(_ m: AppModel) -> Bool {
        // the list's own pickers (H, L) close WITH their action: since they draw a view, the line below alone closed the view and left the action
        // (lead, 13 Nov 2026, key trace on main156)
        if state.pending != nil { TriageActions.close(m); return true }
        // a picker another feature mounts over the list (V's move picker) closes like the list's own pickers
        if m.overlay != nil, ListOverlays.view(m) != nil { m.overlay = nil; return true }
        if !state.selection.isEmpty { state.clearSelection(); return true }
        if state.filter != nil { ListFilters.clear(m); return true }
        return false
    }
}
