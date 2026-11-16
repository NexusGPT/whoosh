import SwiftUI

/// "What should the reply say?": the AI draft field, opened in place above the foot (J3.3).
struct AIDraftField: View {
    @Bindable var model: ComposerModel
    @Bindable var app: AppModel
    @FocusState private var focused: Bool
    var body: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Image(systemName: "sparkles").foregroundStyle(WhooshPalette.accent)
            TextField(ComposerWords.aiFieldPlaceholder, text: $app.typed)
                .textFieldStyle(.plain).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label)
                .focused($focused).onSubmit { _ = model.runDraft(app) }
                .whooshAx(ComposerIds.aiDraft + ".field")
            if model.aiBusy { ProgressView().controlSize(.small) }
            else { KeyCap("↵") }
        }
        .padding(.horizontal, WhooshSpacing.s12).frame(height: WhooshMetrics.glassPillHeight + WhooshSpacing.s4)
        .background(WhooshPalette.field, in: RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous))
        .overlay { RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous).strokeBorder(WhooshPalette.focusHalo, lineWidth: WhooshSpacing.s2 / 2 + 0.5) }
        .popoverEntrance(anchor: .bottomLeading)
        .onAppear { focused = true; model.aiInput = "" }
        .onChange(of: app.typed) { _, v in if ComposerMacFeature.hasFocus(app) { model.aiInput = v } }
    }
}

/// The rewrite choices in words: shorter, warmer, more formal. They act on one paragraph; ⌘Z brings it back (J3.4).
struct RewriteMenu: View {
    @Bindable var model: ComposerModel
    var body: some View {
        PickerPanel(width: WhooshSpacing.s32 * 6) {
            ForEach(RewriteStyle.allCases, id: \.self) { s in
                Button { _ = model.rewrite(s) } label: {
                    PickerRow(title: s.title.prefix(1).uppercased() + s.title.dropFirst(), highlighted: s == .shorter) { EmptyView() }
                }
                .buttonStyle(.plain).whooshAx(ComposerIds.rewriteChoice(s))
            }
        }
        .popoverEntrance(anchor: .bottomLeading)
        .whooshAx(ComposerIds.rewriteChoices)
    }
}

/// The AI mark: shown while the body is exactly the AI's words. Editing clears it; Accept clears it.
struct AIMarkPill: View {
    @Bindable var model: ComposerModel
    var body: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Image(systemName: "sparkles").foregroundStyle(WhooshPalette.accent)
            Text(ComposerWords.aiMark).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
            Button(ComposerWords.accept) { _ = model.acceptAI() }
                .buttonStyle(.plain).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.accent)
                .whooshAx(ComposerIds.aiAccept)
        }
        .padding(.horizontal, WhooshSpacing.s10).frame(height: WhooshSpacing.s24)
        .background(WhooshPalette.mine, in: Capsule())
        .transition(.opacity)
        .whooshAx(ComposerIds.aiMarks)
    }
}

/// The quoted text of a reply, folded by default (J3.1).
struct QuotedFold: View {
    @Bindable var model: ComposerModel
    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            Button { withAnimation(WhooshMotion.unfold) { model.quotedFolded.toggle() } } label: {
                HStack(spacing: WhooshSpacing.s6) {
                    Image(systemName: "chevron.right").rotationEffect(.degrees(model.quotedFolded ? 0 : 90))
                        .font(.system(size: WhooshTypography.caption.size - WhooshSpacing.s2, weight: .semibold))
                    Text(model.quotedFolded ? ComposerWords.quotedFold : ComposerWords.quotedUnfold).whooshText(WhooshTypography.subhead)
                }.foregroundStyle(WhooshPalette.secondaryLabel).contentShape(Rectangle())
            }.buttonStyle(.plain).whooshAx("mac-composer.quoted.toggle")
            if !model.quotedFolded {
                ForEach(model.quoted) { q in
                    VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
                        Text("\(q.from)  \(q.time)").whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
                        Text(q.text).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                    }
                    .padding(.leading, WhooshSpacing.s10)
                    .overlay(alignment: .leading) { Rectangle().fill(WhooshPalette.separator).frame(width: WhooshSpacing.s2) }
                }
            }
        }
    }
}

#if os(macOS)
/// Files dragged in or handed over by the notch: a tile each, name and size, a remove button.
struct AttachmentStrip: View {
    @Bindable var model: ComposerModel
    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: WhooshSpacing.s8) {
                ForEach(Array(model.attachments.enumerated()), id: \.element.id) { i, a in
                    // attachments hook: a pending file is the AttachmentsMac file proxy (Quick Look, open, drag, menu, remove).
                    AttachmentProxy(pendingName: a.name, index: i, remove: { model.removeAttachment(a) }) {
                    HStack(spacing: WhooshSpacing.s8) {
                        Image(systemName: "doc").foregroundStyle(WhooshPalette.secondaryLabel)
                        VStack(alignment: .leading, spacing: 0) {
                            Text(a.name).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.label).lineLimit(1)
                            Text(a.sizeWords).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                        }
                        Button { model.removeAttachment(a) } label: { Image(systemName: "xmark").foregroundStyle(WhooshPalette.secondaryLabel) }
                            .buttonStyle(.plain).accessibilityLabel(ComposerWords.remove(a.name))
                    }
                    .padding(.horizontal, WhooshSpacing.s10).frame(height: WhooshMetrics.fileTile + WhooshSpacing.s4)
                    .background(WhooshPalette.fill, in: RoundedRectangle(cornerRadius: WhooshRadius.row, style: .continuous))
                    }
                    .transition(.opacity)
                }
            }
        }.whooshAx(ComposerIds.attachments)
    }
}

#endif

/// Under the tiles: what each file that is not ready yet says (being added, will be fetched when sent, not on this Mac, too large).
struct AttachmentNotes: View {
    @Bindable var model: ComposerModel
    var body: some View {
        let notes = model.attachments.compactMap { a in ComposerWords.fileState(a.state).map { (a, $0) } }
        if !notes.isEmpty {
            VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
                ForEach(notes, id: \.0.id) { a, words in
                    Text(a.name + ": " + words).whooshText(WhooshTypography.caption)
                        .foregroundStyle(a.state == .gone || a.state == .tooLarge ? WhooshPalette.warning : WhooshPalette.secondaryLabel).lineLimit(1)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .whooshAx(ComposerIds.attachmentNotes)
        }
    }
}
