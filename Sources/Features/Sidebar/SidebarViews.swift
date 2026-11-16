import Foundation

/// Round 11 (docs/PRD.md "A91 shape", owner card 2026-11-09 16:30: "Main is primary, so keep primary, just don't make it a Label
/// because it is confusing ... 'All' (again not a label)"). Primary and All are VIEWS of the inbox (contract delta 16): the Labels
/// section holds labels only, and the two views are reached by the Go-to keys. This file holds what the sidebar does for a view:
/// open it, refuse in words anything that would treat it as a label, and answer the socket.
enum ViewRefusal {
    /// The contract's `VIEW_REFUSAL_CODE`: the `field` of a refusal that says "a view is not a label".
    static let field = "view_not_a_label"

    /// The contract's own sentence when `command` would treat a view as a label; nil when the command passes. The same pure door
    /// both cores call first, so the sentence is the one the core would give.
    static func words(for command: Command) -> String? {
        do { try checkViewCommand(command: command); return nil }
        catch CoreError.Invalid(let f, let words) where f == field { return words }
        catch { return nil }
    }

    /// The sentence of a thrown core error when it is this refusal.
    static func words(from error: Error) -> String? {
        if case CoreError.Invalid(let f, let words) = error, f == field { return words }
        return nil
    }
}

extension SidebarState {
    /// The Go-to commands that open a VIEW of the inbox, with the view each opens. `go-inbox` is G then I (Superhuman's map).
    /// `go-all` is the All view's own command: it has no row in the v8 key list, so the shortcuts lane binds its key
    /// (docs/lanes/sidebar.md "round 11"); until then the command answers the socket and the palette.
    static let goViews: [(command: String, view: InboxView)] = [("go-inbox", .primary), ("go-all", .all)]

    /// Opens a view of the inbox: the inbox with that tab, no account filter, "All inboxes" highlighted. The tab is written before
    /// the highlight moves so the refresh a mailbox change makes already reads it. A view the owner has no row for (All inboxes hidden)
    /// still opens: the keys must not depend on a hidden row.
    @discardableResult func openView(_ view: InboxView, model: AppModel) -> Bool {
        let tab = inboxViewId(view: view)
        if model.tab != tab { model.tab = tab; model.focusIndex = 0 }
        if !select("mailbox.all-inboxes") { model.mailbox = nil; model.smartFolderId = nil }
        clearAccounts(model: model)   // the filter goes, and the list reads the tab (refresh)
        return true
    }

    /// One door for a label command from the sidebar: a command that would treat a view as a label is refused here in the
    /// contract's own words (what the list's failure line then shows), before the core is asked; everything else is the core's.
    @discardableResult func perform(_ command: Command, model: AppModel) -> Bool {
        if let words = ViewRefusal.words(for: command) {
            refusal = SidebarRefusal(field: ViewRefusal.field, words: words)
            model.lastFailure = ActionFailed(accountName: "Whoosh", reasonWords: words, retryable: false)
            return false
        }
        refusal = nil
        return model.perform(command)
    }
}

/// The last refusal the sidebar showed in words (read by the socket row `mac-main.sidebar.refusal`).
struct SidebarRefusal: Equatable {
    let field: String
    let words: String
}

extension SidebarFeature {
    /// The key a Go-to command shows: its sequence in the preset in force ("G then I"), nil while the preset binds none.
    static func goKey(_ command: String) -> String? {
        ShortcutEngine.shared.preset.sequences.first { $0.command == command }?.label
    }

    static func viewRows(_ st: SidebarState) -> [String: (AppModel) throws -> [ControlRow]] {
        [
            /// The two views and how to reach them: the Go-to command, the key it shows and the tab it opens.
            "mac-main.sidebar.go-views": { m in
                SidebarState.goViews.map { g in
                    ["ax": "mac-main.sidebar.command.\(g.command)", "command": g.command, "view": inboxViewId(view: g.view),
                     "name": Words.text(inboxViewName(view: g.view)), "key": goKey(g.command) ?? "", "open": m.tab == inboxViewId(view: g.view) && m.mailbox == nil]
                }
            },
            /// The last refusal in words; no row while there is none.
            "mac-main.sidebar.refusal": { _ in
                st.refusal.map { [["field": $0.field, "words": $0.words]] } ?? []
            },
        ]
    }

    static func viewClicks(_ st: SidebarState) -> [String: (AppModel) -> Bool] {
        var t: [String: (AppModel) -> Bool] = [:]
        for g in SidebarState.goViews { t["mac-main.sidebar.command.\(g.command)"] = { m in CommandRegistry.handler(g.command)?(m) ?? false } }
        /// The drive's way to ask the core for what the sidebar never offers: pin a view. The door refuses in words and nothing changes.
        for v in inboxViews() {
            t["mac-main.sidebar.ask.pin-view.\(inboxViewId(view: v))"] = { m in st.perform(.pinLabel(labelId: inboxViewId(view: v), pinned: true), model: m) }
        }
        return t
    }
}
