import SwiftUI
#if os(macOS)
import AppKit
#endif

/// The searchmac lane's registration: the search screen's rows, keys, clicks and the commands it takes over
/// from the shortcut engine's stand-ins ("search" opens the screen; Enter, Down, Up act on its results).
@MainActor
public enum SearchMacFeature: FeatureModule, CommandProviding {
    public static let id = "searchmac"
    public static let surfaces: [String] = ["mac-search"]
    static var s: SearchMacModel { .shared }
    static func inSearch(_ m: AppModel) -> Bool { m.surface == slot.id }
    /// The surface word of this screen. It has no window and claims no pane: its field, dropdown and results are drawn by the main window's list pane.
    static let slot = SurfaceSlot("mac-search")

    /// Focus is the search itself, and the thread it opened until Escape brings the results back.
    public static func hasFocus(_ m: AppModel) -> Bool {
        inSearch(m) || comingBack(m) || s.releasedWithText(m)
    }

    /// The thread this screen opened is still the one in the reader: Escape brings the results back.
    static func comingBack(_ m: AppModel) -> Bool {
        // the mail the reader shows is the selected result's own row (AppModel.shownMessageId); the list's focused row is another mail when the result
        // is outside the inbox's rows or the list's focus moved (main100: J5.6's Esc after the other journeys stayed on the reader)
        s.returnToSearch && m.surface == "mac-reader" && (m.shownMessageId == s.openedMessageId || m.focusedMessageId == s.openedMessageId)
    }

    public static func commands() -> [String: (AppModel) -> Bool] {
        [
            "search": { _ in s.open() },
            // routed by focus: outside the search these return false and the next provider answers
            "open": { m in inSearch(m) ? s.enter() : false },
            // On the results: the selection moves. In the reader, on a result this screen opened: the neighbour result opens in its place (the
            // preset's J and K inside a conversation), and at either end the key stays here: it never moves the inbox's own focus under the search
            // (lead, 13 Nov, owner A162).
            "next": { m in if inSearch(m) { return s.move(1) }; if comingBack(m) { _ = s.openNeighbour(1); return true }; return false },
            "previous": { m in if inSearch(m) { return s.move(-1) }; if comingBack(m) { _ = s.openNeighbour(-1); return true }; return false },
            // J5.6: L with every result selected opens the list's label picker on all of them; with none selected the list's own L acts on the shown result
            "label": { m in inSearch(m) && !s.picked.isEmpty && s.labelPickedKey() },
        ]
    }

    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        [
            "⌘A": { m in inSearch(m) && s.selectAllKey() },   // the field's words while the caret is in it; every result after Esc from one (J5.6)
            "Tab": { m in inSearch(m) && s.tab() },
            // The keys are on the results (after Return, SearchFocus.toResults): Return opens the selected one and the arrows move, here and not
            // through another screen's binding (the reader's Return is "open and reply"). With the cursor in the field the field's own handlers
            // take these three first (SearchField), so nothing runs twice.
            "Enter": { m in inSearch(m) && !s.holds && s.enter() },
            "Down": { m in inSearch(m) && !s.holds && s.move(1) },
            "Up": { m in inSearch(m) && !s.holds && s.move(-1) },
            "Escape": { m in
                if inSearch(m) { return s.close() }
                if comingBack(m) { return s.open(returning: true) }
                if s.releasedWithText(m) { return s.close() }   // the field let go but still shows its text: Esc clears it
                return false
            },
        ]
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        var t: [String: (AppModel) -> Bool] = [
            // a click on the field takes the keys; on the closed search it opens it first, as a click on the list header's field does (the pill becomes the
            // live field in place, ListHeader.swift CommandField; mac-search.md links-in "the search field (click)"). main92: the click did nothing after
            // Esc had closed the search, the typed words went to the list and Enter opened a reply (par-searchmac J2.4 on)
            "mac-search.bar": { _ in (s.active || s.open()) && { s.claim(); s.caret = true; s.picked = []; s.focusTick += 1; return true }() },   // a click on the field: it takes the keys (and its text back)
            "mac-search.open": { _ in s.open() },
            "mac-search.save-as-smart-folder": { _ in s.startSaving() },
            "mac-search.save-as-smart-folder.confirm": { m in inSearch(m) && s.naming && s.confirmName() },
            "mac-search.bar.clear": { _ in s.active && { s.clearField(); s.claim(); s.focusTick += 1; return true }() },
            // the dropdown's rows: a click runs the row as Enter does; `.tab` adds it as a chip as Tab does
            // the skeleton hold is for the shot verb: it keeps the skeleton rows on screen until released (a key never sets it)
            // rig door: the list's own rows change under an open field, as a sync does. The last row, the one focused, is archived in the core:
            // AppModel.refresh then clamps `focusIndex` (AppModel.swift:76) with no pointer and no key near the list. False unless the focused row is the last.
            "mac-search.rig.archive-focused-row": { m in
                guard inSearch(m), m.rows.count > 1, m.focusIndex == m.rows.count - 1 else { return false }
                return m.onFocused { .archive(messageIds: [$0]) }
            },
            // the closed line "All other mail · N" (SPEC A150): a click opens or closes it, as Enter does while it is selected
            "mac-search.more": { m in inSearch(m) && s.toggleRest() },
            "mac-search.skeleton.hold": { _ in s.holdSkeleton = true; return true },
            "mac-search.skeleton.release": { _ in s.holdSkeleton = false; if s.loading { s.settle() }; return true },
        ]
        for i in s.suggestions.indices {
            t["mac-search.dropdown.\(i)"] = { m in guard inSearch(m) else { return false }; s.choose(i); return s.enter() }
            t["mac-search.dropdown.\(i).tab"] = { m in guard inSearch(m) else { return false }; s.choose(i); return s.tab() }
            // the pointer resting on a row (SearchDropdown `onHover`): the row takes its own tint (`hovered`); the keyboard highlight and Return stay where they were
            t["mac-search.dropdown.\(i).hover"] = { m in guard inSearch(m), s.dropdownVisible else { return false }; s.hovered = i; return true }
        }
        for i in s.query.chips.indices { t["mac-search.chips.\(i).remove"] = { _ in s.removeChip(i) } }
        for (i, f) in s.folders.enumerated() {
            let edit: (AppModel) -> Bool = { _ in s.editFolder(i) }
            for id in ["mac-search.sidebar.smart.\(i).edit", "mac-search.sidebar.smart-folders.\(i).edit",
                       "mac-search.sidebar.smart-folders.\(LabelsContactsSource.slug(f.name)).edit"] { t[id] = edit }
        }
        for i in s.recents.indices { t["mac-search.recent.\(i)"] = { _ in s.useRecent(i) } }
        for i in s.results.indices { t["mac-search.results.\(i)"] = { _ in s.openResult(i) } }
        return t
    }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] { SearchMacRows.all }

    public static func paletteCommands() -> [(title: String, key: String)] { [(title: Words.text("Search mail"), key: "/")] }
}
