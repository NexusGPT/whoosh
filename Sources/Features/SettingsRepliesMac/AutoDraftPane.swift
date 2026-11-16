import SwiftUI

/// Brouillon automatique: one column, three groups (the board's round 7 rework, "cleaner and clearer"). Which messages (the entry filter, the AI
/// decision, the exit filter, each with what it kept these 7 days), with what (the sources), how (the tone and the prompt).
struct AutoDraftPane: View {
    @State private var store = RepliesStore.shared
    private let P = RepliesAx.id("auto")

    var body: some View {
        let use = store.use(.auto)
        RepliesScaffold(page: .auto) {
            HStack(spacing: WhooshSpacing.s10) {
                Text(RepliesWords.enabled).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                SettingsSwitch(on: use?.enabled ?? false, refusals: store.refusals)
                    .contentShape(Rectangle())
                    .onTapGesture { store.setEnabled(.auto, !(use?.enabled ?? false)) }
                    .accessibilityElement().accessibilityLabel(RepliesWords.enabled)
                    .accessibilityAddTraits(.isButton).accessibilityValue(use?.enabled == true ? RepliesWords.on : RepliesWords.off)
            }
            .whooshAx("\(P).enabled")
        } content: {
            RepliesGroup(title: RepliesWords.forWhich, link: (RepliesWords.tryOnMessage, "\(P).try", { store.tryOnMessage(.auto) })) {
                steps
            }
            .popover(isPresented: trialShown, arrowEdge: .trailing) { TrialReportView(report: store.trial) }
            footnote
            UseSourcesGroup(kind: .auto)
            UseHowGroup(kind: .auto)
        }
    }

    private var trialShown: Binding<Bool> {
        Binding(get: { store.popover == .trial }, set: { if !$0 { store.popover = nil } })
    }

    @ViewBuilder private var steps: some View {
        let f = store.filter, c = store.counts
        stepRow("entry", "line.3.horizontal.decrease", ai: false, RepliesWords.entryFilter,
                f.map { RepliesFacts.summary($0.entryRule, labels: RepliesWords.scopeLine($0.scope, store.labels)) } ?? "",
                c.map { RepliesWords.kept($0.afterEntry, of: $0.received) }, on: f?.entryEnabled ?? true)
        stepRow("ai", "sparkle", ai: true, RepliesWords.aiDecision, f?.aiDescription ?? "",
                c.map { RepliesWords.kept($0.afterAi, of: $0.afterEntry) }, on: f?.aiEnabled ?? true)
        stepRow("exit", "line.3.horizontal.decrease", ai: false, RepliesWords.exitFilter,
                f.map { RepliesFacts.summary($0.exitRule, labels: nil) } ?? "",
                c.map { RepliesWords.kept($0.afterExit, of: $0.afterAi) }, on: f?.exitEnabled ?? true)
    }

    /// One line of « Pour quels messages »: a click opens the page « Quels messages » (one page holds the three cards).
    private func stepRow(_ key: String, _ symbol: String, ai: Bool, _ title: String, _ caption: String, _ count: String?, on: Bool) -> some View {
        SettingsRow(ax: "\(P).step.\(key)", title: title, caption: on ? caption : RepliesWords.cardOff,
                    leading: AnyView(RepliesRowIcon(symbol: symbol, ai: ai)), onTap: { RepliesRouter.go(.filter) }) {
            RepliesValue(text: on ? (count ?? "") : RepliesWords.off)
        }
        .opacity(on ? 1 : 0.55)
    }

    @ViewBuilder private var footnote: some View {
        if let c = store.counts {
            Text(Self.foot(c))
                .whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                .padding(.horizontal, WhooshSpacing.s4).padding(.top, WhooshSpacing.s8)
                .contentTransition(.numericText())
                .whooshAx("\(P).foot")
        }
    }

    /// The board's foot: the week's figures, the drafts in the label colour and bold.
    static func foot(_ c: ReplyStepCounts) -> AttributedString {
        var out = AttributedString(RepliesWords.lastDays(c.days, received: c.received, drafts: c.drafts))
        if let r = out.range(of: RepliesWords.drafts(c.drafts)) {
            out[r].inlinePresentationIntent = .stronglyEmphasized
            out[r].foregroundColor = Color.primary   // the label colour of either theme, over the line's grey
        }
        return out
    }
}

/// « Avec quoi »: the one line that says how many sources are on, and opens Sources (sources are chosen in ONE place, owner round 7).
struct UseSourcesGroup: View {
    let kind: ReplyUseKind
    @State private var store = RepliesStore.shared
    var body: some View {
        let P = RepliesAx.id(kind == .auto ? "auto" : "on-demand")
        RepliesGroup(title: RepliesWords.withWhat) {
            SettingsRow(ax: "\(P).sources", title: RepliesWords.sources, onTap: { RepliesRouter.go(.sources) }) {
                RepliesValue(text: RepliesWords.sourcesOn(store.sources.enabledTotal))
            }
        }
    }
}

/// « Comment »: the tone (my voice or another tone), writing like my earlier replies in the thread, and the prompt.
struct UseHowGroup: View {
    let kind: ReplyUseKind
    @State private var store = RepliesStore.shared
    var body: some View {
        let P = RepliesAx.id(kind == .auto ? "auto" : "on-demand")
        let use = store.use(kind)
        RepliesGroup(title: RepliesWords.how) {
            SettingsRow(ax: "\(P).tone", title: RepliesWords.tone) {
                SettingsPopup(ax: "\(P).tone.popup", options: [("voice", RepliesWords.myVoice)] + store.tones.map { ($0.id, $0.name) },
                              selection: use?.toneId ?? "voice") { store.setTone(kind, $0) }
            }
            SettingsRow(ax: "\(P).match-thread", title: RepliesWords.matchThread,
                        onTap: { store.setMatchThread(kind, !(use?.matchThread ?? false)) }) {
                SettingsSwitch(on: use?.matchThread ?? false, refusals: store.refusals)
            }
            SettingsRow(ax: "\(P).prompt", title: RepliesWords.prompt, onTap: { RepliesRouter.go(.prompt) }) {
                RepliesValue(text: RepliesWords.repliesPrompt)
            }
        }
    }
}
