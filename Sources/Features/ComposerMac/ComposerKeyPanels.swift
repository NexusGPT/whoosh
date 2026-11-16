import SwiftUI

/// ⌘⇧A and the paperclip: Choose a file, and the files attached lately. Return takes the highlighted row, Esc closes.
struct AttachPanel: View {
    @Bindable var model: ComposerModel

    var body: some View {
        PickerPanel(width: WhooshSpacing.s32 * 9) {
            HStack {
                Text(ComposerWords.attach).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
                Spacer(minLength: 0)
                KeyCap("⌘⇧A")
            }
            .padding(.horizontal, WhooshSpacing.s10).padding(.top, WhooshSpacing.s4)
            Button { _ = model.pickAttachRow(0) } label: {
                PickerRow(title: ComposerWords.chooseFile, highlighted: model.listIndex == 0) {
                    if model.listIndex == 0 { KeyCap("↵", onSelection: true) } else { Image(systemName: "paperclip").foregroundStyle(WhooshPalette.secondaryLabel) }
                }
            }.buttonStyle(.plain).whooshAx(ComposerIds.attachChoose)
            let recent = model.library.recentFiles
            if !recent.isEmpty {
                Text(ComposerWords.recentFiles).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.tertiaryLabel)
                    .padding(.horizontal, WhooshSpacing.s10).padding(.top, WhooshSpacing.s4)
                ForEach(Array(recent.enumerated()), id: \.element) { i, path in
                    Button { _ = model.pickAttachRow(i + 1) } label: {
                        PickerRow(title: URL(fileURLWithPath: path).lastPathComponent, highlighted: model.listIndex == i + 1, dimmed: true) { EmptyView() }
                    }.buttonStyle(.plain).whooshAx(ComposerIds.attachRecent(i))
                }
            }
        }
        .popoverEntrance(anchor: .bottomLeading)
        .whooshAx(ComposerIds.attachPanel)
    }
}

/// ⌘⇧,: the question before a draft with text in it is thrown away. Return discards, Esc keeps.
struct DiscardCapsule: View {
    @Bindable var model: ComposerModel
    @Bindable var app: AppModel

    var body: some View {
        HStack(spacing: WhooshSpacing.s12) {
            Text(ComposerWords.discardQuestion).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.label)
            Button { model.closePanels() } label: {
                HStack(spacing: WhooshSpacing.s6) { Text(ComposerWords.keep); KeyCap("Esc") }
                    .whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
            }.buttonStyle(.plain).whooshAx(ComposerIds.discardKeep)
            Button { _ = model.confirmDiscard(app) } label: {
                HStack(spacing: WhooshSpacing.s6) { Text(ComposerWords.discard); KeyCap("↵") }
                    .whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.accent)
            }.buttonStyle(.plain).whooshAx(ComposerIds.discardConfirm)
        }
        .padding(.horizontal, WhooshSpacing.s16).frame(height: WhooshMetrics.segmentHeight)
        .whooshGlass(in: Capsule())
        .transition(.opacity)
        .whooshAx(ComposerIds.discard + ".ask")
    }
}
