import SwiftUI

/// The accounts menu at the sidebar foot (owner: "a pull up / pull down submenu"). Collapsed it is one bar,
/// SIDEBAR_FOOT_PX tall: the account marks in a row, the sync line under them. A click or a drag up pulls it up as a
/// panel over the sidebar; a click on the bar, a drag down, Escape or a click outside pulls it down. The panel follows
/// the drag, settles on the system spring in ACCOUNTS_MENU_MS and can be reversed mid-way; nothing waits for it.
struct SidebarAccountsMenu: View {
    let accounts: [SidebarAccount]
    let model: AppModel
    @Bindable var state: SidebarState
    @Environment(\.accessibilityReduceMotion) private var reduce
    @GestureState private var drag: CGFloat = 0
    @State private var panelHeight: CGFloat = 0

    static let foot = CGFloat(WhooshConstants.SIDEBAR_FOOT_PX)

    var body: some View {
        GeometryReader { g in
            ZStack(alignment: .bottom) {
                if state.accountsMenuOpen {
                    Rectangle().fill(WhooshPalette.sheetScrim).contentShape(Rectangle())
                        .onTapGesture { state.setAccountsMenu(false, reduce: reduce) }
                        .transition(.opacity)
                        .padding(.bottom, Self.foot)
                        .whooshAx("mac-main.sidebar.accounts-menu.outside")
                }
                panel(maxHeight: g.size.height - Self.foot - WhooshMetrics.sidebarTop)
                    .offset(y: offset)
                    .frame(maxHeight: .infinity, alignment: .bottom)
                    .clipped()
                    .padding(.bottom, Self.foot)
                    .allowsHitTesting(state.accountsMenuOpen)
                SidebarAccountsBar(accounts: accounts, model: model, open: state.accountsMenuOpen)
                    .frame(height: Self.foot)
                    .contentShape(Rectangle())
                    .onTapGesture { state.setAccountsMenu(!state.accountsMenuOpen, reduce: reduce) }
                    .gesture(pull)
                    .whooshAx("mac-main.sidebar.accounts-menu")
                    .accessibilityValue(state.accountsMenuOpen ? "pulled up" : "collapsed")
            }
        }
    }

    /// 0 = fully up; panelHeight = hidden behind the bar. The drag moves it between the two.
    private var offset: CGFloat {
        let rest: CGFloat = state.accountsMenuOpen ? 0 : panelHeight + WhooshSpacing.s16
        return min(max(rest + drag, 0), panelHeight + WhooshSpacing.s16)
    }

    private var pull: some Gesture {
        DragGesture(minimumDistance: WhooshSpacing.s4)
            .updating($drag) { v, s, _ in s = v.translation.height }
            .onEnded { v in
                let d = v.predictedEndTranslation.height
                if d < -WhooshSpacing.s24 { state.setAccountsMenu(true, reduce: reduce) }
                else if d > WhooshSpacing.s24 { state.setAccountsMenu(false, reduce: reduce) }
            }
    }

    private func panel(maxHeight: CGFloat) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            Capsule().fill(WhooshPalette.tertiaryLabel).frame(width: WhooshSpacing.s32, height: WhooshSpacing.s4)
                .frame(maxWidth: .infinity).padding(.top, WhooshSpacing.s6).padding(.bottom, WhooshSpacing.s2)
                .contentShape(Rectangle()).gesture(pull)
            allAccounts
            ScrollView(.vertical) {
                SidebarAccounts(accounts: accounts, model: model, state: state).padding(.bottom, WhooshSpacing.s8)
            }
            .scrollIndicators(.never)
            .onScrollGeometryChange(for: Bool.self) { g in g.contentOffset.y + g.containerSize.height < g.contentSize.height } action: { _, more in
                state.menuMoreBelow = more
            }
            .mask { SidebarScrollEdge(fades: state.menuMoreBelow) }
        }
        .padding(.horizontal, WhooshSpacing.s6)
        .frame(maxHeight: max(maxHeight, 0), alignment: .top)
        .fixedSize(horizontal: false, vertical: true)
        .background {
            Color.clear.whooshSystemMaterial(.regularMaterial, in: UnevenRoundedRectangle(topLeadingRadius: WhooshRadius.sheet, topTrailingRadius: WhooshRadius.sheet))
                .overlay(UnevenRoundedRectangle(topLeadingRadius: WhooshRadius.sheet, topTrailingRadius: WhooshRadius.sheet)
                    .fill(WhooshPalette.panel))
        }
        .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { panelHeight = $0 }
        .padding(.horizontal, WhooshSpacing.s6)
        .whooshAx("mac-main.sidebar.accounts-menu.panel")
    }

    /// The All-accounts tick: ticked while no account filters the list; a click clears the filter.
    private var allAccounts: some View {
        let all = state.uncheckedNow(model).isEmpty
        return Button { state.clearAccounts(model: model) } label: {
            HStack(spacing: WhooshSpacing.s8) {
                Image(systemName: "tray.2").font(.system(size: WhooshMetrics.actionIcon - 3)).foregroundStyle(WhooshPalette.accent)
                    .frame(width: AccountMarkSize.sidebar.points)
                Text(Words.text("All accounts")).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.label)
                Spacer(minLength: WhooshSpacing.s4)
                SidebarCount(value: accounts.reduce(0) { $0 + $1.unread }).foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
                Image(systemName: "checkmark").font(.system(size: WhooshSpacing.s10, weight: .semibold))
                    .foregroundStyle(WhooshPalette.accent).opacity(all ? 1 : 0).frame(width: WhooshSpacing.s10)
            }
            .padding(.leading, WhooshSpacing.s6).padding(.trailing, WhooshSpacing.s6)
            .frame(height: WhooshMetrics.accountItem + WhooshSpacing.s12)
            .contentShape(Rectangle())
        }
        .buttonStyle(SidebarPressStyle())
        .whooshAx("mac-main.accounts.all")
    }
}

/// The panel's bottom edge while more accounts sit below it: the last row dissolves over one row-gap instead of
/// being sliced mid-mark (owner: no mark cut). Opaque when the list ends inside the panel.
struct SidebarScrollEdge: View {
    let fades: Bool
    var body: some View {
        VStack(spacing: 0) {
            Rectangle()
            LinearGradient(colors: [.black, .black.opacity(fades ? 0 : 1)], startPoint: .top, endPoint: .bottom)
                .frame(height: WhooshSpacing.s24)
        }
    }
}

enum SidebarAccountsMotion {
    static var pullSeconds: Double { Double(WhooshConstants.ACCOUNTS_MENU_MS) / 1000 }
    static func pull(reduce: Bool) -> Animation { reduce ? WhooshMotion.hover : .spring(duration: pullSeconds, bounce: 0) }
}

extension SidebarState {
    @discardableResult
    func setAccountsMenu(_ open: Bool, reduce: Bool = false) -> Bool {
        guard open != accountsMenuOpen else { return false }
        withAnimation(SidebarAccountsMotion.pull(reduce: reduce)) { accountsMenuOpen = open }
        return true
    }
}
