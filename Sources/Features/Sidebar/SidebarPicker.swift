import SwiftUI

/// What the list lane's title band reads (docs/lanes/sidebar.md "## Round 6"): the title of what the sidebar has selected, the account
/// scope, and the items the hidden sidebar's picker lists, the same ones the sidebar holds. Choosing an item does what a sidebar click does.
public struct SidebarPickItem: Identifiable, Equatable, Sendable {
    public let id: String
    /// "Mailboxes", "Smart folders" or "Labels": the picker's group captions.
    public let group: String
    public let title: String
    public let symbol: String
    public let count: Int?
    public let selected: Bool
    /// 0 at the top level; a label's depth in the tree.
    public let depth: Int
}

@MainActor
extension SidebarVisibility {
    /// The selected item's name ("All inboxes", "Snoozed", "Provanta RFP"): the title band's title in both states.
    public static func selectionTitle(model: AppModel = .shared) -> String {
        let id = SidebarState.shared.selection.id
        return pickItems(model: model).first { $0.id == id }?.title ?? Words.text("All inboxes")
    }

    /// The account the list is filtered to, or nil for every account: the band's scope words.
    public static func scopeName(model: AppModel = .shared) -> String? {
        model.accountFilter.flatMap { f in SidebarData.accounts(core: model.core).first { $0.id == f }?.account.displayName }
    }

    /// Mailboxes (Outbox first while a send waits), Smart folders, then every label the filter shows.
    public static func pickItems(model: AppModel = .shared) -> [SidebarPickItem] {
        let st = SidebarState.shared
        var out: [SidebarPickItem] = []
        let waiting = SidebarState.outboxWaiting(core: model.core)
        if waiting > 0 {
            out.append(SidebarPickItem(id: SidebarState.outboxId, group: "Mailboxes", title: Words.text("Outbox"), symbol: SidebarState.outboxSymbol,
                                       count: waiting, selected: st.selection.id == SidebarState.outboxId, depth: 0))
        }
        for b in st.visibleMailboxes + st.visibleViews {
            let id = "mailbox.\(SidebarState.slug(b.name))"
            out.append(SidebarPickItem(id: id, group: "Mailboxes", title: Words.text(b.name), symbol: b.symbol,
                                       count: SidebarState.shownCount(model.counts.mailboxes[b.name.lowercased()]), selected: st.selection.id == id, depth: 0))
        }
        for f in SidebarData.smartFolders(core: model.core) {
            out.append(SidebarPickItem(id: "smart.\(f.id)", group: "Smart folders", title: f.name, symbol: "folder", count: nil,
                                       selected: st.selection.id == "smart.\(f.id)", depth: 0))
        }
        let labels = SidebarData.labels(core: model.core)
        for l in labels where st.labelVisible(l, in: labels) {
            let count = l.hasChildren ? l.total : l.own
            out.append(SidebarPickItem(id: "label.\(l.id)", group: "Labels", title: SidebarData.title(l.label), symbol: "tag", count: count > 0 ? count : nil,
                                       selected: st.selection.id == "label.\(l.id)", depth: l.depth))
        }
        return out
    }

    /// The picker's choice: the same selection a sidebar click makes (All inboxes also clears the account filter).
    @discardableResult public static func choose(_ id: String, model: AppModel = .shared) -> Bool {
        let st = SidebarState.shared
        guard pickItems(model: model).contains(where: { $0.id == id }) else { return false }
        if !st.select(id) { st.selection.id = id; st.showInList(id) }
        if id == "mailbox.all-inboxes" { st.clearAccounts(model: model) }
        return true
    }

    static func pickRows() -> [ControlRow] {
        pickItems().map { ["ax": "mac-main.sidebar.pick-item.\($0.id)", "id": $0.id, "group": $0.group, "title": $0.title, "count": $0.count ?? -1,
                           "selected": $0.selected, "depth": $0.depth] }
    }
}

extension SidebarFeature {
    /// One door per item the hidden sidebar's picker lists: it runs `SidebarVisibility.choose`, what choosing the item does
    /// (the same selection a sidebar click makes). Built at each lookup, so it follows the rows the sidebar shows now (doors-nav).
    static func pickerClicks() -> [String: (AppModel) -> Bool] {
        var t: [String: (AppModel) -> Bool] = [:]
        for item in SidebarVisibility.pickItems() {
            let id = item.id
            t["mac-main.sidebar.pick-item.\(id)"] = { m in SidebarVisibility.choose(id, model: m) }
        }
        return t
    }
}
