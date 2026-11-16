import SwiftUI

/// The iPhone Settings: the Mailboxes screen's foot sets `overlay = "iphone-settings"`, and the host draws this view for that slot
/// (`SettingsPhoneFeature.view(for:)`, slot id "iphone-settings"). One navigation stack: the home list, one page per row.
public struct SettingsPhoneRoot: View {
    private let m = AppModel.shared
    private let state = SettingsPhoneState.shared
    @State private var store = GeneralStore.shared

    public init() {}

    public var body: some View {
        NavigationStack(path: Bindable(state).path) {
            SettingsPhoneHome()
                .navigationDestination(for: SettingsPhonePage.self) { destination($0) }
        }
        .preferredColorScheme(scheme)
        .onAppear { m.surface = "iphone-settings" }
    }

    @ViewBuilder private func destination(_ page: SettingsPhonePage) -> some View {
        switch page {
        case .accounts: SettingsPhoneAccountsPage()
        case .sync: SettingsPhoneSyncPage()
        case .ai: SettingsPhoneAIPage()
        case .language: SettingsPhoneLanguagePage()
        case .appearance: SettingsPhoneAppearancePage()
        }
    }

    /// Appearance is applied here: `GeneralStore.apply` restyles the Mac's windows only, so the phone's screen takes the pick itself.
    private var scheme: ColorScheme? {
        switch store.appearance { case .auto: return nil; case .light: return .light; case .dark: return .dark }
    }
}
