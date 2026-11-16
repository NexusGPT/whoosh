import SwiftUI

/// Templates: saved messages with their files. A pick fills the subject, the text and the files; the foot of the panel
/// keeps the message on screen as a new template.
struct TemplatesPanel: View {
    @Bindable var model: ComposerModel
    var body: some View {
        PickerPanel(width: WhooshSpacing.s32 * 9) {
            panelTitle(ComposerWords.templates)
            ForEach(Array(model.library.templates.enumerated()), id: \.element.id) { i, t in
                Button { _ = model.applyTemplate(i) } label: {
                    PickerRow(title: t.name, highlighted: i == 0) {
                        if !t.files.isEmpty { Image(systemName: "paperclip").foregroundStyle(i == 0 ? WhooshPalette.onAccent : WhooshPalette.secondaryLabel) }
                    }
                }.buttonStyle(.plain).whooshAx(ComposerIds.template(i))
            }
            Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator).padding(.vertical, WhooshSpacing.s4)
            Button { _ = model.saveTemplate() } label: {
                PickerRow(title: ComposerWords.saveTemplate) { Image(systemName: "plus").foregroundStyle(WhooshPalette.secondaryLabel) }
            }.buttonStyle(.plain).whooshAx(ComposerIds.templatesSave)
        }
        .popoverEntrance(anchor: .bottomLeading)
        .whooshAx(ComposerIds.templates + ".panel")
    }
}

/// Snippets (⌘;): short saved texts that go in at the caret.
struct SnippetsPanel: View {
    @Bindable var model: ComposerModel
    var body: some View {
        PickerPanel(width: WhooshSpacing.s32 * 9) {
            HStack { panelTitle(ComposerWords.snippets); Spacer(minLength: 0); KeyCap("⌘;") }.padding(.trailing, WhooshSpacing.s8)
            ForEach(Array(model.library.snippets.enumerated()), id: \.element.id) { i, sn in
                Button { _ = model.insertSnippet(i) } label: {
                    PickerRow(title: sn.name, highlighted: i == 0) { EmptyView() }
                }.buttonStyle(.plain).whooshAx(ComposerIds.snippet(i))
            }
            Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator).padding(.vertical, WhooshSpacing.s4)
            Button { _ = model.saveSnippet() } label: {
                PickerRow(title: ComposerWords.saveSnippet) { Image(systemName: "plus").foregroundStyle(WhooshPalette.secondaryLabel) }
            }.buttonStyle(.plain).whooshAx(ComposerIds.snippetsSave)
        }
        .popoverEntrance(anchor: .bottomLeading)
        .whooshAx(ComposerIds.snippets + ".panel")
    }
}

/// Instant Reply: whole replies to the message, each a different answer. A pick fills the text; the owner edits or sends.
struct InstantReplyPanel: View {
    @Bindable var model: ComposerModel
    var body: some View {
        PickerPanel(width: WhooshSpacing.s32 * 12) {
            panelTitle(ComposerWords.instantReply)
            if model.instantOptions.isEmpty {
                Text(model.aiBusy ? ComposerWords.thinking : model.line).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                    .padding(.horizontal, WhooshSpacing.s10).padding(.vertical, WhooshSpacing.s6)
            }
            ForEach(Array(model.instantOptions.enumerated()), id: \.offset) { i, text in
                Button { _ = model.pickInstantReply(i) } label: {
                    Text(text).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.label).multilineTextAlignment(.leading)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.horizontal, WhooshSpacing.s10).padding(.vertical, WhooshSpacing.s6)
                        .contentShape(Rectangle()).hoverAck(radius: WhooshRadius.control)
                }.buttonStyle(.plain).whooshAx(ComposerIds.instantChoice(i))
            }
        }
        .popoverEntrance(anchor: .bottomLeading)
        .whooshAx(ComposerIds.instantReply + ".panel")
    }
}

/// The chevron next to Send: send and mark done, send now with no undo.
struct ComposerMoreSendButton: View {
    @Bindable var model: ComposerModel
    @Bindable var app: AppModel
    var body: some View {
        Button { model.closePanelsExcept(more: true) } label: {
            Image(systemName: "chevron.up").font(.system(size: WhooshTypography.caption.size, weight: .semibold))
                .foregroundStyle(WhooshPalette.secondaryLabel)
                .frame(width: WhooshMetrics.actionButton, height: WhooshMetrics.actionButton)
                .hoverAck(radius: WhooshRadius.control).contentShape(Rectangle())
        }
        .buttonStyle(.plain).disabled(model.isCounting).opacity(model.isCounting ? 0.4 : 1)
        .help(ComposerWords.moreSend).accessibilityLabel(ComposerWords.moreSend).whooshAx(ComposerIds.sendMore)
        .overlay(alignment: .bottomTrailing) {
            if model.moreSendOpen {
                PickerPanel(width: WhooshSpacing.s32 * 9) {
                    Button { _ = model.sendAndMarkDone(app) } label: { PickerRow(title: ComposerWords.sendAndDone, highlighted: true) { KeyCap("⌘⇧↵", onSelection: true) } }
                        .buttonStyle(.plain).whooshAx(ComposerIds.sendDone)
                    Button { _ = model.sendInstantly(app) } label: { PickerRow(title: ComposerWords.sendNow) { KeyCap("⌘⇧Z") } }
                        .buttonStyle(.plain).whooshAx(ComposerIds.sendInstant)
                }
                .popoverEntrance(anchor: .bottomTrailing)
                .fixedSize().offset(y: -WhooshMetrics.actionButton - WhooshSpacing.s4)
            }
        }
    }
}

@MainActor private func panelTitle(_ text: String) -> some View {
    Text(text).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
        .padding(.horizontal, WhooshSpacing.s10).padding(.top, WhooshSpacing.s4)
}
