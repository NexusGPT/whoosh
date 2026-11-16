import SwiftUI

private let D = "mac-settings-ai"

/// The rows a check reads for the "new agent" row: the two words the row draws (its title and its button), never typed twice.
/// Registered by `SettingsAIMacFeature.rowSources()` as `"\(D).agents.new.row": { _ in AgentsRows.newRow() }` (that file is outside this lane's files).
enum AgentsRows {
    static func newRow() -> [ControlRow] { [["title": AIWords.agentsYouMade, "button": AIWords.newAgent]] }
}

/// Agents: a card per agent naming what it may do, in words. "New agent" with no provider says which is missing.
struct AgentsView: View {
    let store: AISettingsStore
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            SettingsGroup(title: AIWords.navAgents, ax: "\(D).agents.section") {
                SettingsRow(ax: "\(D).agents.new.row", title: AIWords.agentsYouMade) {
                    SettingsButton(title: AIWords.newAgent, symbol: "plus", primary: true, ax: "\(D).agents.new") { store.newAgent() }
                }
                if let missing = store.formMissing {
                    SettingsRow(ax: "\(D).agents.form", title: missing, captionStrong: true) {
                        SettingsButton(title: AIWords.goToProviders, ax: "\(D).agents.form.link") { store.section = .providers }
                    }
                    .transition(.opacity)
                }
            }
            if let draft = store.draftAgent { AgentCard(store: store, agent: draft, editing: true, slot: 1) }
            ForEach(Array(store.agents.enumerated()), id: \.element.id) { AgentCard(store: store, agent: $1, editing: false, slot: $0 + 1 + (store.draftAgent == nil ? 0 : 1)) }
        }
    }
}

private struct AgentCard: View {
    let store: AISettingsStore
    let agent: AgentConfig
    let editing: Bool
    let slot: Int
    private let all: [AgentPermission] = [.readMail, .search, .draft, .label]
    var body: some View {
        SettingsGroup {
            VStack(alignment: .leading, spacing: WhooshSpacing.s12) {
                HStack {
                    if editing {
                        TextField(AIWords.agentName, text: Binding(get: { agent.name }, set: { v in store.mutateAgent(agent.id) { $0.name = v } }))
                            .textFieldStyle(.plain).font(.system(size: 15, weight: .semibold))
                            .accessibilityIdentifier("\(D).agents.name")
                    } else {
                        Text(agent.name).font(.system(size: 15, weight: .semibold)).foregroundStyle(WhooshPalette.label)
                    }
                    Spacer()
                    ModelMenu(store: store, agent: agent, slot: slot)
                }
                if let why = store.agentRefusals[agent.id] {
                    Text(why).font(.system(size: 12.5)).foregroundStyle(WhooshPalette.label).accessibilityIdentifier("\(D).agents.model.\(slot).refusal")
                }
                Text(sentence).font(.system(size: 12.5)).foregroundStyle(WhooshPalette.secondaryLabel).contentTransition(.opacity)
                HStack(spacing: WhooshSpacing.s6) {
                    ForEach(all, id: \.self) { p in
                        let on = agent.allowed.contains(p)
                        Button { store.toggle(p, on: agent.id) } label: { AIChip(text: AIWords.words(p), tone: on ? .accent : .neutral) }
                            .buttonStyle(.plain)
                            .accessibilityIdentifier("\(D).agents.allow.\(slug(p))")
                    }
                    Spacer()
                    if editing {
                        AIButton(title: AIWords.save, primary: true, id: "\(D).agents.save") { store.saveDraft() }
                    } else {
                        AIButton(title: AIWords.duplicate, id: "\(D).agents.duplicate") { store.duplicate(agent) }
                    }
                }
            }
            .padding(SettingsMetrics.rowSideX)
        }
        .accessibilityIdentifier("\(D).agents.card")
    }
    private var sentence: String {
        agent.allowed.isEmpty ? AIWords.mayDoNothing :
            "\(AIWords.allowed): " + agent.allowed.map(AIWords.words).joined(separator: ", ") + "."
    }
}

func slug(_ p: AgentPermission) -> String {
    switch p { case .readMail: "read"; case .search: "search"; case .draft: "draft"; case .label: "label"; case .sendWithConfirmation: "send" }
}

/// The agent's model: a pop-up pill with the provider's mark. The menu lists what each connected key opens, the current one ticked;
/// a provider with no key is greyed. A choice saves the agent at once.
private struct ModelMenu: View {
    let store: AISettingsStore
    let agent: AgentConfig
    let slot: Int
    private var providerName: String { store.providers.first { $0.id == agent.providerId }?.name ?? agent.providerId }
    var body: some View {
        Menu {
            ForEach(store.providers, id: \.id) { p in
                Section(p.name) {
                    let offered = store.modelChoices().filter { $0.provider.id == p.id }
                    ForEach(Array(offered.enumerated()), id: \.offset) { _, c in
                        if let m = c.model {
                            Button { store.setModel(agent.id, to: c) } label: {
                                if p.id == agent.providerId && m == agent.model { Label(m, systemImage: "checkmark") } else { Text(m) }
                            }
                        } else {
                            Button(AIWords.addKeyAbove) {}.disabled(true)
                        }
                    }
                }
            }
        } label: {
            PopPill {
                HStack(spacing: WhooshSpacing.s6) {
                    if let mark = ProviderMark.of(agent.providerId) { ProviderMarkView(mark: mark, side: 16, probe: false) }
                    Text("\(providerName) · \(agent.model)")
                }
            }
        }
        .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
        .accessibilityIdentifier("\(D).agents.model.\(slot)")
    }
}
