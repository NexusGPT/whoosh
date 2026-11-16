import SwiftUI

private let D = "mac-settings-ai"

/// Settings > Decisions: the decision model on top (provider Laya, Jev or Off, the key), then what it files, one row per page.
struct DecisionsHomeView: View {
    let store: AISettingsStore
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            EngineBlock(store: store)
            SettingsGroup(title: AIWords.filesTitle) {
                ForEach(store.pages) { p in PageRow(page: p) { store.section = .page(p.id) } }
            }
            .accessibilityIdentifier("\(D).decisions.pages")
            SettingsFootnote(text: AIWords.decisionsLead)
        }
        .onAppear { store.layaStateChanged() }   // opening the page reads the disk: the line never starts from a guess
    }
}

private struct PageRow: View {
    let page: DecisionPage
    let open: () -> Void
    var body: some View {
        SettingsRow(ax: "\(D).decisions.open.\(page.id)", title: Words.text(page.title), caption: summary, onTap: open) {
            HStack(spacing: 8) {
                Text(AIWords.entries(page.entries.count)).font(.system(size: 11.5)).foregroundStyle(WhooshPalette.secondaryLabel)
                Image(systemName: "chevron.right").font(.system(size: 11, weight: .semibold)).foregroundStyle(WhooshPalette.tertiaryLabel)
            }
        }
    }
    private var summary: String {
        page.entries.map { e in
            switch e.kind {
            case .rule: "\(AIWords.ifWord) \(Words.text(e.text)) → \(Words.text(e.outcome))"
            case .model: AIWords.askTheModel
            case .default: "\(AIWords.otherwise) \(Words.text(e.outcome))"
            }
        }.joined(separator: " · ")
    }
}

/// Where every "ask the model" entry runs: Laya on this Mac, Jev (with a key), or Off. The core holds the choice; this draws it.
struct EngineBlock: View {
    let store: AISettingsStore
    private var engine: AISettingsStore.Engine { store.shownEngine }
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            SettingsGroup(title: AIWords.decisionModel, ax: "\(D).decisions.engine") {
                SettingsRow(ax: "\(D).decisions.engine.row", title: AIWords.providerTitle, caption: store.engineRefusal ?? line(engine), captionStrong: store.engineRefusal != nil) {
                    SettingsSegmented(axPrefix: "\(D).decisions.engine",
                                      options: AISettingsStore.Engine.allCases.map { (value: $0, key: $0.rawValue, title: title($0)) },
                                      selection: engine) { store.chooseEngine($0) }
                }
                switch engine {
                case .local:
                    // the owner's pick M1: one line per model the app can run (the catalogue's, then the added ones), the filled circle on the model in
                    // use, and last the line "Other model" that adds one
                    // the card draws the line between two of its rows itself: a second one here made the lines between models 2.5 pt thick
                    // and ran them to the card's edge (lead's read of the main192 pictures, 15 Nov)
                    ForEach(store.modelLines, id: \.id) { entry in DecisionModelRow(entry: entry, store: store) }
                    OtherModelLine(store: store)
                case .jev: JevRows(store: store)
                case .off: EmptyView()
                }
            }
            if engine == .jev { SettingsFootnote(text: AIWords.jevPrivacy) }
            if engine == .local, inUseReady { nextNote }
        }
    }
    /// The model in use is on this Mac: the note under the card says what happens next (board `.quiet`).
    private var inUseReady: Bool {
        if case .ready = store.modelState(DecisionModels.shared.line(id: store.chosenModel)) { return true } else { return false }
    }
    private var nextNote: some View {
        let parts = AIWords.layaNext.components(separatedBy: "%@")
        return KeySentence(before: parts.first ?? "", key: "R", after: parts.count > 1 ? parts[1] : "")
            .font(.system(size: 11.5)).foregroundStyle(WhooshPalette.secondaryLabel)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, WhooshSpacing.s4).padding(.top, WhooshSpacing.s8)
            .accessibilityIdentifier("\(D).decisions.local.next")
    }
    private func title(_ e: AISettingsStore.Engine) -> String { [.local: AIWords.engineLocal, .jev: AIWords.engineJev, .off: AIWords.engineOff][e]! }
    private func line(_ e: AISettingsStore.Engine) -> String {
        if e == .local {
            let model = DecisionModels.shared.line(id: store.chosenModel)
            if model.family != .laya { return AIWords.engineLocalModelLine(model.name) }   // a model the owner chose by name answers every question itself
        }
        if e == .local, AISettingsStore.labelsByApple { return AIWords.engineLocalAppleLine }   // round 12: the owner sees which model sorts the labels
        return [.local: AIWords.engineLocalLine, .jev: AIWords.engineJevLine, .off: AIWords.engineOffLine][e]!
    }
}

/// A sentence with a key cap inside it that wraps as text does: the words flow and the cap sits between them (lead, 13 Nov, picture of the Ready state at
/// 700x480: as three pieces side by side, the first wrapped to two lines and the cap floated apart at its right). A language written without spaces flows by
/// character. VoiceOver reads it as one sentence.
private struct KeySentence: View {
    let before: String
    let key: String
    let after: String
    var body: some View {
        let spaced = (before + after).contains(" ")
        FlowLayout(spacing: spaced ? 3.5 : 0, lineSpacing: 0) {
            ForEach(Array(tokens(before, spaced).enumerated()), id: \.offset) { Text($0.element).frame(minHeight: WhooshMetrics.keyCap) }
            KeyCap(key).padding(.horizontal, spaced ? 0 : 3)
            ForEach(Array(tokens(after, spaced).enumerated()), id: \.offset) { Text($0.element).frame(minHeight: WhooshMetrics.keyCap) }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(before + key + after)
    }
    private func tokens(_ s: String, _ spaced: Bool) -> [String] {
        spaced ? s.split(separator: " ").map(String.init) : s.filter { !$0.isWhitespace }.map(String.init)
    }
}

extension ModelPhase {
    private static func size(_ n: Int64) -> String { AttachmentSize.words(n) }
    static func of(_ s: LayaModelState, expected: Int64) -> ModelPhase {
        switch s {
        case .ready: return .ready
        case .notDownloaded: return .missing(line: AIWords.notOnThisMac(size(expected)))
        case .failed(let why): return .failed(line: why)
        case .downloading(let r, let t):
            let total = t > 0 ? t : expected
            return .downloading(line: AIWords.downloadingOf(size(r), size(total)), fraction: total > 0 ? Double(r) / Double(total) : nil)
        }
    }
    static func of(_ s: MeaningModelState, expected: Int64) -> ModelPhase {
        switch s {
        case .ready: return .ready
        case .notDownloaded: return .missing(line: AIWords.notOnThisMac(size(expected)))
        case .failed(let why): return .failed(line: why)
        case .downloading(let r, let t):
            let total = t > 0 ? t : expected
            return .downloading(line: AIWords.downloadingOf(size(r), size(total)), fraction: total > 0 ? Double(r) / Double(total) : nil)
        }
    }
}

private struct JevRows: View {
    let store: AISettingsStore
    private var key: Binding<String> { Binding(get: { store.keyDrafts["jev"] ?? "" }, set: { store.keyDrafts["jev"] = $0 }) }
    var body: some View {
        SettingsRow(ax: "\(D).decisions.jev.key", title: AIWords.jevKeyTitle, caption: store.refusals["jev"], captionStrong: store.refusals["jev"] != nil,
                    leading: AnyView(ProviderMarkView(mark: .jev))) {
            HStack(spacing: 8) {
                if !store.jevConnected {
                    SettingsField(ax: "\(D).decisions.jev.key.field", placeholder: AIWords.jevKeyPlaceholder, text: key, secure: true, width: 170, mono: true) { connect() }
                    AIButton(title: AIWords.connect, primary: !key.wrappedValue.isEmpty, id: "\(D).decisions.jev.connect") { connect() }
                }
                AIChip(text: store.jevChip, tone: store.jevState == .connected ? .good : .neutral)
                if store.jevConnected && store.jevState != .connected {   // a saved key: one fixed sample tells whether Jev takes it (DECISION-MODEL.md:63)
                    AIButton(title: AIWords.test, id: "\(D).decisions.jev.test") { store.testJev() }
                }
            }
        }
        SettingsRow(ax: "\(D).decisions.jev.consent", title: AIWords.jevConsentLine,
                    caption: store.jevConsent ? AIWords.allowed2 : nil) {
            if store.jevConsent { AIButton(title: AIWords.withdraw, id: "\(D).decisions.jev.consent.withdraw") { store.setConsent(false) } }
            else { AIButton(title: AIWords.allow, primary: true, id: "\(D).decisions.jev.consent.allow") { store.setConsent(true) } }
        }
    }
    private func connect() { let k = key.wrappedValue; key.wrappedValue = ""; store.connectJev(k) }
}
