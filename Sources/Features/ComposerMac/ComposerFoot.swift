import SwiftUI
#if os(macOS)
import AppKit
#endif

/// The foot: the tools on the left (attach, dictate, AI draft, rewrite), the line and the actions on the right.
/// The AI field, the rewrite menu, the send-later picker and the undo bar rise from it.
struct ComposerFoot: View {
    @Bindable var model: ComposerModel
    @Bindable var app: AppModel
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(spacing: WhooshSpacing.s8) {
            if model.aiFieldOpen { AIDraftField(model: model, app: app).transition(.opacity) }
            bar
        }
        .padding(.horizontal, WhooshMetrics.readerSide).padding(.top, WhooshSpacing.s10).padding(.bottom, WhooshSpacing.s14)
        .overlay(alignment: .top) { Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator) }
        .overlay(alignment: .top) { statusCapsule.offset(y: -WhooshSpacing.s32) }
        .animation(reduceMotion ? WhooshMotion.readerSwap : WhooshMotion.unfold, value: model.aiFieldOpen)
        .task(id: model.outboxId) {
            while model.outboxId != nil, !Task.isCancelled {
                try? await Task.sleep(for: .milliseconds(200))
                model.reconcile(app)
            }
        }
    }

    @ViewBuilder private var statusCapsule: some View {
        switch model.phase {
        case _ where model.discardAsk:
            DiscardCapsule(model: model, app: app)
        case .counting(_, let countdown):
            UndoBar(message: ComposerWords.sending, countdown: countdown, onUndo: { _ = model.undo(app) }, onExpire: { model.expire(app) })
                .fixedSize()
                .whooshAx(ComposerIds.undoBar)
        case .sent: SentCapsule()
        default: EmptyView()
        }
    }

    private var bar: some View {
        HStack(spacing: WhooshSpacing.s4) {
            ComposerIconButton(symbol: "paperclip", label: ComposerWords.attachTip, key: "⌘⇧A", active: model.attachOpen, disabled: model.isCounting, id: ComposerIds.attach) { _ = model.toggleAttachPanel() }
                .overlay(alignment: .bottomLeading) { if model.attachOpen { AttachPanel(model: model).fixedSize().offset(y: -WhooshMetrics.actionButton - WhooshSpacing.s4) } }
            dictateButton
            ComposerIconButton(symbol: "sparkles", label: ComposerWords.aiDraft, active: model.aiFieldOpen, disabled: model.isCounting, id: ComposerIds.aiDraft) { _ = model.openAIField(app) }
            ComposerIconButton(symbol: "wand.and.stars", label: ComposerWords.rewriteTip, active: model.rewriteOpen, disabled: model.isCounting || model.session.document.isEmpty, id: ComposerIds.rewrite) { _ = model.openRewrite() }
                .overlay(alignment: .bottomLeading) { if model.rewriteOpen { RewriteMenu(model: model).fixedSize().offset(y: -WhooshMetrics.actionButton - WhooshSpacing.s4) } }
            ComposerIconButton(symbol: "doc.text", label: ComposerWords.templates, active: model.templatesOpen, disabled: model.isCounting, id: ComposerIds.templates) { _ = model.toggleTemplates() }
                .overlay(alignment: .bottomLeading) { if model.templatesOpen { TemplatesPanel(model: model).fixedSize().offset(y: -WhooshMetrics.actionButton - WhooshSpacing.s4) } }
            ComposerIconButton(symbol: "text.insert", label: ComposerWords.snippets, key: "⌘;", active: model.snippetsOpen, disabled: model.isCounting, id: ComposerIds.snippets) { _ = model.toggleSnippets() }
                .overlay(alignment: .bottomLeading) { if model.snippetsOpen { SnippetsPanel(model: model).fixedSize().offset(y: -WhooshMetrics.actionButton - WhooshSpacing.s4) } }
            ComposerIconButton(symbol: "bolt", label: ComposerWords.instantReply, active: model.instantOpen, disabled: model.isCounting, id: ComposerIds.instantReply) { _ = model.toggleInstantReply() }
                .overlay(alignment: .bottomLeading) { if model.instantOpen { InstantReplyPanel(model: model).fixedSize().offset(y: -WhooshMetrics.actionButton - WhooshSpacing.s4) } }
            ComposerIconButton(symbol: model.reminderAt == nil ? "bell" : "bell.fill", label: ComposerWords.remindTitle, key: "⌘⇧H", active: model.remindOpen || model.reminderAt != nil, disabled: model.isCounting && model.reminderAt != nil, id: ComposerIds.remind) { _ = model.toggleRemind() }
                .overlay(alignment: .bottomLeading) { if model.remindOpen { RemindPanel(model: model, app: app).fixedSize().offset(y: -WhooshMetrics.actionButton - WhooshSpacing.s4) } }
            ComposerIconButton(symbol: "trash", label: ComposerWords.discardTip, key: "⌘⇧,", active: model.discardAsk, disabled: model.isCounting, id: ComposerIds.discard) { _ = model.requestDiscard(app) }
            Spacer(minLength: WhooshSpacing.s12)
            lineView
                Button { _ = ComposerMacFeature.clickTargets()[ComposerIds.sendLater]?(app) } label: {
                    HStack(spacing: WhooshSpacing.s6) {
                        Image(systemName: "clock")
                        Text(ComposerWords.sendLater).whooshText(WhooshTypography.subhead)
                        KeyCap("⌘⇧L")
                    }
                    .foregroundStyle(model.sendLaterOpen ? WhooshPalette.accent : WhooshPalette.secondaryLabel)
                    .padding(.horizontal, WhooshSpacing.s8).frame(height: WhooshMetrics.actionButton)
                    .hoverAck(radius: WhooshRadius.control).contentShape(Rectangle())
                }
                .buttonStyle(.plain).disabled(model.isCounting).help(ComposerControlKeys.tooltip(ComposerIds.sendLater)).whooshAx(ComposerIds.sendLater)
                .overlay(alignment: .bottomTrailing) { if model.sendLaterOpen { SendLaterPanel(model: model, app: app).fixedSize().offset(y: -WhooshMetrics.actionButton - WhooshSpacing.s4) } }
            HStack(spacing: WhooshSpacing.s2) {
                ComposerSendButton(enabled: model.canSend && !model.isCounting) { _ = model.send(app) }
                ComposerMoreSendButton(model: model, app: app)
            }
        }
    }

    private var dictateButton: some View {
        let d = Dictation.shared
        let live = d.state != .idle
        _ = model.dictationRevision
        return HStack(spacing: WhooshSpacing.s6) {
            ComposerIconButton(symbol: live ? "mic.fill" : "mic", label: ComposerWords.dictateTip, active: live, id: ComposerIds.dictate) { ComposerMacFeature.toggleDictation() }
            if live {
                Text(d.state == .listening ? (d.liveWords.isEmpty ? ComposerWords.listening : d.liveWords) : ComposerWords.cleaning)
                    .whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.accent).lineLimit(1).frame(maxWidth: WhooshSpacing.s32 * 7, alignment: .leading)
                    .transition(.opacity)
            }
        }
        .animation(WhooshMotion.readerSwap, value: live)
    }

    @ViewBuilder private var lineView: some View {
        let words = model.line.isEmpty ? Dictation.shared.line : model.line
        if !words.isEmpty {
            Text(words).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(2)
                .frame(maxWidth: WhooshSpacing.s32 * 10, alignment: .trailing).transition(.opacity)
                .whooshAx(ComposerIds.line)
                .help(model.lineHelp.map { Words.text("Details: %@", $0) } ?? "")
        }
    }
}
