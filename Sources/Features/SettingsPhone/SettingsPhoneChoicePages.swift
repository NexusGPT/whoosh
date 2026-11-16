import SwiftUI

/// Language: System, then the catalogue's languages by their own names. A pick is `GeneralStore.setLanguage`, the call the Mac's
/// pane makes: `Words` stores it and every string of the app answers in it at once.
struct SettingsPhoneLanguagePage: View {
    private let state = SettingsPhoneState.shared
    @State private var store = GeneralStore.shared

    /// System first, then each language under its own name: (code, name).
    @MainActor static var choices: [(code: String, name: String)] {
        [(Words.system, SettingsPhoneWords.system)] + Words.languages.map { ($0.code, $0.name) }
    }

    var body: some View {
        SettingsPhoneFrame(title: SettingsPhoneWords.language, titleAx: "iphone-settings.language.title", onBack: { _ = state.back() }) {
            SettingsPhoneCard {
                ForEach(Self.choices, id: \.code) { c in
                    SettingsPhoneChoice(title: c.name, on: store.language == c.code, ax: "iphone-settings.language.\(c.code)") { store.setLanguage(c.code) }
                    if c.code != Self.choices.last?.code { SettingsPhoneDivider() }
                }
            }
            if let why = store.errors[.language] {
                SettingsPhoneNote(text: why).accessibilityIdentifier("iphone-settings.language.error")
            }
        }
    }
}

/// Appearance: Auto, Light, Dark. A pick is `GeneralStore.setAppearance`, the record the Mac reads; the screen's root applies it.
struct SettingsPhoneAppearancePage: View {
    private let state = SettingsPhoneState.shared
    @State private var store = GeneralStore.shared

    var body: some View {
        SettingsPhoneFrame(title: SettingsPhoneWords.appearance, titleAx: "iphone-settings.appearance.title", onBack: { _ = state.back() }) {
            SettingsPhoneCard {
                ForEach(GeneralStore.Appearance.allCases, id: \.rawValue) { a in
                    SettingsPhoneChoice(title: SettingsPhoneWords.theme(a.rawValue), on: store.appearance == a, ax: "iphone-settings.appearance.\(a.rawValue)") { store.setAppearance(a) }
                    if a != GeneralStore.Appearance.allCases.last { SettingsPhoneDivider() }
                }
            }
            if let why = store.errors[.appearance] {
                SettingsPhoneNote(text: why).accessibilityIdentifier("iphone-settings.appearance.error")
            }
        }
    }
}
