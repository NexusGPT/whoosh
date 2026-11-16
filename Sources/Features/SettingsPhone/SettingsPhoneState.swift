import SwiftUI
import Observation

/// The pages the home list pushes. Each is one row of the home list and one file of this folder.
enum SettingsPhonePage: String, CaseIterable, Hashable, Identifiable {
    case accounts, sync, ai, language, appearance
    var id: String { rawValue }

    /// The coloured square's symbol and hue: the Mac's own for the panes it has (SettingsPane), Sync's cloud in green.
    var symbol: String {
        switch self {
        case .accounts: return SettingsPane.accounts.symbol
        case .sync: return "icloud"
        case .ai: return SettingsPane.ai.symbol
        case .language: return "globe"
        case .appearance: return "circle.lefthalf.filled"
        }
    }
    var hue: ThemeColor {
        switch self {
        case .accounts: return SettingsPane.accounts.hue
        case .sync: return SettingsHue.green
        case .ai: return SettingsPane.ai.hue
        case .language: return SettingsHue.blue
        case .appearance: return SettingsHue.indigo
        }
    }
    @MainActor var title: String {
        switch self {
        case .accounts: return SettingsPhoneWords.accounts
        case .sync: return SettingsPhoneWords.sync
        case .ai: return SettingsPhoneWords.ai
        case .language: return SettingsPhoneWords.language
        case .appearance: return SettingsPhoneWords.appearance
        }
    }
}

/// The home list's groups, in the order the screen draws them: Mail, Sync, AI, General.
struct SettingsPhoneGroup: Identifiable {
    let id: String
    let pages: [SettingsPhonePage]
    @MainActor var title: String {
        switch id {
        case "mail": return SettingsPhoneWords.mail
        case "sync": return SettingsPhoneWords.sync
        case "ai": return SettingsPhoneWords.ai
        default: return SettingsPhoneWords.general
        }
    }
    static let all = [
        SettingsPhoneGroup(id: "mail", pages: [.accounts]),
        SettingsPhoneGroup(id: "sync", pages: [.sync]),
        SettingsPhoneGroup(id: "ai", pages: [.ai]),
        SettingsPhoneGroup(id: "general", pages: [.language, .appearance]),
    ]
}

/// What the screen holds beyond the core: the page stack and the count of Retry presses. A setting itself is never kept here:
/// language and appearance are `GeneralStore`'s (the record the Mac reads), the accounts are `AccountsSession`'s, the AI needs
/// are `AISetupStatus`'s, the sync line is the core's `settings_sync/*` state (SettingsPhoneSync).
@MainActor @Observable
final class SettingsPhoneState {
    static let shared = SettingsPhoneState()

    var path: [SettingsPhonePage] = []
    /// Counts Retry presses; the Sync page reads it so a press redraws it on the input frame.
    private(set) var retries = 0

    var current: SettingsPhonePage? { path.last }

    func open(_ page: SettingsPhonePage) { if path.last != page { path.append(page) } }
    @discardableResult func back() -> Bool { guard !path.isEmpty else { return false }; path.removeLast(); return true }

    /// Closes the screen: the stack empties, the overlay the Mailboxes screen set goes away.
    func close(_ m: AppModel) { path = []; m.overlay = nil; m.surface = "iphone-mailboxes" }

    /// Retry reads the sync state and every account's sync state again. The core has no "sync now" command yet (finding in the notes),
    /// so a press asks the core again and the line shows what the core answers.
    func retry(_ m: AppModel) {
        retries += 1
        AccountsSession.shared.refreshStates()
        m.refresh()
    }
}
