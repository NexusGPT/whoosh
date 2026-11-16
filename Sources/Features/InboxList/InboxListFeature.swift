import SwiftUI

/// The inbox list's registration (folder Sources/Features/InboxList/): its rows, keys, clicks and palette
/// commands. Keys the shortcuts engine knows reach `commands()` (CommandProviding); the rest bind here.
@MainActor
public enum InboxListFeature: FeatureModule, CommandProviding {
    public static let id = "inboxlist"
    public static let surfaces: [String] = ["mac-main"]
    static var state: InboxListState { .shared }
    /// click ids are registered per index ahead of time; a list longer than this is clicked by key instead
    static let clickableRows = 64

    /// The ids the shortcuts table names for this lane. Filters read the core (delta 10); mark-not-done unarchives (delta 11).
    public static func commands() -> [String: (AppModel) -> Bool] {
        [
            "archive": TriageActions.archive,
            "snooze": { TriageActions.openPicker($0, { .snooze(messageIds: $0) }, overlay: Overlay.snooze) },
            "label": { TriageActions.openPicker($0, { .label(messageIds: $0) }, overlay: Overlay.label) },
            "pin": TriageActions.pin,
            "undo": TriageActions.undo,
            "open": TriageActions.enter,
            "star": TriageKeys.star, "mute": TriageKeys.mute,
            "delete": TriageKeys.trash, "junk": TriageKeys.spam, "trash": TriageKeys.trash, "spam": TriageKeys.spam,
            // with a snooze or label picker up, the move keys move the picker's highlighted line, never the list under it
            "next": { m in TriageActions.live(m) != nil ? TriagePick.move(1, m) : ListFilters.move(m, 1) },
            "previous": { m in TriageActions.live(m) != nil ? TriagePick.move(-1, m) : ListFilters.move(m, -1) },
            "remove-label": { LabelKeys.remove($0, step: 0) }, "remove-label-next": { LabelKeys.remove($0, step: 1) },
            "remove-label-previous": { LabelKeys.remove($0, step: -1) }, "remove-all-labels": { LabelKeys.remove($0, step: 0, all: true) },
            "next-split": { PinnedTabs.step($0, 1) }, "previous-split": { PinnedTabs.step($0, -1) },
            "select-conversation": SelectionActions.toggle,
            "extend-selection-down": { SelectionActions.extend($0, 1) }, "extend-selection-up": { SelectionActions.extend($0, -1) },
            "clear-selection": SelectionActions.clear, "mark-not-done": TriageKeys.markNotDone,
            "select-from-here": SelectionActions.fromHere, "select-all": SelectionActions.all,
            "filter-unread": { ListFilters.toggle(.unread, $0) }, "filter-starred": { ListFilters.toggle(.starred, $0) },
            "filter-important": { ListFilters.toggle(.important, $0) }, "filter-no-reply": { ListFilters.toggle(.noReply, $0) },
        ]
    }

    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        #if os(macOS)
        TriagePick.installOnce()
        #endif
        return [
            "X": SelectionActions.toggle,
            "⇧J": { SelectionActions.extend($0, 1) }, "Shift+J": { SelectionActions.extend($0, 1) },
            "⇧K": { SelectionActions.extend($0, -1) }, "Shift+K": { SelectionActions.extend($0, -1) },
            "U": TriageActions.toggleRead,
            "⇧E": TriageKeys.markNotDone, "Shift+E": TriageKeys.markNotDone,
            "S": TriageKeys.star, "#": TriageKeys.trash, "!": TriageKeys.spam,
            "⇧#": TriageKeys.trash, "⇧!": TriageKeys.spam,
            "⇧M": TriageKeys.mute, "Shift+M": TriageKeys.mute,
            "Escape": { m in
                // a thread overlay is never the list's on the Mac (search opens it): pass, so search gets it back
                if m.overlay == "mac-thread" { return false }
                // an open mail or a reader holding the keys: one step back to the list first (owner 2026-11-13); the selection is cleared by the NEXT Esc
                if state.pending == nil, MacShellState.shared.backToList(m) { return true }
                let picker = state.pending != nil
                TriageActions.close(m)
                // nothing laid over the list: Esc clears the selection, then the filter
                if !picker, m.overlay == nil, m.surface == "mac-main", SelectionActions.clear(m) { return true }
                state.clearSelection()
                m.overlay = nil
                m.surface = "mac-main"
                return true
            },
        ]
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        var t = ListClicks.targets()
        // the lines of the snooze and label pickers (H, L): each acts only while its picker is up
        for kind in ["laterToday", "tomorrowMorning", "nextWeek", "pickDate"] { t["mac-main.snooze.option.\(kind)"] = { m in TriagePick.clickSnooze(kind, m) } }
        for l in AppModel.shared.core.labels() where l.kind == .owner { t["mac-main.label.option.\(l.id)"] = { m in TriagePick.clickLabel(l.id, m) } }
        // the palette line of a feature with no key answers `mac-palette.result.<slug>` (PaletteCatalog.act): the focused row's rule
        t["mac-palette.result.\(PaletteCatalog.resultSlug(RuleFromMessage.title))"] = { RuleFromMessage.run($0) }
        return t
    }

    public static func paletteCommands() -> [(title: String, key: String)] {
        [(Words.text("Mark all as read"), ""), (Words.text("Select row"), LiveKeys.chord("select-conversation", "X")),
         (Words.text("Select all"), LiveKeys.chord("select-all", "⇧⌘A")),
         (Words.text("Show unread"), LiveKeys.chord("filter-unread", "⇧U")), (Words.text("Show starred"), LiveKeys.chord("filter-starred", "⇧S")),
         (Words.text("Show no reply"), LiveKeys.chord("filter-no-reply", "⇧R")),
         (Words.text("Group by sender"), ""), (Words.text("Group by domain"), ""), (Words.text("Compact rows"), ""),
         (Words.text("Show account emblems"), ""), (Words.text("Star"), LiveKeys.chord("star", "S")),
         (Words.text("Mute thread"), LiveKeys.chord("mute", "⇧M")), (Words.text("Move to trash"), LiveKeys.chord("delete", "⇧#")),
         (Words.text("Report spam"), LiveKeys.chord("junk", "⇧!")),
         (RuleFromMessage.title, "")]   // no key: the palette and the row's menu show it (J5.9)
    }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        ["mac-main.list": ListProbe.rows, "mac-main.snooze": ListProbe.snoozed, "mac-main.splits": ListProbe.splits, "mac-main.tabs": ListProbe.tabs,
         "mac-main.list.selection": { _ in state.selection.map { ["id": $0] } },
         "mac-main.list.state": { m in [ListProbe.listState(m)] }, "mac-main.list.perf": { _ in [ListPerf.row()] },
         "mac-main.picker": TriagePick.rows]
    }

    /// The shell's list pane (docs/lanes/appshell.md "## Slot contract").
    public static func view(for slot: SurfaceSlot) -> AnyView? {
        slot == .macList ? AnyView(InboxListView()) : nil
    }

    /// The zero state's words, or nil while the split has rows; the socket's `uistate.empty_state` reads it
    /// through `Registrations.emptyState`.
    public static func emptyState(_ m: AppModel) -> [String: Any]? {
        EmptyStateWords.make(m).map { ["title": $0.title, "line": $0.line, "key_hint": $0.keyHint] }
    }
}
