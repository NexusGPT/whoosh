import SwiftUI

/// The iPhone Settings' registrations: rows, clicks and the screen's slot register here, never in a hub.
/// ax ids follow `<surface>.<region>[.<element>...][.<index>]` (checks/settingsphone-check.py).
/// Reached from the Mailboxes screen's foot (`overlay = "iphone-settings"`); the host draws `view(for: SurfaceSlot("iphone-settings"))`.
public enum SettingsPhoneFeature: FeatureModule {
    public static let id = "settingsphone"
    public static let surfaces: [String] = ["iphone-settings"]

    static var state: SettingsPhoneState { .shared }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        [
            "iphone-settings.nav": { m in SettingsPhoneProbe.nav(m) },
            "iphone-settings.title": { m in SettingsPhoneProbe.title(m) },
            "iphone-settings.pages": { m in SettingsPhoneProbe.pages(m) },
            "iphone-settings.summary": { m in SettingsPhoneProbe.summary(m) },
            "iphone-settings.sync": { m in SettingsPhoneProbe.sync(m) },
            "iphone-settings.accounts": { m in SettingsPhoneProbe.accounts(m) },
            "iphone-settings.notifications": { m in SettingsPhoneProbe.notifications(m) },
            "iphone-settings.language": { m in SettingsPhoneProbe.language(m) },
            "iphone-settings.appearance": { m in SettingsPhoneProbe.appearance(m) },
            "iphone-settings.ai": { m in SettingsPhoneProbe.ai(m) },
        ]
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        var c: [String: (AppModel) -> Bool] = [
            "iphone-settings.summary": { _ in state.open(.accounts); return true },
            "iphone-settings.back": { _ in state.back() },
            "iphone-settings.done": { m in state.close(m); return true },
            "iphone-settings.sync.retry": { m in
                guard SettingsPhoneSync.read(m).retry else { return false }
                state.retry(m); return true
            },
        ]
        for p in SettingsPhonePage.allCases { c["iphone-settings.row.\(p.rawValue)"] = { _ in state.open(p); return true } }
        for choice in SettingsPhoneLanguagePage.choices {
            c["iphone-settings.language.\(choice.code)"] = { _ in GeneralStore.shared.setLanguage(choice.code) }
        }
        for a in GeneralStore.Appearance.allCases {
            c["iphone-settings.appearance.\(a.rawValue)"] = { _ in GeneralStore.shared.setAppearance(a) }
        }
        return c
    }

    public static func view(for slot: SurfaceSlot) -> AnyView? {
        slot.id == "iphone-settings" ? AnyView(SettingsPhoneRoot()) : nil
    }
}
