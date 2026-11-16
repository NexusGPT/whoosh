import SwiftUI

/// Settings > Decisions: the decision model and what it files; one page of the list opens in place (back returns).
struct DecisionsPaneView: View {
    @State private var store = AISettingsStore.shared

    var body: some View {
        SettingsPaneScaffold(pane: .decisions) {
            if case .page(let id) = store.section {
                DecisionPageView(store: store, pageId: id).id(id).transition(.opacity)
            } else {
                DecisionsHomeView(store: store).transition(.opacity)
            }
        }
        .animation(.easeOut(duration: WhooshMotion.readerCrossFade), value: store.section)
        .animation(.easeOut(duration: WhooshMotion.rowInsert), value: store.previews.count)
        .onAppear {
            store.seedLabels()
            switch store.section { case .providers, .agents: store.section = .decisions; default: break }
        }
        .accessibilityIdentifier("mac-settings-ai")
    }
}

/// Settings > AI and agents: the providers (keys), speech, and the agents you allow.
struct AIAgentsPaneView: View {
    @State private var store = AISettingsStore.shared

    var body: some View {
        SettingsPaneScaffold(pane: .ai) {
            ProvidersView(store: store)
            SettingsGroup(title: AIWords.searchGroup) { MeaningSetupRow() }
            AgentsView(store: store)
        }
        .animation(.easeOut(duration: WhooshMotion.rowInsert), value: store.agents.count)
        .onAppear {
            store.syncFromCore()
            switch store.section { case .providers, .agents: break; default: store.section = .providers }
        }
        .accessibilityIdentifier("mac-settings-ai.agents")
    }
}
