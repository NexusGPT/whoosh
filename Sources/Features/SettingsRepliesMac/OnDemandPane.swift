import SwiftUI

/// Sur demande: the same choices for ⌘J, in the same three groups as Brouillon automatique (the board's screen d). In a thread: the key, an
/// instruction before, a change after; with what; how.
struct OnDemandPane: View {
    @State private var store = RepliesStore.shared
    private let P = RepliesAx.id("on-demand")

    var body: some View {
        let o = store.onDemand
        RepliesScaffold(page: .onDemand) {
            RepliesGroup(title: RepliesWords.inThread) {
                SettingsRow(ax: "\(P).key", title: RepliesWords.writeDraft) { SettingsKey(text: "⌘J") }
                SettingsRow(ax: "\(P).instruction-before", title: RepliesWords.instructionBefore, caption: RepliesWords.instructionBeforeLine,
                            onTap: { store.setOption(.instructionBefore, !o.instructionBefore) }) {
                    SettingsSwitch(on: o.instructionBefore, refusals: store.refusals)
                }
                SettingsRow(ax: "\(P).change-after", title: RepliesWords.changeAfter, caption: RepliesWords.changeAfterLine,
                            onTap: { store.setOption(.changeAfter, !o.changeAfter) }) {
                    SettingsSwitch(on: o.changeAfter, refusals: store.refusals)
                }
            }
            UseSourcesGroup(kind: .onDemand)
            UseHowGroup(kind: .onDemand)
        }
    }
}
