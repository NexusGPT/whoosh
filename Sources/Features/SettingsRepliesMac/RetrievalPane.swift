import SwiftUI

/// Recherche: how Whoosh finds in the sources what it must know to reply. The prompt that writes the questions, the excerpts and what comes with
/// each, the sorting and the AI filter after (owner: "basically all retrieval strategies").
struct RetrievalPane: View {
    @State private var store = RepliesStore.shared
    private let P = RepliesAx.id("retrieval")

    var body: some View {
        RepliesScaffold(page: .retrieval) {
            SettingsButton(title: RepliesWords.tryOnMessage, ax: "\(P).try") { store.tryOnMessage(.onDemand) }
                .popover(isPresented: shown(.trial), arrowEdge: .bottom) { TrialReportView(report: store.trial) }
        } content: {
            if let r = store.retrieval {
                RepliesGroup(title: RepliesWords.questions, note: RepliesWords.retrievalPromptName, card: false) {
                    RepliesTextEditor(key: .retrievalPrompt)
                }
                RepliesGroup(title: RepliesWords.excerpts) {
                    SettingsRow(ax: "\(P).per-question", title: RepliesWords.perQuestion) {
                        SettingsPopup(ax: "\(P).per-question.popup", options: RepliesChoices.with(RepliesChoices.perQuestion, r.excerptsPerQuestion).map { ($0, "\($0)") },
                                      selection: r.excerptsPerQuestion) { store.setRetrieval(.excerptsPerQuestion(n: $0)) }
                    }
                    SettingsRow(ax: "\(P).size", title: RepliesWords.excerptSize) {
                        SettingsPopup(ax: "\(P).size.popup", options: [ExcerptSize.sentence, .paragraph, .section].map { ($0, RepliesWords.size($0)) },
                                      selection: r.excerptSize) { store.setRetrieval(.excerptSize(size: $0)) }
                    }
                    SettingsRow(ax: "\(P).fields", title: RepliesWords.attached, onTap: { store.popover = .fields }) {
                        RepliesValue(text: RepliesWords.fieldsLine(r.attachedFields))
                            .popover(isPresented: shown(.fields), arrowEdge: .trailing) { FieldsPopover(fields: r.attachedFields) }
                    }
                }
                RepliesGroup(title: RepliesWords.sorting) {
                    SettingsRow(ax: "\(P).keep", title: RepliesWords.keep) {
                        SettingsPopup(ax: "\(P).keep.popup", options: RepliesChoices.with(RepliesChoices.keep, r.keepN).map { ($0, RepliesWords.best($0)) },
                                      selection: r.keepN) { store.setRetrieval(.keepN(n: $0)) }
                    }
                    SettingsRow(ax: "\(P).clash", title: RepliesWords.clash) {
                        SettingsPopup(ax: "\(P).clash.popup", options: [ClashRule.keepBothDated, .keepNewest, .keepOldest].map { ($0, RepliesWords.clashRule($0)) },
                                      selection: r.clashRule) { store.setRetrieval(.clash(rule: $0)) }
                    }
                    SettingsRow(ax: "\(P).ai-filter", title: RepliesWords.aiFilter, onTap: { store.popover = .aiFilter }) {
                        RepliesValue(text: r.aiFilterQuestion.map { RepliesWords.quoted($0) } ?? Words.text("Off"))   // round 24: the language's own quotes
                            .popover(isPresented: shown(.aiFilter), arrowEdge: .trailing) { AiFilterPopover(question: r.aiFilterQuestion) }
                    }
                }
            } else {
                SettingsFootnote(text: RepliesWords.notReady, ax: "\(P).empty")
            }
        }
    }

    private func shown(_ p: RepliesPopover) -> Binding<Bool> {
        Binding(get: { store.popover == p }, set: { if !$0, store.popover == p { store.popover = nil } })
    }
}

/// The choices the pop-ups offer: the contract's ceilings bound them (REPLY_EXCERPTS_CEILING 20, REPLY_KEEP_CEILING 50, proposed in
/// docs/lanes/replies-contract.md); a stored value outside the list is added so the pop-up always names what is stored.
enum RepliesChoices {
    static let perQuestion: [UInt32] = [3, 5, 8, 10]
    static let keep: [UInt32] = [3, 6, 10, 20]
    static let sample: [UInt32] = [50, 100, 200, 500]
    static let examples: [UInt32] = [0, 1, 3, 5]
    static func with(_ list: [UInt32], _ stored: UInt32?) -> [UInt32] {
        guard let stored, !list.contains(stored) else { return list }
        return (list + [stored]).sorted()
    }
}

/// « Joint à chaque extrait »: date, sender, folder, subject, each one tick.
struct FieldsPopover: View {
    let fields: ExcerptFields
    @State private var store = RepliesStore.shared
    var body: some View {
        let P = RepliesAx.id("retrieval", "fields")
        VStack(alignment: .leading, spacing: WhooshSpacing.s6) {
            Text(RepliesWords.attached).whooshText(WhooshTypography.body, emphasized: true)
            ForEach(RepliesField.allCases, id: \.self) { f in
                ReplyTick(title: RepliesWords.field(f), on: RepliesWords.isOn(f, fields), ax: "\(P).\(f.rawValue)") { store.toggleField(f) }
            }
        }
        .padding(WhooshSpacing.s14).frame(minWidth: 220, alignment: .leading)
    }
}

/// « Filtre par l'IA »: the question each excerpt must answer yes to, or none (the filter is off).
struct AiFilterPopover: View {
    let question: String?
    @State private var store = RepliesStore.shared
    @State private var text = ""   // local-ui: the words being typed
    var body: some View {
        let P = RepliesAx.id("retrieval", "ai-filter")
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            Text(RepliesWords.aiFilter).whooshText(WhooshTypography.body, emphasized: true)
            Text(RepliesWords.aiFilterLine).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).fixedSize(horizontal: false, vertical: true)
            SettingsField(ax: "\(P).field", placeholder: RepliesWords.aiFilterHint, text: $text, width: 300) { save() }
            HStack {
                SettingsButton(title: RepliesWords.noFilter, ax: "\(P).off") { store.setRetrieval(.aiFilterQuestion(question: nil)); store.popover = nil }
                Spacer(minLength: 0)
                SettingsButton(title: Words.text("Save"), primary: true, ax: "\(P).save") { save() }
            }
        }
        .padding(WhooshSpacing.s14).frame(width: 330, alignment: .leading)
        .onAppear { text = question ?? "" }
    }
    private func save() {
        let q = text.trimmingCharacters(in: .whitespacesAndNewlines)
        store.setRetrieval(.aiFilterQuestion(question: q.isEmpty ? nil : q))
        if store.error == nil { store.popover = nil }
    }
}
