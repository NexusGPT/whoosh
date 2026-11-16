import SwiftUI

/// Round 6, S-B structure (docs/lanes/sidebar.md "## Round 6"): the sections fold from a chevron shown on hover, All inboxes
/// unfolds to one row per account under the account-group captions, and the three account entry points share one filter.
/// The folded sections and the unfolded All inboxes are remembered across launches (UserDefaults, a sidebar-owned default).
extension SidebarState {
    static let collapsedKey = "whoosh.sidebar.collapsedSections"
    static let allInboxesKey = "whoosh.sidebar.allInboxesOpen"
    static let sectionTitles: [String: String] = ["mailboxes": "Mailboxes", "smart-folders": "Smart folders", "labels": "Labels"]

    func isCollapsed(_ section: String) -> Bool { collapsedSections.contains(section) }

    @discardableResult func toggleSection(_ id: String) -> Bool {
        guard Self.sectionTitles[id] != nil else { return false }
        if collapsedSections.contains(id) { collapsedSections.remove(id) } else { collapsedSections.insert(id) }
        UserDefaults.standard.set(collapsedSections.sorted(), forKey: Self.collapsedKey)
        return true
    }

    @discardableResult func toggleAllInboxes() -> Bool {
        allInboxesOpen.toggle()
        UserDefaults.standard.set(allInboxesOpen, forKey: Self.allInboxesKey)
        return true
    }

    /// The accounts in group order, each group's members under it; accounts with no group trail without a caption.
    static func inboxGroups(_ accounts: [SidebarAccount], groups: [String]) -> [(caption: String?, members: [SidebarAccount])] {
        var out: [(caption: String?, members: [SidebarAccount])] = groups.compactMap { g in
            let m = accounts.filter { $0.account.group == g }
            return m.isEmpty ? nil : (g, m)
        }
        let loose = accounts.filter { $0.account.group == nil }
        if !loose.isEmpty { out.append((nil, loose)) }
        return out
    }

    /// What the Mailboxes chip says: "All" while every account is checked, the account's name when one is, else how many.
    static func chipLabel(accounts: [SidebarAccount], unchecked: Set<String>) -> String {
        let checked = accounts.filter { !unchecked.contains($0.id) }
        if unchecked.isEmpty || checked.isEmpty { return Words.text("All") }
        if checked.count == 1 { return checked[0].account.displayName }
        return Words.text("%lld inboxes", checked.count)
    }

    /// A count shows only when the core serves one above zero (Junk, Trash and Archive serve none).
    static func shownCount(_ served: UInt32?) -> Int? {
        guard let served, served > 0 else { return nil }
        return Int(served)
    }
}

/// One account's row under All inboxes: its mark, its name and its inbox count; a click filters the list to it (again clears).
struct SidebarInboxRow: View {
    let account: SidebarAccount
    let model: AppModel
    let chosen: Bool
    let action: () -> Void
    @State private var hovering = false
    var body: some View {
        Button(action: action) {
            HStack(spacing: WhooshSpacing.s8) {
                SidebarAccountMark(account: account, model: model).frame(width: WhooshMetrics.actionIcon + WhooshSpacing.s4)
                Text(account.account.displayName).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label).lineLimit(1)
                Spacer(minLength: WhooshSpacing.s4)
                if account.unread > 0 { SidebarCount(value: account.unread).foregroundStyle(WhooshPalette.sidebarSecondaryLabel) }
                Image(systemName: "checkmark").font(.system(size: WhooshSpacing.s10, weight: .semibold))
                    .foregroundStyle(WhooshPalette.accent).opacity(chosen ? 1 : 0).frame(width: WhooshSpacing.s10)
            }
            .padding(.leading, WhooshSpacing.s16 + WhooshSpacing.s8).padding(.trailing, WhooshSpacing.s8)
            .frame(height: WhooshMetrics.sidebarRow + WhooshSpacing.s2)
            .background(RoundedRectangle(cornerRadius: WhooshRadius.control)
                .fill(chosen || hovering ? AnyShapeStyle(WhooshPalette.hover) : AnyShapeStyle(Color.clear)))
            .contentShape(Rectangle())
        }
        .buttonStyle(SidebarPressStyle())
        .onHover { h in withAnimation(WhooshMotion.hover) { hovering = h } }
        .help("\(account.account.displayName) · \(account.health.live ? account.account.address : account.health.words)")
        .whooshAx("mac-main.sidebar.inbox.\(account.id)")
    }
}

/// The group caption over an account's rows.
struct SidebarInboxCaption: View {
    let title: String
    var body: some View {
        Text(title).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
            .padding(.leading, WhooshSpacing.s16 + WhooshSpacing.s8).padding(.top, WhooshSpacing.s6)
            .frame(maxWidth: .infinity, minHeight: WhooshMetrics.sidebarRow - WhooshSpacing.s4, alignment: .leading)
            .whooshAx("mac-main.sidebar.inbox-group.\(SidebarState.slug(title))")
    }
}

extension SidebarFeature {
    static func structureRows(_ st: SidebarState) -> [String: (AppModel) throws -> [ControlRow]] {
        [
            "mac-main.sidebar.sections": { _ in
                ["mailboxes", "smart-folders", "labels"].map { id in
                    ["ax": "mac-main.sidebar.section.\(id).chevron", "id": id, "name": SidebarState.sectionTitles[id] ?? id, "collapsed": st.isCollapsed(id),
                    "saved": (UserDefaults.standard.stringArray(forKey: SidebarState.collapsedKey) ?? []).contains(id)]
                }
            },
            "mac-main.sidebar.inbox-rows": { m in
                let all = SidebarData.accounts(core: m.core)
                let open = st.allInboxesOpen && !st.isCollapsed("mailboxes")
                return SidebarState.inboxGroups(all, groups: SidebarData.groups(core: m.core)).flatMap { g in
                    g.members.map { a in
                        ["ax": "mac-main.sidebar.inbox.\(a.id)", "id": a.id, "name": a.account.displayName, "group": g.caption ?? "", "count": a.unread,
                         "filtered": m.accountFilter == a.id, "visible": open]
                    }
                }
            },
            "mac-main.sidebar.inbox-groups": { m in
                let all = SidebarData.accounts(core: m.core)
                return SidebarState.inboxGroups(all, groups: SidebarData.groups(core: m.core)).compactMap { g in
                    g.caption.map { ["ax": "mac-main.sidebar.inbox-group.\(SidebarState.slug($0))", "name": $0, "count": g.members.count] }
                }
            },
            "mac-main.sidebar.accounts-menu.rows": { m in
                let all = SidebarData.accounts(core: m.core)
                return [["ax": "mac-main.accounts.all", "title": Words.text("All accounts")]] + all.map { ["ax": "mac-main.accounts.\($0.id)", "title": $0.account.displayName] }
                    + [["ax": "mac-main.sidebar.accounts-menu.add", "title": Words.text("Add account…"), "tooltip": SidebarState.addAccountTooltip, "route": SidebarState.addAccountSurface]]
            },
            "mac-main.sidebar.empty": { m in
                SidebarData.accounts(core: m.core).isEmpty ? [["ax": SidebarEmptyState.addAx, "title": Words.text("Add account"), "route": SidebarState.addAccountSurface]] : []
            },
            "mac-main.sidebar.all-inboxes": { _ in [["open": st.allInboxesOpen, "saved": UserDefaults.standard.bool(forKey: SidebarState.allInboxesKey), "ax": "mac-main.sidebar.all-inboxes.chevron"]] },
            "mac-main.sidebar.pick-all": { m in
                let all = SidebarData.accounts(core: m.core)
                return [["ax": "mac-main.sidebar.pick.all", "checked": st.uncheckedNow(m).isEmpty, "checked_count": all.filter { st.isChecked($0.id, model: m) }.count]]
            },
            "mac-main.sidebar.chip": { m in
                [["label": SidebarState.chipLabel(accounts: SidebarData.accounts(core: m.core), unchecked: st.uncheckedNow(m)),
                  "filter": m.accountFilter ?? ""]]
            },
        ]
    }

    static func structureClicks(_ st: SidebarState) -> [String: (AppModel) -> Bool] {
        var t: [String: (AppModel) -> Bool] = [
            "mac-main.sidebar.all-inboxes.chevron": { _ in st.toggleAllInboxes() },
            /// The key path: the command registry answers "sidebar" the way a bound key does.
            "mac-main.sidebar.command.sidebar": { m in CommandRegistry.handler("sidebar")?(m) ?? false },
            "mac-main.sidebar.command.accounts": { m in CommandRegistry.handler("accounts")?(m) ?? false },
            "mac-main.sidebar.accounts-menu.add": { m in st.openAddAccount(model: m) },
            SidebarEmptyState.addAx: { m in st.openAddAccount(model: m) },
        ]
        for (command, _) in goMailboxes { t["mac-main.sidebar.command.\(command)"] = { m in CommandRegistry.handler(command)?(m) ?? false } }
        for id in SidebarState.sectionTitles.keys { t["mac-main.sidebar.section.\(id).chevron"] = { _ in st.toggleSection(id) } }
        for a in SidebarData.accounts() {
            t["mac-main.sidebar.inbox.\(a.id)"] = { m in
                guard let live = SidebarData.accounts(core: m.core).first(where: { $0.id == a.id }) else { return false }
                st.clickAccount(live, model: m)
                st.select("mailbox.all-inboxes")
                return true
            }
        }
        return t
    }
}

extension SidebarState {
    /// A label shows in every mailbox (scope all) or only while its own account is the filter.
    static func scopeShows(_ l: MailLabel, filter: String?) -> Bool {
        switch l.scope {
        case .all: return true
        case .account(let id): return filter == id
        }
    }
}

extension SidebarState {
    /// The add-account surface the Settings and Welcome entries route to.
    static let addAccountSurface = "mac-add-account"
    static var addAccountTooltip: String {
        let key = ShortcutEngine.shared.chord(for: "add-account")
        return key.map { "\(Words.text("Add account…"))  \($0)" } ?? Words.text("Add account…")
    }

    /// The pull-up's last row: closes the menu and opens the add-account surface.
    @discardableResult func openAddAccount(model: AppModel) -> Bool {
        setAccountsMenu(false)
        model.surface = Self.addAccountSurface
        return true
    }
}
