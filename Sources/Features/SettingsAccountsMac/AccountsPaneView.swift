#if os(macOS)
import SwiftUI

/// The sizes the icon and colour picker is drawn at: it never grows past the smallest Settings window.
enum AccountsLayout {
    static let pickerWidth = WhooshMetrics.readerMeasure * 1.25
    static let pickerMaxHeight: CGFloat = SettingsMetrics.windowMin.height - 64
}

private let P = "mac-settings-accounts"

/// Settings > Accounts: the account that needs you on top, then every working account under its group; one click opens
/// an account's own page. Add account (⌘N) and Move to iPhone sit in the header.
struct AccountsPaneView: View {
    @Environment(\.colorScheme) private var scheme   // the service status dot is a plain colour of the app's warning colour
    @State private var session = AccountsSession.shared
    @State private var model = SettingsWindowModel.shared

    var body: some View {
        SettingsPaneScaffold(pane: .accounts, trailing: {
            SettingsButton(title: ImportWords.settingsOffer, symbol: "tray.and.arrow.down", ax: "\(P).import") { session.openImport() }
            SettingsButton(title: AccountsWords.moveToIphone + "…", symbol: "iphone", ax: "\(P).move-to-iphone") { _ = WelcomeMacFeature.openAtQR(AppModel.shared) }
            SettingsButton(title: AccountsWords.addAccount + "…", symbol: "plus", key: "⌘N", primary: true, ax: "\(P).add") { session.adding = true }
        }, content: {
            if !session.failing.isEmpty { needsYou }
            working
            serviceStatus
            SettingsFootnote(text: session.failure ?? AccountsWords.footer, ax: "\(P).notice")
        })
        .overlay { if session.importing { AccountsImportSheet(session: session) } }
        .animation(.easeOut(duration: WhooshMotion.readerCrossFade), value: session.importing)
        .onAppear { session.shown = true; session.reload() }
        .onDisappear { session.shown = false }
        .accessibilityIdentifier(P)
    }

    private var needsYou: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 7) {
                Circle().fill(WhooshPalette.warning).frame(width: 7, height: 7)
                Text(AccountsWords.needsYouCount(session.failing.count)).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label)
            }
            .padding(.horizontal, WhooshSpacing.s4).padding(.bottom, WhooshSpacing.s6).padding(.top, WhooshSpacing.s16)
            SettingsCard {
                ForEach(session.failing) { a in AccountRowView(session: session, account: a, needsYou: true) }
            }
            .whooshAx("\(P).failing")
        }
    }

    /// One row per mail service, in the fixed order; the state in words from this Mac's own accounts of that service.
    private var serviceStatus: some View {
        SettingsGroup(title: AccountsWords.servicesTitle) {
            ForEach(session.serviceRows, id: \.service) { r in
                SettingsRow(ax: "\(P).service-status.\(r.service.rawValue)", title: r.service.name,
                            dot: r.state == .needsSignIn || r.state == .unreachable ? WhooshPalette.warning.color(scheme) : nil) {
                    Text(r.state.words).font(.system(size: 13)).foregroundStyle(WhooshPalette.secondaryLabel)
                }
            }
        }
    }

    private var working: some View {
        let rest = session.sections.map { (title: $0.title, accounts: $0.accounts.filter { a in session.state(a.id)?.lastError == nil }) }.filter { !$0.accounts.isEmpty }
        let total = rest.reduce(0) { $0 + $1.accounts.count }
        return VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: WhooshSpacing.s8) {
                Text(AccountsWords.workingCount(total)).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label)
                Spacer(minLength: WhooshSpacing.s8)
                if session.accounts.count > 6 { AccountsFind(text: $session.query) }
            }
            .padding(.horizontal, WhooshSpacing.s4).padding(.bottom, WhooshSpacing.s6).padding(.top, WhooshSpacing.s16)
            if rest.isEmpty {
                Text(AccountsWords.findNothing).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                    .padding(.horizontal, WhooshSpacing.s4).whooshAx("\(P).find.empty")
            }
            ForEach(Array(rest.enumerated()), id: \.offset) { _, section in
                VStack(alignment: .leading, spacing: 0) {
                    if let title = section.title {
                        HStack(spacing: WhooshSpacing.s8) {
                            Text(title).font(.system(size: 12, weight: .semibold)).foregroundStyle(WhooshPalette.secondaryLabel)
                            Text(AccountsWords.groupCount(section.accounts.count)).font(.system(size: 11)).foregroundStyle(WhooshPalette.tertiaryLabel)
                        }
                        .padding(.horizontal, WhooshSpacing.s4).padding(.bottom, WhooshSpacing.s6).padding(.top, WhooshSpacing.s8)
                        .accessibilityElement(children: .contain).accessibilityIdentifier("\(P).group.\(title)")
                    }
                    SettingsCard { ForEach(section.accounts) { a in AccountRowView(session: session, account: a, needsYou: false) } }
                }
            }
        }
        .accessibilityElement(children: .contain)
        .whooshAx("\(P).accounts")
    }
}

/// `.filt` a quiet search field: a fill, a hairline, the system magnifier.
private struct AccountsFind: View {
    @Binding var text: String
    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: "magnifyingglass").font(.system(size: 11)).foregroundStyle(WhooshPalette.tertiaryLabel)
            TextField(AccountsWords.findPlaceholder, text: $text).textFieldStyle(.plain).font(.system(size: 12)).foregroundStyle(WhooshPalette.label)
                .whooshAx("\(P).find")
        }
        .padding(.horizontal, 8).frame(width: 200, height: 24)
        .background(RoundedRectangle(cornerRadius: 7, style: .continuous).fill(WhooshPalette.fill))
        .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: 0.5))
        .settingsRing("\(P).find", radius: 7)
    }
}

/// The one button shape of the picker: kept for its callers, drawn by the window's button.
struct AccountsPill: View {
    let title: String
    let filled: Bool
    let id: String
    let action: () -> Void
    var body: some View { SettingsButton(title: title, primary: filled, ax: id, action: action) }
}
#endif
