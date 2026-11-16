import SwiftUI

/// Registration for the sidebar lane: the rows the sidebar shows and the clicks its controls answer.
/// The same functions back the real controls (SidebarView) and the socket verbs `rows` / `click`.
public enum SidebarFeature: FeatureModule {
    public static let id = "sidebar"
    public static let surfaces: [String] = ["mac-main"]

    /// The shell's sidebar pane (docs/lanes/appshell.md "## Slot contract").
    public static func view(for slot: SurfaceSlot) -> AnyView? {
        guard slot == .macSidebar else { return nil }
        SidebarVisibility.startOnce()
        return AnyView(SidebarView())
    }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        let st = SidebarState.shared
        let rows: [String: (AppModel) throws -> [ControlRow]] = [
            "mac-main.sidebar.mailboxes": { m in
                let waiting = SidebarState.outboxWaiting(core: m.core)
                let outbox: [ControlRow] = waiting > 0
                    ? [["ax": "mac-main.sidebar.mailbox.outbox", "name": "Outbox", "count": waiting, "first": true]] : []
                return outbox + st.visibleMailboxes.map { b in
                    var r: ControlRow = ["ax": "mac-main.sidebar.mailbox.\(SidebarState.slug(b.name))", "name": b.name, "title": Words.text(b.name)]
                    if let c = SidebarState.shownCount(m.counts.mailboxes[b.name.lowercased()]) { r["count"] = c }
                    return r
                }
            },
            "mac-main.sidebar.views": { m in
                SidebarState.views.map { b in
                    var r: ControlRow = ["ax": "mac-main.sidebar.mailbox.\(SidebarState.slug(b.name))", "name": b.name, "title": Words.text(b.name),
                                         "selected": st.selection.id == "mailbox.\(SidebarState.slug(b.name))"]
                    if let c = SidebarState.shownCount(m.counts.mailboxes[b.name.lowercased()]) { r["count"] = c }
                    return r
                }
            },
            "mac-main.sidebar.accounts": { m in
                let book = AccountBook.shared.accounts(core: m.core)
                return SidebarData.accounts(core: m.core).map { a in
                    let mark = AccountBook.mark(of: a.id, in: book)
                    var r: ControlRow = ["ax": "mac-main.accounts.\(a.id)", "id": a.id, "name": a.account.displayName,
                                         "icon": mark.iconValue, "colour": mark.colour,
                                         "address": a.account.address, "group": a.account.group ?? "", "unread": a.unread,
                                         "ring_px": Int(AccountMarkSize.sidebar.points), "filtered": m.accountFilter == a.id,
                                         "named": st.openGroups.contains(a.account.group ?? "")]
                    if let p = a.problem { r["status"] = "sign in again"; r["line"] = "\(a.account.displayName) \(p)" }
                    if let b = a.backfill { r["backfill"] = b }
                    r["health"] = a.health.words
                    return r
                }
            },
            "mac-main.sidebar.groups": { m in
                let all = SidebarData.accounts(core: m.core)
                return SidebarData.groups(core: m.core).map { g in
                    let members = all.filter { $0.account.group == g }
                    return ["ax": "mac-main.sidebar.group.\(SidebarState.slug(g))", "name": g, "count": members.count,
                            "open": st.openGroups.contains(g), "warn": members.contains { $0.problem != nil }]
                }
            },
            "mac-main.sidebar.smart-folders": { m in
                SidebarData.smartFolders().map { f in
                    var r: ControlRow = ["ax": "mac-main.sidebar.smart.\(f.id)", "name": f.name, "rule": f.ruleWords]
                    if let c = SidebarData.smartFolderCount(f.id, model: m) { r["count"] = c }
                    return r
                }
            },
            "mac-main.sidebar.inbox-picker": { m in
                SidebarData.accounts(core: m.core).map { a in
                    ["ax": "mac-main.sidebar.pick.\(a.id)", "id": a.id, "name": a.account.displayName, "checked": st.isChecked(a.id, model: m),
                     "open": st.pickerOpen]
                }
            },
            "mac-main.sidebar.visibility": { _ in [SidebarVisibility.row()] },
            "mac-main.sidebar.style": { _ in
                [["light": st.lightSidebar, "sections": st.sections, "hidden": st.hidden.sorted()]]
            },
        ]
        return rows.merging(round2Rows(st)) { _, new in new }.merging(menuRows(st)) { _, new in new }
            .merging(structureRows(st)) { _, new in new }.merging(labelRows(st)) { _, new in new }
            .merging(viewRows(st)) { _, new in new }
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        let st = SidebarState.shared
        var t: [String: (AppModel) -> Bool] = [
            "mac-main.accounts.all": { m in st.clearAccounts(model: m); return true },
            "mac-main.sidebar.mailboxes.pick": { _ in st.pickerOpen.toggle(); return true },
            "mac-main.sidebar.pick.all": { m in st.checkAll(model: m); return true },
            "mac-main.sidebar.toggle.button": { _ in SidebarVisibility.toggle() },
            "mac-main.sidebar.show-all": { _ in st.showHidden() },
        ]
        for a in SidebarData.accounts() {
            t["mac-main.accounts.\(a.id)"] = { m in
                guard let live = SidebarData.accounts(core: m.core).first(where: { $0.id == a.id }) else { return false }
                st.clickAccount(live, model: m); return true
            }
            t["mac-main.sidebar.pick.\(a.id)"] = { m in st.togglePick(a.id, model: m) }
        }
        for g in SidebarData.groups() { t["mac-main.sidebar.group.\(SidebarState.slug(g))"] = { _ in st.toggleGroup(g); return true } }
        for b in SidebarState.mailboxes {
            let s = SidebarState.slug(b.name)
            t["mac-main.sidebar.hide.\(s)"] = { _ in st.hidden.insert(s).inserted }
        }
        for l in SidebarData.labels() where l.hasChildren { t["mac-main.sidebar.label.\(l.id).toggle"] = { _ in st.toggleLabel(l.id); return true } }
        return t.merging(round2Clicks(st)) { old, _ in old }.merging(menuClicks(st)) { old, _ in old }
            .merging(structureClicks(st)) { old, _ in old }.merging(labelClicks(st)) { old, _ in old }
            .merging(viewClicks(st)) { old, _ in old }.merging(pickerClicks()) { old, _ in old }
    }
}
