import SwiftUI

/// The Mailboxes control the owner asked for: "a button (drop down like to be able to select one or many inboxes)".
/// A popover of tick boxes, one per inbox, grouped as the accounts are; each tick applies to the list at once.
struct MailboxPicker: View {
    let accounts: [SidebarAccount]
    let model: AppModel
    @Bindable var state: SidebarState

    var body: some View {
        Button { state.pickerOpen.toggle() } label: {
            HStack(spacing: WhooshSpacing.s2) {
                Text(label).whooshText(WhooshTypography.caption)
                Image(systemName: "chevron.down").font(.system(size: WhooshSpacing.s8, weight: .bold))
            }
            .foregroundStyle(state.uncheckedNow(model).isEmpty ? AnyShapeStyle(WhooshPalette.sidebarSecondaryLabel) : AnyShapeStyle(WhooshPalette.accent))
            .padding(.horizontal, WhooshSpacing.s6)
            .frame(height: WhooshMetrics.sidebarRow - WhooshSpacing.s4)
            .background(Capsule().fill(WhooshPalette.fill))
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("mac-main.sidebar.mailboxes.pick")
        .help(Words.text("Choose one or many inboxes"))
        .popover(isPresented: $state.pickerOpen, arrowEdge: .trailing) { list.popoverEntrance(anchor: .leading) }
    }

    private var label: String { SidebarState.chipLabel(accounts: accounts, unchecked: state.uncheckedNow(model)) }

    private var list: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text(Words.text("Inboxes")).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.label)
                Spacer()
                Toggle(isOn: Binding(get: { state.uncheckedNow(model).isEmpty }, set: { _ in state.checkAll(model: model) })) {
                    Text(Words.text("All")).whooshText(WhooshTypography.caption)
                }
                .tickBoxStyle()
                .accessibilityIdentifier("mac-main.sidebar.pick.all")
            }
            .padding(.horizontal, WhooshSpacing.s12).padding(.vertical, WhooshSpacing.s8)
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(SidebarData.groups(), id: \.self) { g in
                        Text(g).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
                            .padding(.horizontal, WhooshSpacing.s12).padding(.top, WhooshSpacing.s8)
                        ForEach(accounts.filter { $0.account.group == g }) { a in tick(a) }
                    }
                }
                .padding(.bottom, WhooshSpacing.s8)
            }
        }
        .frame(width: WhooshMetrics.blockWidth / 2 + WhooshSpacing.s16, height: WhooshMetrics.readerMeasure * 0.8)
    }

    private func tick(_ a: SidebarAccount) -> some View {
        Toggle(isOn: Binding(get: { state.isChecked(a.id, model: model) }, set: { _ in state.togglePick(a.id, model: model) })) {
            HStack(spacing: WhooshSpacing.s6) {
                AccountMark(account: a.account, size: .compact, needsAttention: a.problem != nil, hasUnread: a.unread > 0)
                VStack(alignment: .leading, spacing: 0) {
                    Text(a.account.displayName).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.label)
                    Text(a.account.address).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                        .lineLimit(1).truncationMode(.middle)
                }
            }
        }
        .tickBoxStyle()
        .padding(.horizontal, WhooshSpacing.s12).padding(.vertical, WhooshSpacing.s2)
        .accessibilityIdentifier("mac-main.sidebar.pick.\(a.id)")
    }
}

private extension View {
    /// The Mac's own tick box; iOS (where this target also builds) keeps the default toggle.
    @ViewBuilder func tickBoxStyle() -> some View {
        #if os(macOS)
        toggleStyle(.checkbox)
        #else
        self
        #endif
    }
}
