import SwiftUI

private let D = "mac-settings-ai"

/// Providers: one row per provider; paste a key, the provider is asked, its models appear. Then speech.
struct ProvidersView: View {
    let store: AISettingsStore
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            SettingsGroup(title: AIWords.navProviders, ax: "\(D).providers.section") {
                ForEach(store.providers, id: \.id) { p in ProviderRow(store: store, provider: p) }
                ChatGPTRow()
            }
            SettingsFootnote(text: AIWords.providersLead)
            SpeechGroup(store: store)
        }
    }
}

private struct ProviderRow: View {
    let store: AISettingsStore
    let provider: AiProvider
    var body: some View {
        SettingsRow(ax: "\(D).provider.\(provider.id)", title: provider.name,
                    caption: store.refusals[provider.id] ?? (provider.connected ? provider.models.joined(separator: " · ") : nil),
                    captionStrong: store.refusals[provider.id] != nil,
                    leading: ProviderMark.of(provider.id).map { AnyView(ProviderMarkView(mark: $0)) }) {
            HStack(spacing: 8) {
                AIChip(text: store.state(of: provider.id), tone: store.isChecking(provider.id) ? .neutral : provider.connected ? .good : (store.refusals[provider.id] != nil ? .warn : .neutral))
                if !provider.connected && !store.isChecking(provider.id) {
                    SettingsField(ax: "\(D).provider.\(provider.id).key", placeholder: AIWords.keyPlaceholder, text: draft, secure: true, width: 180, mono: true) { store.connect(provider.id) }
                    AIButton(title: AIWords.connect, primary: !(store.keyDrafts[provider.id] ?? "").isEmpty, id: "\(D).provider.\(provider.id).connect") { store.connect(provider.id) }
                }
            }
        }
    }
    private var draft: Binding<String> {
        Binding(get: { store.keyDrafts[provider.id] ?? "" }, set: { store.keyDrafts[provider.id] = $0 })
    }
}

private struct ChatGPTRow: View {
    var body: some View {
        SettingsRow(ax: "\(D).provider.chatgpt", title: AIWords.chatgptSignIn, caption: AIWords.chatgptNote,
                    leading: AnyView(ProviderMarkView(mark: .chatgpt))) { AIChip(text: ChatGPTState.words()) }
    }
}

/// The state words of the ChatGPT row: read from the sign-in controller, so a successful sign-in shows "connected".
enum ChatGPTState {
    @MainActor static func words(for controller: SignInController = .shared) -> String {
        controller.chatgptConnected ? AIWords.connected : AIWords.notConnected
    }
}
