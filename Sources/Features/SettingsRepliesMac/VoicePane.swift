import SwiftUI

/// Voix: the analysis of my sent mail, the text it writes (« Ma voix », editable), and the examples a draft is given.
struct VoicePane: View {
    @State private var store = RepliesStore.shared
    private let P = RepliesAx.id("voice")

    var body: some View {
        RepliesScaffold(page: .voice) {
            RepliesGroup(title: RepliesWords.analysis) {
                SettingsRow(ax: "\(P).analyse.row", title: RepliesWords.analyseSent, caption: RepliesWords.lastAnalysis(store.voice, store.analysis)) {
                    HStack(spacing: WhooshSpacing.s8) {
                        SettingsPopup(ax: "\(P).sample.popup", options: RepliesChoices.with(RepliesChoices.sample, store.voice?.sampleSize).map { ($0, RepliesWords.latest($0)) },
                                      selection: store.voiceSample ?? store.voice?.sampleSize ?? 100) { store.voiceSample = $0 }
                        SettingsButton(title: RepliesWords.analyse, ax: "\(P).analyse") { store.analyse(store.voiceSample ?? store.voice?.sampleSize ?? 100) }
                            .disabled(isRunning)
                    }
                }
            }
            RepliesGroup(title: RepliesWords.myVoice, note: RepliesWords.myVoiceNote, card: false) {
                RepliesTextEditor(key: .voiceText)
            }
            RepliesGroup(title: RepliesWords.examples) {
                SettingsRow(ax: "\(P).examples", title: RepliesWords.samePerson) {
                    SettingsPopup(ax: "\(P).examples.popup", options: RepliesChoices.with(RepliesChoices.examples, store.voice?.samePersonExamples).map { ($0, RepliesWords.upTo($0)) },
                                  selection: store.voice?.samePersonExamples ?? 3) { store.setExamples($0) }
                }
                SettingsRow(ax: "\(P).tones", title: RepliesWords.otherTones, onTap: { store.popover = .tones }) {
                    RepliesValue(text: store.tones.isEmpty ? RepliesWords.noOtherTone : store.tones.map(\.name).joined(separator: ", "))
                        .popover(isPresented: shown(.tones), arrowEdge: .trailing) { TonesPopover() }
                }
            }
        }
    }

    private var isRunning: Bool { if case .running = store.analysis { return true } else { return false } }

    private func shown(_ p: RepliesPopover) -> Binding<Bool> {
        Binding(get: { store.popover == p }, set: { if !$0, store.popover == p { store.popover = nil } })
    }
}

/// « Autres tons »: each tone, its name and its words; one more with « Ajouter », one less with its minus. A removed tone sends the uses that
/// chose it back to my voice (the core's rule).
struct TonesPopover: View {
    @State private var store = RepliesStore.shared
    @State private var name = ""   // local-ui: the new tone being typed
    @State private var words = ""
    var body: some View {
        let P = RepliesAx.id("voice", "tone")
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            Text(RepliesWords.otherTones).whooshText(WhooshTypography.body, emphasized: true)
            ForEach(store.tones, id: \.id) { t in
                HStack(alignment: .firstTextBaseline, spacing: WhooshSpacing.s8) {
                    VStack(alignment: .leading, spacing: 1) {
                        Text(t.name).whooshText(WhooshTypography.body)
                        Text(t.text).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).fixedSize(horizontal: false, vertical: true)
                    }
                    Spacer(minLength: 0)
                    Button { store.deleteTone(t.id) } label: { Image(systemName: "minus.circle").foregroundStyle(WhooshPalette.secondaryLabel) }
                        .buttonStyle(.plain).help(Words.text("Delete")).whooshAx("\(P).\(t.id).delete")
                }
                .transition(.opacity)
            }
            Divider()
            SettingsField(ax: "\(P).new.name", placeholder: RepliesWords.toneName, text: $name, width: 280)
            SettingsField(ax: "\(P).new.text", placeholder: RepliesWords.toneWords, text: $words, width: 280) { add() }
            HStack { Spacer(); SettingsButton(title: Words.text("Add"), primary: true, ax: "\(P).add") { add() } }
        }
        .padding(WhooshSpacing.s14).frame(width: 310, alignment: .leading)
        .animation(.easeOut(duration: WhooshMotion.rowInsert), value: store.tones.map(\.id))
    }
    private func add() {
        store.saveTone(ReplyTone(id: "", name: name, text: words))
        if store.error == nil { name = ""; words = "" }
    }
}
