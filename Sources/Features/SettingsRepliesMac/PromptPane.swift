import SwiftUI

/// Le prompt: the text the model reads to write, with its variables, its fixed notes and its versions (owner round 7: "the prompt itself needs to
/// be in there somewhere too"). A save is a new version; an older version comes back as a new one.
struct PromptPane: View {
    @State private var store = RepliesStore.shared
    private let P = RepliesAx.id("prompt")

    var body: some View {
        RepliesScaffold(page: .prompt) {
            SettingsButton(title: RepliesWords.seeWhatModelGets, ax: "\(P).preview") { store.openPreview() }
                .popover(isPresented: shown(.preview), arrowEdge: .bottom) { PreviewPopover(text: store.preview) }
        } content: {
            RepliesTextEditor(key: .prompt, minHeight: 160).padding(.top, WhooshSpacing.s4)
            GroupedCard {
                SettingsRow(ax: "\(P).variables", title: RepliesWords.variables, onTap: { store.popover = .variables }) {
                    RepliesValue(text: RepliesWords.variablesLine(store.variables.count))
                        .popover(isPresented: shown(.variables), arrowEdge: .trailing) { VariablesPopover() }
                }
                SettingsRow(ax: "\(P).notes", title: RepliesWords.fixedNotes, onTap: { store.popover = .notes }) {
                    RepliesValue(text: RepliesWords.notesLine(store.notes.count))
                        .popover(isPresented: shown(.notes), arrowEdge: .trailing) { NotesPopover() }
                }
                SettingsRow(ax: "\(P).versions", title: RepliesWords.versions, onTap: { store.popover = .versions }) {
                    RepliesValue(text: store.prompt.map { RepliesWords.versionLine($0) } ?? "")
                        .popover(isPresented: shown(.versions), arrowEdge: .trailing) { VersionsPopover() }
                }
            }
            .padding(.top, 10)
        }
    }

    private func shown(_ p: RepliesPopover) -> Binding<Bool> {
        Binding(get: { store.popover == p }, set: { if !$0, store.popover == p { store.popover = nil } })
    }
}

/// « Variables »: the seven, each with its meaning. While the prompt is being edited a click types it.
struct VariablesPopover: View {
    @State private var store = RepliesStore.shared
    var body: some View {
        RepliesList(title: RepliesWords.variables) {
            ForEach(store.variables, id: \.id) { v in
                Button { store.placeVariable(v) } label: {
                    HStack(alignment: .firstTextBaseline, spacing: WhooshSpacing.s8) {
                        Text("@" + RepliesVariables.name(v)).font(.system(size: 13, weight: .medium)).foregroundStyle(WhooshPalette.accent).frame(minWidth: 120, alignment: .leading)
                        Text(RepliesVariables.meaning(v)).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).fixedSize(horizontal: false, vertical: true)
                    }
                }
                .buttonStyle(.plain).whooshAx(RepliesAx.id("prompt", "variable", v.id))
            }
        }
    }
}

/// « Notes fixes »: the notes joined to every draft; one more, one less.
struct NotesPopover: View {
    @State private var store = RepliesStore.shared
    var body: some View {
        let P = RepliesAx.id("prompt", "note")
        RepliesList(title: RepliesWords.fixedNotes) {
            ForEach(store.notes, id: \.id) { n in
                HStack(alignment: .firstTextBaseline, spacing: WhooshSpacing.s8) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(n.title).whooshText(WhooshTypography.body)
                        RepliesTextEditor(key: .note(n.id))
                    }
                    Spacer(minLength: 0)
                    Button { store.deleteNote(n.id) } label: { Image(systemName: "minus.circle").foregroundStyle(WhooshPalette.secondaryLabel) }
                        .buttonStyle(.plain).help(Words.text("Delete")).whooshAx("\(P).\(n.id).delete")
                }
            }
            Divider()
            SettingsField(ax: "\(P).new.title", placeholder: RepliesWords.noteTitle, text: $store.newNoteTitle, width: 300)
            RepliesTextEditor(key: .newNote, placeholder: RepliesWords.newNoteHint)
        }
    }
}

/// « Versions »: newest first; the current one is ticked, an older one comes back with « Rétablir ».
struct VersionsPopover: View {
    @State private var store = RepliesStore.shared
    var body: some View {
        RepliesList(title: RepliesWords.versions) {
            ForEach(store.versions, id: \.version) { v in
                HStack(spacing: WhooshSpacing.s8) {
                    SettingsCheck(on: v.current)
                    Text(RepliesWords.versionLine(v)).whooshText(WhooshTypography.body)
                    Spacer(minLength: WhooshSpacing.s12)
                    if !v.current {
                        SettingsButton(title: RepliesWords.restore, ax: RepliesAx.id("prompt", "version", "\(v.version)", "restore")) { store.restore(v.version) }
                    }
                }
            }
        }
    }
}

/// « Voir ce que le modèle reçoit »: the prompt with the newest message's values in place and the notes dropped.
struct PreviewPopover: View {
    let text: String?
    @State private var store = RepliesStore.shared
    var body: some View {
        ScrollView {
            // which text this is: the one being edited (not saved), or the saved version
            Text(store.previewOfDraft ? RepliesWords.previewOfDraft : RepliesWords.previewOfSaved(store.prompt?.version ?? 0))
                .whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                .frame(maxWidth: .infinity, alignment: .leading).padding([.horizontal, .top], WhooshSpacing.s14)
                .whooshAx(RepliesAx.id("prompt", "preview", "source"))
            Text(text ?? RepliesWords.notReady).font(.system(size: 12.5, design: .monospaced)).foregroundStyle(WhooshPalette.label)
                .textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading).padding(WhooshSpacing.s14)
        }
        .frame(width: 460, height: 360)
        .whooshAx(RepliesAx.id("prompt", "preview", "text"))
    }
}

/// The frame every list popover shares: its title, then its rows, at a reading width.
struct RepliesList<Content: View>: View {
    let title: String
    @ViewBuilder var content: Content
    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            Text(title).whooshText(WhooshTypography.body, emphasized: true)
            content
        }
        .padding(WhooshSpacing.s14).frame(width: 340, alignment: .leading)
        .animation(.easeOut(duration: WhooshMotion.rowInsert), value: RepliesStore.shared.reads)
    }
}
