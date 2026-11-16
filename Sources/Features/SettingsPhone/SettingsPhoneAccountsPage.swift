import SwiftUI

/// The accounts in the order the Mac lists them (its groups, then the core's order inside each group). One read for the page, the
/// probe rows and the taps: the order the owner sees is the order the check reads.
@MainActor
enum SettingsPhoneAccounts {
    static func ordered(_ s: AccountsSession) -> [AccountEntry] { s.sections.flatMap(\.accounts) }

    /// The state in words: the sign-in ask, a pause, or the sync line the Mac's row shows.
    static func stateWords(_ a: AccountEntry, _ s: AccountsSession) -> String {
        if failing(a, s) { return AccountsWords.signInAgain }
        if a.paused { return AccountsWords.paused }
        return s.syncLine(a.id)
    }
    static func failing(_ a: AccountEntry, _ s: AccountsSession) -> Bool { s.state(a.id)?.lastError != nil }

    /// The mark the inbox draws for this account (PhoneTile): the account's icon in a ring of its colour; dashed + orange dot = sign in again.
    static func mark(_ a: AccountEntry, _ s: AccountsSession) -> PhoneAccount {
        PhoneAccount(id: a.id, name: a.name, address: a.address, colour: a.colour, group: a.group, iconId: a.iconId, needsAttention: failing(a, s))
    }
}

/// Accounts: the accounts that came from the Mac, each with its mark, name, address and state. Read only: they change on the Mac.
struct SettingsPhoneAccountsPage: View {
    private let state = SettingsPhoneState.shared
    @State private var session = AccountsSession.shared

    var body: some View {
        let ordered = SettingsPhoneAccounts.ordered(session)
        SettingsPhoneFrame(title: SettingsPhoneWords.accounts, titleAx: "iphone-settings.accounts.title", onBack: { _ = state.back() }) {
            ForEach(Array(session.sections.enumerated()), id: \.offset) { _, section in
                if let title = section.title { SettingsPhoneSection(title: title) }
                SettingsPhoneCard {
                    ForEach(section.accounts) { a in
                        let index = ordered.firstIndex { $0.id == a.id } ?? 0
                        row(a, index: index)
                        if a.id != section.accounts.last?.id { SettingsPhoneDivider(inset: PhoneMetrics.gutter + PhoneMetrics.tileSmall + WhooshSpacing.s12) }
                    }
                }
            }
            SettingsPhoneNote(text: SettingsPhoneWords.accountsFooter)
        }
        .task { session.reload() }
    }

    private func row(_ a: AccountEntry, index: Int) -> some View {
        let failing = SettingsPhoneAccounts.failing(a, session)
        return HStack(spacing: WhooshSpacing.s12) {
            PhoneTile(account: SettingsPhoneAccounts.mark(a, session), small: true)
            VStack(alignment: .leading, spacing: 0) {
                Text(a.name).phoneText(PhoneType.body).foregroundStyle(WhooshPalette.label).lineLimit(1)
                Text(a.address).phoneText(PhoneType.footnote).foregroundStyle(WhooshPalette.sidebarSecondaryLabel).lineLimit(1)
                Text(SettingsPhoneAccounts.stateWords(a, session)).phoneText(PhoneType.footnote)
                    .foregroundStyle(failing ? AnyShapeStyle(WhooshPalette.warning) : AnyShapeStyle(WhooshPalette.sidebarSecondaryLabel)).lineLimit(2)
            }
            Spacer(minLength: 0)
            // The Mac's notification setting for this account, mirrored (C-J4.8): shown only when it is off, in the Mac's own words.
            if session.isMuted(a.id) {
                Text(AccountsWords.noNotifications).phoneText(PhoneType.footnote).foregroundStyle(WhooshPalette.sidebarSecondaryLabel).lineLimit(1)
            }
        }
        .padding(.horizontal, PhoneMetrics.gutter).padding(.vertical, WhooshSpacing.s8)
        .frame(minHeight: PhoneMetrics.accountRow, alignment: .leading)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("iphone-settings.accounts.row.\(index)")
    }
}
