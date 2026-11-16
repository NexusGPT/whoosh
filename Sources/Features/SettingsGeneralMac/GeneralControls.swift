import SwiftUI

/// The language pop-up: "System · <the Mac's language>" first, then the catalogue languages by their own names.
/// The choice applies at once to every window.
struct GeneralLanguagePopup: View {
    let store: GeneralStore

    static var options: [(code: String, name: String)] {
        [(Words.system, GeneralWords.languageSystem)] + Words.languages.sorted { $0.code < $1.code }.map { ($0.code, $0.name) }
    }
    /// "System · English": the Mac's own language beside the word System.
    static var systemTitle: String {
        let mac = Words.languages.first { $0.code == Words.current }?.name
        return mac.map { GeneralWords.languageSystem + " · " + $0 } ?? GeneralWords.languageSystem
    }

    var body: some View {
        SettingsPopup(ax: "mac-settings-general.appearance.language",
                      options: Self.options.map { (value: $0.code, title: $0.code == Words.system ? Self.systemTitle : $0.name) },
                      selection: store.language) { store.setLanguage($0) }
            .help(GeneralWords.languageHint)
    }
}

/// The Auto, Light and Dark pictures: three small windows, the chosen one ringed in blue.
struct GeneralAppearancePicker: View {
    let store: GeneralStore
    var body: some View {
        HStack(spacing: 12) {
            ForEach(GeneralStore.Appearance.allCases, id: \.self) { a in
                let on = store.appearance == a
                Button { store.setAppearance(a) } label: {
                    VStack(spacing: 4) {
                        MiniWindow(kind: a)
                            .frame(width: SettingsMetrics.miniSize.width, height: SettingsMetrics.miniSize.height)
                            .padding(2)
                            .overlay(RoundedRectangle(cornerRadius: SettingsMetrics.miniSize.height / 5 + 2, style: .continuous)
                                .strokeBorder(WhooshPalette.accent, lineWidth: 2).opacity(on ? 1 : 0))
                        Text(GeneralWords.theme(a)).font(.system(size: 11, weight: on ? .semibold : .regular))
                            .foregroundStyle(on ? WhooshPalette.label : WhooshPalette.secondaryLabel)
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .animation(.easeOut(duration: WhooshMotion.hoverTint), value: on)
                .accessibilityLabel(GeneralWords.theme(a))
                .accessibilityAddTraits(on ? .isSelected : [])
                .whooshAx("mac-settings-general.appearance.theme.\(a.rawValue)")
            }
        }
    }
}

/// `.mini` a window drawn small: a sidebar bar and a blue line, on the light or the dark ground (Auto is both halves).
private struct MiniWindow: View {
    let kind: GeneralStore.Appearance
    private let light = Color(.sRGB, red: 0.957, green: 0.957, blue: 0.965)
    private let dark = Color(.sRGB, red: 0.149, green: 0.149, blue: 0.165)
    var body: some View {
        let shape = RoundedRectangle(cornerRadius: SettingsMetrics.popupRadius, style: .continuous)
        ZStack(alignment: .topLeading) {
            switch kind {
            case .light: light
            case .dark: dark
            case .auto: HStack(spacing: 0) { light; dark }
            }
            RoundedRectangle(cornerRadius: 3).fill(Color.gray.opacity(0.35)).frame(width: 14).padding(5)
            RoundedRectangle(cornerRadius: 2).fill(Color(.sRGB, red: 0, green: 0.478, blue: 1).opacity(0.85))
                .frame(height: 4).padding(.leading, 23).padding(.trailing, 5).padding(.top, 8)
        }
        .clipShape(shape)
        .overlay(shape.strokeBorder(WhooshPalette.separator, lineWidth: 0.5))
    }
}
