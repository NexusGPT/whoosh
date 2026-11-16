import SwiftUI

private let D = "mac-settings-ai"

/// Dictation: the engine as a pop-up pill, its state in one line under it, and the line that says how to use it, with the key drawn as a key cap.
struct SpeechGroup: View {
    /// The card's title and its row's title: ONE place, drawn below and served by the `mac-settings-ai.speech` row (`title`, `row_title`) so a check reads them.
    static var title: String { AIWords.dictationTitle }
    static var rowTitle: String { AIWords.speechToText }
    let store: AISettingsStore
    var body: some View {
        SettingsGroup(title: Self.title, ax: "\(D).speech") {
            VStack(alignment: .leading, spacing: 0) {
                SettingsRow(ax: "\(D).speech.row", title: Self.rowTitle) { EnginePill(store: store) }
                DictationLine(store: store)
            }
        }
    }
}

private struct EnginePill: View {
    let store: AISettingsStore
    private var name: String {
        store.speechKind == "local" ? AIWords.speechLocal : (store.providers.first { $0.id == store.speechKind }?.name ?? store.speechKind)
    }
    var body: some View {
        VStack(alignment: .trailing, spacing: WhooshSpacing.s2) {
            Menu {
                Button { store.chooseSpeech(provider: nil) } label: { choice(AIWords.speechLocal, on: store.speechKind == "local") }
                ForEach(store.providers.filter(\.connected), id: \.id) { p in
                    Button { store.chooseSpeech(provider: p.id) } label: { choice(p.name, on: store.speechKind == p.id) }
                }
            } label: { PopPill { Text(name) } }
                .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
                .accessibilityIdentifier("\(D).speech.engine")
            if let line { Text(line).font(.system(size: 12)).foregroundStyle(store.speech == .failed ? WhooshPalette.label : WhooshPalette.secondaryLabel).contentTransition(.opacity) }
        }
    }
    @ViewBuilder private func choice(_ title: String, on: Bool) -> some View {
        if on { Label(title, systemImage: "checkmark") } else { Text(title) }
    }
    private var line: String? {
        switch store.speech {
        case .ready: AIWords.speechStateReady
        case .downloading: AIWords.speechStateDownloading
        case .failed: store.speechWords ?? AIWords.speechFailed
        case .none: nil
        }
    }
}

private struct DictationLine: View {
    let store: AISettingsStore
    private let marker = "\u{1}"
    var body: some View {
        HStack(spacing: WhooshSpacing.s4) {
            if let key = store.dictationKey {
                let parts = AIWords.holdLine(marker).components(separatedBy: marker)
                Text(parts.first ?? "")
                KeyCap(key)
                Text(parts.count > 1 ? parts[1] : "")
            } else {
                Text(AIWords.dictNoKey)
                SettingsButton(title: AIWords.openShortcuts, ax: "\(D).speech.shortcuts") { store.openShortcuts() }
            }
            Spacer(minLength: 0)
        }
        .font(.system(size: 12.5)).foregroundStyle(WhooshPalette.secondaryLabel)
        .padding(.horizontal, SettingsMetrics.rowSideX).padding(.bottom, WhooshSpacing.s12)
        .accessibilityIdentifier("\(D).speech.line")
    }
}
