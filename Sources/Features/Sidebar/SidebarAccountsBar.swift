import SwiftUI

/// The collapsed accounts menu: the account marks in one row (as many as fit, then "+N"), the sync line under them,
/// and a chevron that turns as the menu pulls up. An account that needs the owner leads the row with its dashed ring.
struct SidebarAccountsBar: View {
    let accounts: [SidebarAccount]
    let model: AppModel
    let open: Bool

    /// The key shown in the bar's tooltip: the preset's chord for "accounts".
    static var chord: String { ShortcutEngine.shared.chord(for: "accounts") ?? "⌘J" }

    static func slots(width: CGFloat) -> Int {
        let d = AccountMarkSize.sidebar.points, gap = WhooshAccountMark.gap
        return max(1, Int((width - WhooshSpacing.s16 - WhooshSpacing.s24 + gap) / (d + gap)))
    }

    /// Needs-the-owner first, then the account filtering the list, then the rest in their order.
    var ordered: [SidebarAccount] {
        let first = accounts.filter { $0.problem != nil }
        let chosen = accounts.filter { $0.problem == nil && $0.id == model.accountFilter }
        return first + chosen + accounts.filter { $0.problem == nil && $0.id != model.accountFilter }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s6) {
            Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator)
                .padding(.horizontal, WhooshSpacing.s6).opacity(open ? 0 : 1)
            GeometryReader { g in
                let n = Self.slots(width: g.size.width)
                let all = ordered
                let shown = all.count > n ? Array(all.prefix(n - 1)) : all
                HStack(spacing: WhooshAccountMark.gap) {
                    ForEach(shown) { SidebarAccountMark(account: $0, model: model) }
                    if all.count > shown.count {
                        Text("+\(all.count - shown.count)").whooshText(WhooshTypography.caption2, emphasized: true)
                            .foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
                            .padding(.horizontal, WhooshSpacing.s6)
                            .frame(minWidth: AccountMarkSize.sidebar.points, minHeight: AccountMarkSize.sidebar.points)
                            .background(Capsule().fill(WhooshPalette.fill))
                    }
                    Spacer(minLength: 0)
                    Image(systemName: "chevron.up").font(.system(size: WhooshSpacing.s10, weight: .bold))
                        .foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
                        .rotationEffect(.degrees(open ? 180 : 0))
                }
            }
            .frame(height: AccountMarkSize.sidebar.points)
            .padding(.horizontal, WhooshSpacing.s8)
            HStack(spacing: WhooshSpacing.s4) {
                if accounts.contains(where: { $0.problem != nil }) {
                    Circle().fill(WhooshPalette.warning).frame(width: WhooshMetrics.warningDot, height: WhooshMetrics.warningDot)
                }
                Text(line).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
                    .lineLimit(1).truncationMode(.tail)
            }
            .padding(.horizontal, WhooshSpacing.s8)
        }
        .padding(.horizontal, WhooshSpacing.s10)
        .padding(.bottom, WhooshSpacing.s8)
        .frame(maxHeight: .infinity, alignment: .bottom)
        .help("\(Words.text(open ? "Pull the accounts down" : "Pull the accounts up"))  \(Self.chord)")
    }

    /// The sync line; when an account needs the owner, its words lead.
    private var line: String {
        if let bad = accounts.first(where: { $0.problem != nil }) { return "\(bad.account.displayName) · \(bad.health.words)" }
        return MacSyncLine.read(model).summary
    }
}
