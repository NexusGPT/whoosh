import SwiftUI

/// The mac-main sidebar (Native Glass v2 `sidebar(o)`): Mailboxes with the inbox picker, Smart folders, nested Labels,
/// and the Accounts section at the foot, over the vibrant sidebar material (or the flat light variant).
public struct SidebarView: View {
    @State private var state = SidebarState.shared
    let model: AppModel
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduce
    @Namespace private var selectionNS

    public init(model: AppModel = .shared) { self.model = model }

    /// Content starts right under the 32 pt title bar (the pane's frame already begins below it): the first header's
    /// 24 pt box then centres on the list column's tab row (round 5; docs/lanes/sidebar.md).
    static let contentTop = WhooshSpacing.s8

    public var body: some View {
        let _ = LaunchClock.once("sidebar-body")   // perf-archive r7: a launch phase (the time before the list's first body)
        let accounts = SidebarData.accounts(core: model.core)
        // perf-archive r7: the outbox count read ONCE per evaluation (it was read three times here and once in `mailboxes`, each a
        // whole `core.outbox()` read); the same value, read in the same pass
        let waiting = SidebarState.outboxWaiting(core: model.core)
        let _ = state.revision
        VStack(alignment: .leading, spacing: 0) {
            if accounts.isEmpty {
                SidebarEmptyState(model: model, state: state).padding(.top, SidebarView.contentTop)
            } else {
            ScrollView(.vertical) {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(state.sections.filter { $0 != "accounts" }, id: \.self) { s in section(s, accounts, waiting) }
                }
                .padding(.horizontal, WhooshSpacing.s10)
                .padding(.bottom, WhooshSpacing.s8)
            }
            .scrollIndicators(.never)
            .padding(.top, SidebarView.contentTop)
            Color.clear.frame(height: SidebarAccountsMenu.foot)
            }
        }
        .animation(SidebarMotion.count, value: accounts.isEmpty)
        .animation(WhooshMotion.hover, value: waiting > 0)
        .onChange(of: waiting) { _, n in
            if n == 0, state.selection.id == SidebarState.outboxId { state.select("mailbox.all-inboxes", reduce: reduce) }
        }
        .overlay { if !accounts.isEmpty { SidebarAccountsMenu(accounts: accounts, model: model, state: state) } }
        .background { material }
        #if os(macOS)
        .background { SidebarWindowAnchor() }
        #endif
        .overlay(alignment: .trailing) {
            Rectangle().fill(WhooshSidebarMaterial.edge).frame(width: WhooshSidebarMaterial.edgeWidth)
        }
        .whooshAx("mac-main.sidebar")
        .modifier(SidebarDialogs(state: state, model: model))
    }

    @ViewBuilder private func section(_ id: String, _ accounts: [SidebarAccount], _ waiting: Int) -> some View {
        switch id {
        case "mailboxes": mailboxes(accounts, waiting)
        case "smart-folders": smartFolders
        case "labels": SidebarLabelsSection(state: state, model: model, ns: selectionNS)
        default: EmptyView()
        }
    }

    private func fold(_ id: String) { withAnimation(SidebarMotion.disclosure(reduce: reduce)) { _ = state.toggleSection(id) } }

    private func mailboxes(_ accounts: [SidebarAccount], _ waiting: Int) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            SidebarHeader(title: "Mailboxes", ax: "mac-main.sidebar.header.mailboxes", section: "mailboxes", collapsed: state.isCollapsed("mailboxes"),
                          onCollapse: { fold("mailboxes") }) { MailboxPicker(accounts: accounts, model: model, state: state) }
            if !state.isCollapsed("mailboxes") {
                if waiting > 0 {
                    SidebarRow(symbol: SidebarState.outboxSymbol, title: Words.text("Outbox"), count: waiting, selected: state.selection.id == SidebarState.outboxId,
                               selectionNS: selectionNS, selectionEpoch: state.selection.epoch) { state.select(SidebarState.outboxId, reduce: reduce) }
                        .transition(.opacity)
                        .accessibilityIdentifier("mac-main.sidebar.mailbox.outbox")
                }
                ForEach(state.visibleMailboxes, id: \.name) { b in mailbox(b, accounts) }
                ForEach(SidebarState.views.filter { !state.hidden.contains(SidebarState.slug($0.name)) }, id: \.name) { b in mailbox(b, accounts) }
            }
        }
    }

    private func mailbox(_ b: (name: String, symbol: String), _ accounts: [SidebarAccount]) -> some View {
        let id = "mailbox.\(SidebarState.slug(b.name))"
        let isAll = b.name == "All inboxes"
        return VStack(alignment: .leading, spacing: 0) {
            SidebarRow(symbol: b.symbol, title: Words.text(b.name), count: SidebarState.shownCount(model.counts.mailboxes[b.name.lowercased()]),
                       gutterChevron: isAll ? state.allInboxesOpen : nil,
                       onGutter: { withAnimation(SidebarMotion.disclosure(reduce: reduce)) { _ = state.toggleAllInboxes() } },
                       selected: state.selection.id == id, selectionNS: selectionNS, selectionEpoch: state.selection.epoch) {
                state.select(id, reduce: reduce)
                if isAll { state.clearAccounts(model: model) }
            }
            .contextMenu { SidebarMenuContent(subject: .mailbox(SidebarState.slug(b.name)), model: model, state: state) }
            .whooshAx("mac-main.sidebar.mailbox.\(SidebarState.slug(b.name))")
            if isAll && state.allInboxesOpen { inboxRows(accounts) }
        }
    }

    /// All inboxes unfolded: one row per account with its count, under the account-group captions.
    private func inboxRows(_ accounts: [SidebarAccount]) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(Array(SidebarState.inboxGroups(accounts, groups: SidebarData.groups(core: model.core)).enumerated()), id: \.offset) { _, g in
                if let caption = g.caption { SidebarInboxCaption(title: caption) }
                ForEach(g.members) { a in
                    SidebarInboxRow(account: a, model: model, chosen: model.accountFilter == a.id) {
                        state.clickAccount(a, model: model)
                        state.select("mailbox.all-inboxes", reduce: reduce)
                    }
                }
            }
        }
        .transition(.opacity)
    }

    private var smartFolders: some View {
        VStack(alignment: .leading, spacing: 0) {
            SidebarHeader(title: "Smart folders", section: "smart-folders", collapsed: state.isCollapsed("smart-folders"),
                          onCollapse: { fold("smart-folders") }) { EmptyView() }
                .contextMenu { SidebarMenuContent(subject: .smartHeader, model: model, state: state) }
            if !state.isCollapsed("smart-folders") {
                ForEach(SidebarData.smartFolders(), id: \.id) { f in
                    SidebarRow(symbol: "folder", title: f.name, count: SidebarData.smartFolderCount(f.id, model: model), selected: state.selection.id == "smart.\(f.id)",
                               selectionNS: selectionNS, selectionEpoch: state.selection.epoch) { state.select("smart.\(f.id)", reduce: reduce) }
                        .help(f.ruleWords)
                        .contextMenu { SidebarMenuContent(subject: .smart(f.id), model: model, state: state) }
                        .whooshAx("mac-main.sidebar.smart.\(f.id)")
                }
            }
        }
    }

    @ViewBuilder private var material: some View {
        if state.lightSidebar {
            Rectangle().fill(WhooshPalette.background)
        } else {
            Color.clear.whooshSystemMaterial(.regularMaterial, in: Rectangle()).overlay(Rectangle().fill(WhooshSidebarMaterial.tint))
        }
    }
}
