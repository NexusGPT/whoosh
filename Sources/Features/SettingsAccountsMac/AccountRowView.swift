#if os(macOS)
import SwiftUI

/// One account in the list: its emblem, name, address and provider, the unread count and the sync line; a click opens
/// its own page. An account that needs you carries the one button that fixes it.
struct AccountRowView: View {
    let session: AccountsSession
    let account: AccountEntry
    let needsYou: Bool
    private let model = SettingsWindowModel.shared

    private var failing: Bool { session.state(account.id)?.lastError != nil }
    private var syncing: Bool { session.state(account.id).map { !$0.historyFetched && $0.lastError == nil } ?? false }
    private var emblemState: EmblemState { failing ? .needsSignIn : (syncing ? .syncing : .live) }
    private var unread: Int { AccountBook.shared.account(account.id)?.unread ?? 0 }

    var body: some View {
        SettingsRow(ax: "mac-settings-accounts.\(account.id)", title: account.name,
                    caption: caption, captionStrong: needsYou,
                    leading: AnyView(EmblemMark(glyph: EmblemLibrary.resolve(account.iconId), colourHex: account.colour, size: .points(36),
                                                state: emblemState, imagePath: account.imagePath, name: account.name, address: account.address)),
                    onTap: { _ = model.openAccount(account.id) }) {
            HStack(spacing: 10) {
                if account.paused {
                    SettingsChip(text: AccountsWords.paused).whooshAx("mac-settings-accounts.\(account.id).paused")
                }
                if unread > 0 && !needsYou {
                    Text("\(unread)").font(.system(size: 11, weight: .semibold)).monospacedDigit().foregroundStyle(WhooshPalette.label)
                        .padding(.horizontal, 6).frame(minWidth: 22, minHeight: 18)
                        .background(Capsule().fill(WhooshPalette.fill))
                        .accessibilityLabel(AccountsWords.unreadCount(unread))
                }
                if needsYou {
                    SettingsButton(title: AccountsWords.signInAgain, primary: true, ax: "mac-settings-accounts.\(account.id).sign-in-again") {
                        _ = session.signInAgain(account.id, model: AppModel.shared)
                    }
                } else {
                    Text(session.syncLine(account.id)).font(.system(size: 11.5)).foregroundStyle(WhooshPalette.secondaryLabel)
                        .lineLimit(1).frame(maxWidth: 150, alignment: .trailing)
                }
                Image(systemName: "chevron.right").font(.system(size: 11, weight: .semibold)).foregroundStyle(WhooshPalette.tertiaryLabel)
            }
        }
        .accessibilityElement(children: .contain)
    }

    private var caption: String {
        let base = account.address + " · " + account.provider
        return needsYou ? base + "\n" + session.syncLine(account.id) : base
    }
}

/// A hairline progress bar for the older mail still being read: quiet, one tone, no animation of its own.
struct HistoryBar: View {
    let share: Double
    var body: some View {
        Capsule().fill(WhooshPalette.fill)
            .frame(width: WhooshSpacing.s32 * 2, height: WhooshSpacing.s4)
            .overlay(alignment: .leading) {
                Capsule().fill(WhooshPalette.accent).frame(width: WhooshSpacing.s32 * 2 * CGFloat(min(1, share)))
            }
            .accessibilityHidden(true)
    }
}
#endif
