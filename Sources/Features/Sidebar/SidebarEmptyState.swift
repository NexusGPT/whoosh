import SwiftUI

/// The sidebar when the core lists no account (a fresh install; `fixture accounts none`): the glass brand mark at 24 pt, one line and
/// one button that opens the add-account sheet. It stands in place of the mailboxes, smart folders, labels and the accounts bar.
/// It arrives with the sidebar's own cross-fade (SidebarMotion.count); nothing loops. Colours come from WhooshPalette only.
struct SidebarEmptyState: View {
    let model: AppModel
    let state: SidebarState

    static let markSize: CGFloat = 24
    static let surface = "sidebar-empty"
    static let addAx = "mac-main.sidebar.empty.add"

    var body: some View {
        VStack(spacing: WhooshSpacing.s8) {
            BrandMark(size: Self.markSize, surface: Self.surface)
            Text(Words.text("No account yet")).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
            Button { state.openAddAccount(model: model) } label: {
                Text(Words.text("Add account")).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.accent)
                    .padding(.horizontal, WhooshSpacing.s10)
                    .frame(minHeight: WhooshMetrics.accountItem)
                    .contentShape(Rectangle())
            }
            .buttonStyle(SidebarPressStyle())
            .help(SidebarState.addAccountTooltip)
            .whooshAx(Self.addAx)
        }
        .padding(.horizontal, WhooshSpacing.s10)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        // Fades in, leaves at once: a removal fade keeps the view (and its BrandMark row) mounted until the animation completes, which a hidden
        // window never ticks (main55: sidebar-empty-gone-with-an-account read 1 mark after `fixture accounts all`).
        .transition(.asymmetric(insertion: .opacity, removal: .identity))
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("mac-main.sidebar.empty")
    }
}

extension SidebarState {
    /// The account list changed behind the sidebar's back (`fixture accounts none|all`, the last account removed): AppModel.refresh() assigns
    /// only rows and counts that differ, and the sidebar reads the core's accounts untracked, so nothing redraws the empty-state branch
    /// until the revision the body reads moves.
    func accountsChanged() { revision += 1 }

    /// The mail changed (the list's rows or the counts moved): the label rows read the core's labels untracked, so their counts stayed as first drawn while
    /// the tab strip moved (the lead's walk on main183: a mail is opened, the tab says "Needs Reply 29", the sidebar still 30). The label rows are drawn
    /// again once the mail has been quiet for SIDEBAR_COUNTS_FOLLOW_MS: one redraw after a burst of keys, never inside a key's frame. The first version
    /// asked for it with a main-queue block, which runs BEFORE the frame of the key is shown: the real engine's archive went from a median 34.4 ms to
    /// 51.6 ms (main184 against main187, 34 archives each, load 6 to 8).
    func mailChanged() {
        mailRedraw?.cancel()
        let work = DispatchWorkItem { [weak self] in self?.labelsRevision += 1 }
        mailRedraw = work
        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(WhooshConstants.SIDEBAR_COUNTS_FOLLOW_MS), execute: work)
    }
}
