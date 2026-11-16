import Foundation

/// The list's click ids: the controls the drive reaches by id (docs/CONTROL-SOCKET.md). Every id has a control on screen
/// except the pin door, which stands in for the sidebar's pin (reported in docs/lanes/inboxlist.md).
@MainActor
enum ListClicks {
    static var state: InboxListState { .shared }

    static func targets() -> [String: (AppModel) -> Bool] {
        var out: [String: (AppModel) -> Bool] = [
            "mac-main.list.mark-all-read": TriageActions.markAllRead,
            "mac-main.list.compact": { _ in state.compact.toggle(); return true },
            // the Settings > General toggle's key; also a palette row, so the owner can flip it from the list
            "mac-main.list.emblems": { _ in ListSettings.showEmblems.toggle(); return true },
            "mac-main.list.perf.start": { _ in ListPerf.start(); return true },
            "mac-main.list.perf.stop": { _ in ListPerf.stop(); return true },
            "mac-main.list.command": { m in m.press(LiveKeys.search) },
            "mac-main.list.filter": { m in guard state.filter != nil else { return false }; ListFilters.clear(m); return true },
            "mac-main.list.selectionbar.clear": { m in guard !state.selection.isEmpty else { return false }; state.clearSelection(); return true },
            // the undo capsule's Undo button (ListChrome.undoCapsule): drawn only while the last action has an undo entry and no selection bar covers the foot
            "mac-main.list.undo": { m in
                guard state.undoAvailable, state.undoSaid != nil, state.selection.isEmpty else { return false }
                return m.press("Z")
            },
            // the label chip's cross (LabelFilterChip): the same select the button makes; no chip, no door
            "mac-main.list.labelchip": { m in
                guard state.labelFilter != nil, !m.rows.isEmpty else { return false }   // an empty label draws no chip (the empty words), so no control
                _ = SidebarState.shared.select("mailbox.all-inboxes")
                return true
            },
        ]
        for g in InboxListState.Grouping.allCases {
            out["mac-main.list.group.\(g.rawValue)"] = { _ in state.grouping = g; return true }
        }
        for a in RowStrip.items {
            // the bar's buttons: only while a selection is open, like the bar
            out["mac-main.list.selectionbar.\(a.id)"] = { m in !state.selection.isEmpty && TrainerFeature.mouseUsed(a.id, m) }
        }
        for i in 0..<InboxListFeature.clickableRows {
            out["mac-main.list.row.\(i)"] = { m in
                guard m.rows.indices.contains(i) else { return false }
                m.focusIndex = i
                return true
            }
            // the double click on a row (InboxListView.row): focus it, then Enter, the same press the gesture makes
            out["mac-main.list.row.\(i).open"] = { m in
                guard m.rows.indices.contains(i) else { return false }
                m.focusIndex = i
                return m.press("Enter")
            }
            out["mac-main.list.row.\(i).create-rule"] = { m in RuleFromMessage.run(m, row: i) }   // J5.9: the row menu's "Create a rule from this message"
            out["mac-main.list.row.\(i).why"] = { m in
                guard m.rows.indices.contains(i) else { return false }
                let id = m.rows[i].messageId
                state.whyOpen = state.whyOpen == id ? nil : id
                return true
            }
            // the row strip's buttons: focus the row, then the same command the key runs
            for a in RowStrip.items {
                out["mac-main.list.row.\(i).act.\(a.id)"] = { m in
                    guard m.rows.indices.contains(i) else { return false }
                    if !state.selection.isEmpty, !state.selection.contains(m.rows[i].messageId) { state.clearSelection() }
                    m.focusIndex = i
                    return TrainerFeature.mouseUsed(a.id, m)
                }
            }
        }
        let strip = PinnedTabs.all(AppModel.shared)
        let folded = Set(PinnedTabs.split(strip).folded.map(\.id))
        for (i, tab) in strip.enumerated() {
            // a tab, and each tab behind the "+N" chip: the same move the chip's menu makes
            out["mac-main.splits.\(i)"] = { m in goTo(tab.id, m) }
            if folded.contains(tab.id) { out["mac-main.splits.more.\(tab.id)"] = { m in goTo(tab.id, m) } }
            // the tab's context menu: unpin through the label record; a view has no menu, so no door
            if tab.menu.contains("unpin") { out["mac-main.splits.unpin.\(tab.id)"] = { m in PinnedTabs.setPinned(tab.id, false, m) } }
        }
        // the Unread count in the count line (UnreadCount): the same toggle as ⇧U
        out["mac-main.splits.unread"] = { m in ListFilters.toggle(.unread, m) }
        // the pin door: the sidebar's pin does the same command; this id lets the drive pin a label the tab row does not show
        for l in AppModel.shared.core.labels() where !l.pinned {
            out["mac-main.splits.pin.\(l.id)"] = { m in PinnedTabs.setPinned(l.id, true, m) }
        }
        return out
    }

    private static func goTo(_ id: String, _ m: AppModel) -> Bool { PinnedTabs.open(id, m) }
}
