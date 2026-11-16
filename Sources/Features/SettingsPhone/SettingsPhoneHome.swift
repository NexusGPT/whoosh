import SwiftUI

/// The home list (the iOS 26 Settings pattern for this role): the large title, the accounts card, then the groups Mail, Sync, AI and
/// General, each row pushing its own page. Paper-grey ground, white cards, glass only on Done, which floats.
struct SettingsPhoneHome: View {
    private let m = AppModel.shared
    private let state = SettingsPhoneState.shared
    @State private var store = GeneralStore.shared
    @State private var session = AccountsSession.shared
    @State private var ai = AISetupStatus.shared

    var body: some View {
        ZStack(alignment: .top) {
            PhoneGround.grouped.ignoresSafeArea()
            ScrollView {
                VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
                    Text(SettingsPhoneWords.title).phoneText(PhoneType.largeTitle).foregroundStyle(WhooshPalette.label)
                        .padding(.horizontal, PhoneMetrics.gutter).padding(.top, PhoneMetrics.titleTop)
                        .padding(.bottom, SettingsPhoneMetrics.titleBottom)
                        .accessibilityAddTraits(.isHeader).accessibilityIdentifier("iphone-settings.title")
                    summary
                    ForEach(SettingsPhoneGroup.all) { g in
                        SettingsPhoneSection(title: g.title)
                        SettingsPhoneCard {
                            ForEach(g.pages) { page in
                                SettingsPhoneRow(page: page, value: value(page), attention: attention(page)) { state.open(page) }
                                if page != g.pages.last { SettingsPhoneDivider(inset: PhoneMetrics.gutter + SettingsPhoneMetrics.paneIcon + WhooshSpacing.s12) }
                            }
                        }
                    }
                }
                .padding(.bottom, PhoneMetrics.listBottomRoom)
            }
            .scrollIndicators(.hidden)
        }
        .overlay(alignment: .topTrailing) {
            Button { state.close(m) } label: {
                Text(SettingsPhoneWords.done).phoneText(PhoneType.headline).foregroundStyle(WhooshPalette.label)
                    .frame(width: PhoneMetrics.selectWidth, height: PhoneMetrics.roundButton).whooshGlass(in: Capsule())
            }
            .buttonStyle(PressStyle()).accessibilityIdentifier("iphone-settings.done").padding(.horizontal, PhoneMetrics.gutter)
        }
        .task { ai.refresh(); session.reload() }
        .phoneBarHidden()
    }

    /// The accounts card: the first marks, then the count and how many need a sign-in. It opens Accounts.
    private var summary: some View {
        let accounts = session.accounts
        let failing = accounts.filter { SettingsPhoneAccounts.failing($0, session) }.count
        return Button { state.open(.accounts) } label: {
            VStack(alignment: .leading, spacing: WhooshSpacing.s12) {
                HStack(spacing: 0) {
                    HStack(spacing: -SettingsPhoneMetrics.markOverlap) {
                        ForEach(accounts.prefix(SettingsPhoneMetrics.markStack)) { a in
                            PhoneTile(account: SettingsPhoneAccounts.mark(a, session), small: true)
                        }
                    }
                    Spacer(minLength: WhooshSpacing.s8)
                    Image(systemName: "chevron.right").font(.system(size: PhoneType.footnote.size, weight: .semibold)).foregroundStyle(WhooshPalette.tertiaryLabel)
                }
                Text(accounts.isEmpty ? SettingsPhoneWords.accountsFooter : SettingsPhoneWords.accountsLead(count: accounts.count, failing: failing))
                    .phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel).multilineTextAlignment(.leading)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(PhoneMetrics.gutter).frame(maxWidth: .infinity, alignment: .leading).contentShape(Rectangle())
        }
        .buttonStyle(SettingsRowPress())
        .background(PhoneGround.card, in: RoundedRectangle(cornerRadius: PhoneMetrics.cardRadius, style: .continuous))
        .clipShape(RoundedRectangle(cornerRadius: PhoneMetrics.cardRadius, style: .continuous))
        .padding(.horizontal, PhoneMetrics.gutter)
        .accessibilityIdentifier("iphone-settings.summary")
    }

    /// The grey value at a row's right edge: the pick, or the one fact that says where the page stands.
    private func value(_ page: SettingsPhonePage) -> String {
        switch page {
        case .accounts: return "\(session.accounts.count)"
        case .sync:
            let r = SettingsPhoneSync.read(m)
            return r.phase == .inSync || r.phase == .failing ? r.device : (r.phase == .waiting ? SettingsPhoneWords.notSyncedYet : "")
        case .ai: return SettingsPhoneAI.allReady() ? SettingsPhoneWords.ready : ""
        case .language: return SettingsPhoneLanguagePage.choices.first { $0.code == store.language }?.name ?? SettingsPhoneWords.system
        case .appearance: return SettingsPhoneWords.theme(store.appearance.rawValue)
        }
    }

    /// The amber dot: a page whose state needs the owner (a sync that failed, a model that is missing, an account to sign in again).
    private func attention(_ page: SettingsPhonePage) -> Bool {
        switch page {
        case .sync: return SettingsPhoneSync.read(m).phase == .failing
        case .ai: return !SettingsPhoneAI.allReady()
        case .accounts: return session.accounts.contains { SettingsPhoneAccounts.failing($0, session) }
        default: return false
        }
    }
}
