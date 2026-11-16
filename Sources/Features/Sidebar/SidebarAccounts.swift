import SwiftUI

/// The accounts list inside the pulled-up menu: the four groups under disclosure chevrons. A folded group is a strip of
/// 20 pt rings; an open group lists each account with its mark, name, health and unread. One click filters the list
/// to that account (a tick marks it), a second click clears; the failing account shows a dashed ring and its words.
struct SidebarAccounts: View {
    let accounts: [SidebarAccount]
    let model: AppModel
    @Bindable var state: SidebarState
    @Environment(\.accessibilityReduceMotion) private var reduce

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(SidebarData.groups(), id: \.self) { g in group(g, accounts.filter { $0.account.group == g }) }
            addAccount
        }
    }

    private func group(_ name: String, _ members: [SidebarAccount]) -> some View {
        let open = state.openGroups.contains(name)
        return VStack(alignment: .leading, spacing: 0) {
            Button { withAnimation(SidebarMotion.disclosure(reduce: reduce)) { state.toggleGroup(name) } } label: {
                HStack(spacing: WhooshSpacing.s4) {
                    Image(systemName: open ? "chevron.down" : "chevron.right").font(.system(size: WhooshSpacing.s8, weight: .bold))
                        .frame(width: WhooshSpacing.s10)
                    Text(name).whooshText(WhooshTypography.caption, emphasized: true)
                    SidebarCount(value: members.count)
                    Spacer(minLength: 0)
                    if members.contains(where: { $0.problem != nil }) {
                        Circle().fill(WhooshPalette.warning).frame(width: WhooshMetrics.warningDot, height: WhooshMetrics.warningDot)
                            .padding(.trailing, WhooshSpacing.s4)
                    }
                }
                .foregroundStyle(open ? AnyShapeStyle(WhooshPalette.label) : AnyShapeStyle(WhooshPalette.sidebarSecondaryLabel))
                .padding(.horizontal, WhooshSpacing.s6)
                .frame(height: WhooshMetrics.sidebarRow)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("mac-main.sidebar.group.\(SidebarState.slug(name))")
            .accessibilityValue(open ? "open" : "folded")
            if open { ForEach(members) { row($0) } } else { ringStrip(members) }
        }
    }

    /// The pull-up ends with "Add account…": the same height as an account row, a plus for its mark.
    private var addAccount: some View {
        Button { state.openAddAccount(model: model) } label: {
            HStack(spacing: WhooshSpacing.s8) {
                Image(systemName: "plus").font(.system(size: WhooshMetrics.actionIcon - 3, weight: .semibold)).foregroundStyle(WhooshPalette.accent)
                    .frame(width: AccountMarkSize.sidebar.points)
                Text(Words.text("Add account…")).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.label)
                Spacer(minLength: WhooshSpacing.s4)
            }
            .padding(.leading, WhooshSpacing.s16).padding(.trailing, WhooshSpacing.s6)
            .frame(maxWidth: .infinity, minHeight: WhooshMetrics.accountItem + WhooshSpacing.s12, alignment: .leading)
            .contentShape(Rectangle())
        }
        .buttonStyle(SidebarPressStyle())
        .help(SidebarState.addAccountTooltip)
        .whooshAx("mac-main.sidebar.accounts-menu.add")
    }

    private func ringStrip(_ members: [SidebarAccount]) -> some View {
        LazyVGrid(columns: [GridItem(.adaptive(minimum: AccountMarkSize.sidebar.points), spacing: WhooshAccountMark.gap)],
                  alignment: .leading, spacing: WhooshAccountMark.gap) {
            ForEach(members) { a in
                Button { state.clickAccount(a, model: model) } label: { SidebarAccountMark(account: a, model: model) }
                    .buttonStyle(SidebarPressStyle())
                    .help("\(a.account.displayName) · \(a.health.words)")
                    .whooshAx("mac-main.accounts.\(a.id)")
            }
        }
        .padding(.leading, WhooshSpacing.s20)
        .padding(.bottom, WhooshSpacing.s4)
    }

    private func row(_ a: SidebarAccount) -> some View {
        let chosen = model.accountFilter == a.id
        return Button { state.clickAccount(a, model: model) } label: {
            HStack(spacing: WhooshSpacing.s8) {
                SidebarAccountMark(account: a, model: model)
                VStack(alignment: .leading, spacing: 0) {
                    Text(a.account.displayName).whooshText(WhooshTypography.subhead, emphasized: a.unread > 0)
                        .foregroundStyle(WhooshPalette.label).lineLimit(1).truncationMode(.tail)
                    Text(a.health.live ? a.account.address : a.health.words).whooshText(WhooshTypography.caption)
                        .foregroundStyle(a.health.warns ? AnyShapeStyle(WhooshPalette.warning) : AnyShapeStyle(WhooshPalette.sidebarSecondaryLabel))
                        .lineLimit(1).truncationMode(.middle)
                }
                Spacer(minLength: WhooshSpacing.s4)
                if a.unread > 0 { SidebarCount(value: a.unread).foregroundStyle(WhooshPalette.sidebarSecondaryLabel) }
                Image(systemName: "checkmark").font(.system(size: WhooshSpacing.s10, weight: .semibold))
                    .foregroundStyle(WhooshPalette.accent).opacity(chosen ? 1 : 0).frame(width: WhooshSpacing.s10)
            }
            .padding(.leading, WhooshSpacing.s16).padding(.trailing, WhooshSpacing.s6)
            .frame(maxWidth: .infinity, minHeight: WhooshMetrics.accountItem + WhooshSpacing.s12, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: WhooshRadius.control)
                .fill(chosen ? AnyShapeStyle(WhooshPalette.hover) : AnyShapeStyle(Color.clear)))
            .contentShape(Rectangle())
        }
        .buttonStyle(SidebarPressStyle())
        .whooshAx("mac-main.accounts.\(a.id)")
    }
}
