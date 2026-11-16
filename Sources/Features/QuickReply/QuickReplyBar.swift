import SwiftUI
import UniformTypeIdentifiers

/// The quick reply at the foot of the conversation. Round 5 (board cards-s3, owner pick "Lifted until it lands"): in
/// the reader it is a FLOATING GLASS SLAB over the thread, 16 pt in from the pane's sides and 14 pt above its foot:
/// a header line (chevron, "Répondre à <x>" / "Brouillon à <x>", ⇧H; the pop-out ⇧⌘P at the right), the open To /
/// Cc / Cci / Objet rows, the field with its signature, then the foot: From with its emblem and ⇧⌘F, the emoji row,
/// and the one filled blue control, "Envoyer ⌘↵" (quiet while the field is empty). While a send counts down the header
/// is the countdown itself: ring, "Envoi dans N s", "Annuler Z". It rises on the system spring when it opens; typing
/// never waits for it. The reader lane mounts it as an overlay at the bottom of the scroll area (hook "quickreply-r5
/// hook: reader"); the thread keeps `slabHeight` of room under its last card so no card ever sits under the slab.
/// `inset: .compact` is the notch's black card: the round-4 shape (From line, field, round send), no glass, no header.
public struct QuickReplyBar: View {
    public enum Inset: Sendable { case reader, compact }
    let inset: Inset
    @Bindable var model: QuickReplyModel
    @Bindable var app: AppModel
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @FocusState private var focused: Bool
    @State private var risen = false
    private typealias L = WhooshGlass.Lift

    public init(model: QuickReplyModel = .shared, app: AppModel = .shared, inset: Inset = .reader) {
        self.model = model; self.app = app; self.inset = inset
    }
    private var isCompact: Bool { inset == .compact }

    public var body: some View {
        Group {
            if isCompact { compact } else {
                slab
                #if os(macOS)
                    .background(ReplyAnchor(role: .box))   // the box's place: a click outside it leaves the reply (ReplyKeyboard)
                #endif
                    // folded at the send (the sending card carries the countdown): not drawn, takes no click; it stays in the tree so the send
                    // animation, attached below, still starts from its Send button
                    .opacity(model.outboxId == nil ? 1 : 0)
                    .allowsHitTesting(model.outboxId == nil)
                    .animation(WhooshMotion.animation(WhooshMotion.hover, reduceMotion: reduceMotion), value: model.outboxId == nil)
            }
        }
            .sendWave(pane: isCompact ? "mac-notch.reply" : "mac-main.reply", origin: QuickReplyIds.on(isCompact, QuickReplyIds.send), on: model.outboxId)
            .offset(y: risen || reduceMotion ? 0 : WhooshMotion.undoRise)
            .opacity(risen ? 1 : 0)
            .onAppear {
                takeFocus()
                withAnimation(WhooshMotion.animation(WhooshMotion.tabThumb, reduceMotion: reduceMotion) ?? WhooshMotion.readerSwap) { risen = true }
            }
            .onDisappear { model.slabHeight = 0 }
            .onChange(of: model.focusSerial) { _, _ in takeFocus() }
            .onChange(of: model.outboxId) { _, id in if id != nil { focused = false } }   // a send in its window: the text does not hold the keys (QuickReplyModel.letGoOfKeys)
    }

    /// A served signature as the box's one line: its lines joined by a space, blank lines dropped.
    static func signatureLine(_ s: String) -> String {
        s.split(whereSeparator: \.isNewline).map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }.joined(separator: " ")
    }

    /// The reply's text takes the keyboard (round 20, owner 07:28: R left the keys on the list). Asked at once and again on the next turns:
    /// on the frame R lands the editor may not be in the window yet (the slab is rising) and a focus asked then is dropped, and a bar already
    /// drawn never sees `onAppear` again. Each later ask runs only while the text still does not hold the keys. A header row the owner
    /// clicked (Cc, Cci, Objet) or an AI field keeps its keys.
    private func takeFocus() {
        // Only for a reply that ASKED for the keyboard and can be typed in. The box is also in the tree, undrawn, through its send's window
        // (the send animation starts from it): coming back to that conversation during the countdown must not put the cursor in a box nobody
        // sees (lead, 13 Nov, walk s129: J then K during the countdown, the next letter went into the folded box and it came back holding "r").
        guard model.wantsKeys, model.showsBox, model.header.focus == .none, model.ai.focus == .reply else { return }
        focused = true
        for ms in Self.focusRetriesMs {
            DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(ms)) {
                if !focused, model.wantsKeys, model.showsBox, model.header.focus == .none, model.ai.focus == .reply { focused = true }
            }
        }
    }
    /// When the focus is asked again after R: the next turn, then after the slab's rise (WhooshMotion.undoRise runs about 200 ms).
    private static let focusRetriesMs = [0, 80, 240]

    // MARK: the reader's slab

    private var slab: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            QuickReplyHeaderLine(model: model, app: app)
            // above what follows in the box: the people list of its Cc and Cci lines opens over the text, the signature and the From line
            // (picture on main161: those drew over the list)
            if model.header.open { QuickReplyHeaderRows(model: model).transition(.opacity).zIndex(2) }
            if model.ai.phase == .asking { QuickReplyInstructionRow(model: model, app: app).transition(.opacity.combined(with: .move(edge: .top))) }
            field
            if model.ai.changeFieldShown { QuickReplyChangeRow(model: model, app: app).transition(.opacity.combined(with: .move(edge: .top))) }
            if let words = model.ai.failureWords { QuickReplyAIFailure(words: words) }
            if let words = model.failureWords { QuickReplyFailure(words: words, details: model.failureDetails) { _ = model.retry(app) } }
            foot
        }
        // The AI rows rise and fold on the system spring, after the state flipped on the key frame (nothing waits for it).
        .animation(WhooshMotion.animation(WhooshMotion.unfold, reduceMotion: reduceMotion), value: QuickReplyAIRegistry.phaseWord(model.ai.phase))
        .animation(WhooshMotion.animation(WhooshMotion.unfold, reduceMotion: reduceMotion), value: model.ai.changeFieldShown)
        .padding(.horizontal, WhooshSpacing.s14).padding(.vertical, WhooshSpacing.s12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .whooshLift(in: RoundedRectangle(cornerRadius: L.panelRadius, style: .continuous))
        // Under the glass, the pane's own ground (round 20, main115 judge 637ae165 p5: at 960 light "Let me know by Tuesday…" was drawn over
        // "Répondre à Linea Supply ⇧H"; the lift's ultra-thin material lets the thread's words through, sharp on the hidden copy). The lift
        // keeps its edge, highlight and shadow; no word of the thread reads through the reply.
        .background(WhooshPalette.background, in: RoundedRectangle(cornerRadius: L.panelRadius, style: .continuous))
        .padding(.horizontal, L.panelInset).padding(.bottom, L.panelBottom)
        // The thread keeps this much room under its last card: measured from the drawn slab, applied on the same pass.
        .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { model.slabHeight = $0 }
        .whooshAx(QuickReplyIds.slab)
    }

    private var field: some View {
        ZStack(alignment: .topLeading) {
            if model.draft.isEmpty {
                // while the model writes the placeholder says so, in the same place and type (the quiet "writing" state)
                Text(model.ai.isWriting ? QuickReplyCopy.aiWritingPlaceholder : model.context.placeholder).whooshText(WhooshTypography.body)
                    .foregroundStyle(model.ai.isWriting ? WhooshPalette.tertiaryLabel : WhooshPalette.secondaryLabel)
                    .whooshAx("mac-main.quickreply.placeholder")   // a leaf text the socket's `labels` reads (clipped-label check, shots-sheet)
                    .contentTransition(reduceMotion ? .identity : .opacity)
                    .padding(.top, WhooshSpacing.s8).padding(.leading, WhooshSpacing.s8).allowsHitTesting(false)
            }
            VStack(alignment: .leading, spacing: 0) {
                editor.padding(.horizontal, WhooshSpacing.s4).padding(.top, WhooshSpacing.s4)
                    .opacity(model.ai.isWriting && !model.draft.isEmpty ? 0.5 : 1)   // a rewrite on its way: the old words wait, dimmed
                    .animation(WhooshMotion.hover, value: model.ai.isWriting)
                if !model.context.signature.isEmpty {
                    // One line, its lines joined (round 20, main115 judge: the box read "Best,…", the board the signature's name line): the served
                    // signature's words all show while the line has room, "Best, Assem Chammah · Nordlicht".
                    Text(Self.signatureLine(model.context.signature)).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                        .lineLimit(1).truncationMode(.tail).padding(.horizontal, WhooshSpacing.s8).padding(.bottom, WhooshSpacing.s6)
                        .padding(.top, WhooshSpacing.s6)   // a breath between the owner's last line and the signature, so the signature never reads as the next typed line
                        .whooshAx("mac-main.quickreply.signature")
                }
            }
        }
        .frame(minHeight: WhooshMetrics.quickReplyField)
        .fixedSize(horizontal: false, vertical: true)
        .modifier(AILandingLight(trigger: model.ai.landed))
        .contentShape(Rectangle())
        .onTapGesture { model.header.focus = .none; model.ai.focus = .reply; focused = true }
        #if os(macOS)
        .onDrop(of: AttachmentIntake.types, isTargeted: nil) { model.dropFiles($0, app) }   // a dropped file goes on in the full composer
        #endif
        .whooshAx(QuickReplyIds.field)
    }

    private var editor: some View {
        TextEditor(text: $model.draft)
            .whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label)
            .scrollContentBackground(.hidden).focused($focused)
            #if os(macOS)
            .background(ReplyAnchor(role: .text))   // where the reply's text is, for the keyboard guarantee (ReplyKeyboard)
            #endif
            // Plain Return is the text's own: a new line, nothing else (owner 2026-11-13 07:27, "hitting enter actually sends", "not cmd+enter";
            // main115: this handler sent the reply into the undo window). ⌘↵ and ⌃↵ send through the Send button's shortcuts (QuickReplySendPill).
            .onChange(of: focused) { _, on in
                model.fieldFocused = on; if on { model.header.focus = .none; model.ai.focus = .reply; model.wantsKeys = true }
                #if os(macOS)
                QuickReplyPasteWatch.watch(on)   // ⌘V of a picture copied as data is the reply's while the field holds the keys
                #endif
            }
            .onChange(of: model.ai.focus) { _, f in if f == .reply, inset == .reader { focused = true } }   // back from an AI field
            .onChange(of: model.draft) { old, new in
                #if os(macOS)
                if model.takePastedFiles(old: old, new: new, app) { return }   // a pasted file is attached, never its name
                #endif
                model.aiTextChanged(app)   // typing over the AI's words makes them the owner's
            }
            .disabled(model.outboxId != nil)
    }

    private var foot: some View {
        HStack(spacing: WhooshSpacing.s8) {
            QuickReplyFromButton(model: model)
            Spacer(minLength: WhooshSpacing.s8).layoutPriority(-1)   // the From line is offered the width first, the gap takes what is left (round 18)
            QuickReplyEmojiButton(model: model, app: app)
            QuickReplySendPill(enabled: canSend) { _ = model.send(app) }
        }
    }

    // MARK: the notch's compact card (round 4 shape)

    private var compact: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s6) {
            QuickReplyFromButton(model: model, compact: true).frame(height: WhooshSpacing.s20)
            ZStack(alignment: .topLeading) {
                if model.draft.isEmpty {
                    Text(model.context.placeholder).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.secondaryLabel)
                        .padding(.top, WhooshSpacing.s8).padding(.leading, WhooshSpacing.s12).allowsHitTesting(false)
                }
                editor.padding(.horizontal, WhooshSpacing.s8).padding(.top, WhooshSpacing.s8).padding(.bottom, WhooshSpacing.s32)
                VStack {
                    Spacer(minLength: 0)
                    HStack(spacing: WhooshSpacing.s4) {
                        Text(model.context.signature).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                            .lineLimit(1).truncationMode(.tail)
                        Spacer(minLength: WhooshSpacing.s8)
                        QuickReplyEmojiButton(model: model, app: app, compact: true)
                        QuickReplySendButton(enabled: canSend, compact: true) { _ = model.send(app) }
                    }
                    .padding(.leading, WhooshSpacing.s12).padding(.trailing, WhooshSpacing.s8).padding(.bottom, WhooshSpacing.s8)
                }
            }
            .frame(minHeight: WhooshMetrics.quickReplyField)
            .fixedSize(horizontal: false, vertical: true)
            .background(WhooshPalette.field, in: RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous)
                .strokeBorder(focused ? WhooshPalette.accent : WhooshPalette.separator, lineWidth: WhooshMetrics.separator))
            .animation(WhooshMotion.hover, value: focused)
            if let words = model.failureWords { QuickReplyFailure(words: words, details: model.failureDetails, compact: true) { _ = model.retry(app) } }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .overlay(alignment: .topTrailing) {
            if case .counting(_, let countdown) = model.phase {
                UndoBar(message: QuickReplyCopy.sending, countdown: countdown,
                        onUndo: { _ = model.undo(app) }, onExpire: { model.expire(app) })
                    .fixedSize()
                    .offset(y: -(WhooshMetrics.segmentHeight + WhooshSpacing.s8))
                    .whooshAx(QuickReplyIds.on(true, QuickReplyIds.undoBar))
            }
        }
        .whooshAx(QuickReplyIds.on(true, QuickReplyIds.field))
    }

    private var canSend: Bool {
        model.outboxId == nil && !model.draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }
}

/// The reader column: the conversation's measure, the slab floating at its foot. The shell's surface
/// window draws it for `shot mac-quickreply`; the reader lane mounts QuickReplyBar the same way (an overlay).
struct QuickReplyColumn: View {
    var body: some View {
        Color.clear
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(WhooshPalette.background)
            .overlay(alignment: .bottom) { QuickReplyBar().frame(maxWidth: WhooshMetrics.readerMeasure + 2 * WhooshMetrics.readerSide) }
    }
}
