import SwiftUI
import UniformTypeIdentifiers

/// The Mac composer (docs/surfaces/mac-composer.md, native glass): From, To, Subject on a hairline grid, the rich editor,
/// the quoted text folded, a foot with attach, dictate, AI draft, rewrite, send later and Send. The undo bar rises over
/// the foot while a send can still be taken back. Nothing here animates typing.
public struct ComposerView: View {
    @Bindable var model: ComposerModel
    @Bindable var app: AppModel
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var risen = false
    @State private var dropping = false
    @FocusState private var field: ComposerField?
    @Namespace private var ring

    public init(model: ComposerModel = .shared, app: AppModel = .shared) { self.model = model; self.app = app }

    public var body: some View {
        VStack(spacing: 0) {
            titleBar
            ComposerHeader(model: model, field: $field, ring: ring).padding(.horizontal, WhooshMetrics.readerSide)
                .zIndex(3)   // the sender and recipient lists open over the editor below
            Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator)
            editor.sendWave(pane: "mac-composer.body", origin: ComposerIds.send, on: model.outboxId)
            if !model.attachments.isEmpty {
                VStack(alignment: .leading, spacing: WhooshSpacing.s4) {
                    #if os(macOS)
                    AttachmentStrip(model: model)
                    #endif
                    AttachmentNotes(model: model)
                }.padding(.horizontal, WhooshMetrics.readerSide).padding(.bottom, WhooshSpacing.s10)
            }
            ComposerFoot(model: model, app: app)
        }
        .frame(minWidth: WhooshMetrics.blockWidth, idealWidth: WhooshMetrics.blockWidth + WhooshSpacing.s32 * 8, maxWidth: .infinity,
               minHeight: WhooshSpacing.s32 * 12, idealHeight: ComposerOpenMotion.idealHeight, maxHeight: .infinity)
        .background(WhooshPalette.background)
        .background { ComposerKeyLayer(model: model, app: app) }
        .overlay { if dropping { dropVeil } }
        // attachments hook: dropped files and images go through the AttachmentsMac intake (paste goes through ⌘V).
        #if os(macOS)
        .onDrop(of: AttachmentIntake.types, isTargeted: $dropping) { AttachmentIntake.drop($0) }
        #endif
        .offset(y: risen || reduceMotion ? 0 : ComposerOpenMotion.risePoints)     // a window opens with opacity and a short rise, nothing else moves
        .opacity(risen ? 1 : 0)
        .onAppear {
            Dictation.shared.onChange = { model.dictationRevision += 1 }
            applyFocus(model.focusRequest)                     // the cursor a new or reopened composer asked for
            model.session.listKeyHandler = { model.inlineKey($0) }
            model.session.requestLink = { _ = model.toggleLink() }
            model.session.requestColor = { _ = model.toggleColor() }
            model.session.instantSend = { model.sendInstantly(app) }
            #if os(macOS)   // the Mac's pasteboard intake (AttachmentsMac is not in the iPhone build)
            model.session.pasteFiles = { AttachmentIntake.takePasted(AttachmentPasteboard.board) }
            model.session.pasteboardName = AttachmentPasteboard.rigName
            #endif
            withAnimation(reduceMotion ? WhooshMotion.readerSwap : .easeOut(duration: Double(ComposerOpenMotion.milliseconds) / 1000)) { risen = true }
        }
        .animation(model.ringAnimation(reduceMotion: reduceMotion), value: model.ringField)
        .onChange(of: model.focusRequest) { _, r in applyFocus(r) }
        .onChange(of: field) { _, f in model.focusFollows(f) }
        .onChange(of: model.session.triggerState?.trigger) { _, t in model.triggerChanged(t) }
        .onChange(of: app.typed) { _, v in if ComposerMacFeature.hasFocus(app), !model.aiFieldOpen, !v.isEmpty { model.insertAtCaret(v); app.typed = "" } }   // a closed composer keeps its window and this view: it takes nothing from the shared buffer
        .whooshAx(ComposerIds.surface)
    }

    private var titleBar: some View {
        HStack {
            Text(ComposerWords.title(model.kind)).whooshText(WhooshTypography.title3).foregroundStyle(WhooshPalette.label).lineLimit(1)
                .whooshAx("mac-composer.title")                        // a leaf text the socket's `labels` reads (clipped-label check, shots-sheet)
            Spacer()
            if model.kind.canPopIn {
                Button { _ = model.popIn(app) } label: { KeyHint(ComposerWindowKeys.popIn, ComposerWords.popInTitle) }
                    .buttonStyle(.plain).help(ComposerControlKeys.tooltip(ComposerIds.popIn)).whooshAx(ComposerIds.popIn)
            }
            if model.kind != .new {
                Button { _ = model.switchDraft(app) } label: { KeyHint(ComposerWindowKeys.thread, ComposerWords.threadTitle) }
                    .buttonStyle(.plain).help(ComposerControlKeys.tooltip(ComposerIds.thread)).whooshAx(ComposerIds.thread)
            }
            KeyHint("Esc", ComposerWords.close)
        }
        .padding(.horizontal, WhooshMetrics.readerSide).padding(.top, WhooshSpacing.s16).padding(.bottom, WhooshSpacing.s10)
    }

    private var editor: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            ZStack(alignment: .topLeading) {
                RichTextEditor(session: model.session)
                    .allowsHitTesting(!model.isCounting)
                    .whooshAx(ComposerIds.body)
                if model.session.document.isEmpty && !model.aiFieldOpen {
                    Text(ComposerWords.bodyPlaceholder).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.tertiaryLabel)
                        .padding(.top, WhooshSpacing.s10).padding(.leading, WhooshSpacing.s4).allowsHitTesting(false)   // the first typed line's place (the editor's 8 points and its line's own 2)
                }
            }
            #if os(macOS)
            // gap-composer-rich: a picture dropped ON THE TEXT goes into the text; any other file is attached as on the rest of the composer
            .onDrop(of: AttachmentIntake.types, isTargeted: $dropping) { model.dropOnText($0) }
            #endif
            .background(model.aiMarkActive ? AnyShapeStyle(WhooshPalette.mine.opacity(0.5)) : AnyShapeStyle(.clear), in: RoundedRectangle(cornerRadius: WhooshRadius.row, style: .continuous))
            .animation(WhooshMotion.readerSwap, value: model.aiMarkActive)
            .background { ComposerRing(model: model, field: .body, ns: ring) }
            .frame(maxHeight: .infinity)
            .overlay(alignment: .topLeading) { inlineList }
            .overlay(alignment: .topTrailing) { formatPanels }
            if model.aiMarkActive { AIMarkPill(model: model) }
            HStack(alignment: .firstTextBaseline) {
                if !model.signature.isEmpty { model.signatureText.whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).padding(.leading, WhooshSpacing.s4).whooshAx("mac-composer.signature") }
                Spacer(minLength: WhooshSpacing.s12)
                FooterToggle(model: model)
            }
            if !model.quoted.isEmpty { QuotedFold(model: model) }
        }
        .padding(.horizontal, WhooshMetrics.readerSide).padding(.vertical, WhooshSpacing.s10)
    }

    /// The `;` and `:` list under the caret; it opens upward when the caret is low in the editor.
    @ViewBuilder private var inlineList: some View {
        if let st = model.session.triggerState, !model.inlineItems.isEmpty {
            let rows = CGFloat(model.inlineItems.count)
            let height = rows * (WhooshMetrics.sidebarRow + WhooshSpacing.s4) + WhooshSpacing.s16
            let below = st.caret.maxY + height < WhooshSpacing.s32 * 10
            ComposerInlineList(model: model)
                .offset(x: min(st.caret.minX, WhooshMetrics.blockWidth - ComposerInlineList.width),
                        y: below ? st.caret.maxY + WhooshSpacing.s4 : st.caret.minY - height - WhooshSpacing.s4)
        }
    }

    /// The link and colour popovers of the format keys, at the editor's top right.
    @ViewBuilder private var formatPanels: some View {
        if model.linkOpen { LinkPanel(model: model).offset(y: WhooshSpacing.s8) }
        else if model.colorOpen { ColorPanel(model: model).offset(y: WhooshSpacing.s8) }
    }

    private func applyFocus(_ r: ComposerFocusRequest?) {
        guard let r else { return }
        switch r.field {
        case .from: field = nil; model.resignText()
        case .body:
            field = nil
            Task { @MainActor in
                // a key pressed before this turn ran (⇧⌘M then ⇧⌘O at once) asked for another field: the newer request wins
                guard model.focusRequest?.serial == r.serial else { return }
                model.session.hooks.focusBody()
                model.ringField = .body        // the first field SwiftUI focuses by itself must not take the ring from the message
            }
        default: field = r.field
        }
    }

    private var dropVeil: some View {
        RoundedRectangle(cornerRadius: WhooshRadius.sheet, style: .continuous).strokeBorder(WhooshPalette.accent, style: StrokeStyle(lineWidth: WhooshSpacing.s2, dash: [WhooshSpacing.s6]))
            .background(WhooshPalette.mine.opacity(0.4)).padding(WhooshSpacing.s8).allowsHitTesting(false).transition(.opacity)
    }
}

/// "Add “Sent with Whoosh”": off until the owner switches it on; when on, the line closes the text that goes out.
struct FooterToggle: View {
    @Bindable var model: ComposerModel
    var body: some View {
        Button { _ = model.toggleFooter() } label: {
            HStack(spacing: WhooshSpacing.s6) {
                Image(systemName: model.library.footerOn ? "checkmark.square.fill" : "square")
                    .foregroundStyle(model.library.footerOn ? WhooshPalette.accent : WhooshPalette.secondaryLabel)
                Text(ComposerWords.footerOption).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                    .whooshAx("mac-composer.footer.label")
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .animation(WhooshMotion.readerSwap, value: model.library.footerOn)
        .whooshAx(ComposerIds.footer)
    }
}

/// The real keyboard's way in: the same acts as the socket's keys, as hidden shortcuts of the window. A handler that has
/// nothing to do changes nothing.
struct ComposerKeyLayer: View {
    @Bindable var model: ComposerModel
    @Bindable var app: AppModel
    var body: some View {
        ZStack {
            key(.return, [.command, .shift]) { _ = model.sendAndMarkDone(app) }
            // no hidden ⇧⌘Z button: it took the plain ⌘Z too and sent the reply (main95). ⇧⌘Z sends from the text (WhooshTextView,
            // session.instantSend) and, outside a text field, through the shortcuts' "instant-send" (ComposerCommands).
            key(";", [.command]) { _ = model.toggleSnippets() }
            key("l", [.command, .shift]) { model.closePanels(); model.sendLaterOpen = true }
            key("b", [.command, .shift]) { _ = model.focus(.bcc) }
            key("c", [.command, .shift]) { _ = model.focus(.cc) }
            key(.escape, []) { _ = model.close(app) }
        }
        .frame(width: 0, height: 0).opacity(0).accessibilityHidden(true)
    }
    private func key(_ k: KeyEquivalent, _ m: EventModifiers, _ act: @escaping () -> Void) -> some View {
        Button("", action: act).keyboardShortcut(k, modifiers: m)
    }
}
