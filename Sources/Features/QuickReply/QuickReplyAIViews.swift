import SwiftUI

/// The AI pill at the right of the slab's header (boards ai-replies-r4 design 1 and r5 "On a thread": `.ai`, 24 pt, the
/// accent at 16 %, "✦ Brouillon ⌘J"). Quiet (the spark and its cap, no fill) while the field holds no AI draft, so the one
/// filled control of the slab stays Envoyer; tinted while the instruction field is up, while the model writes
/// ("Écriture…", the spark breathing: a symbol effect, no timer of ours) and while the words are the AI's. A click is ⌘J.
/// Beside it, while the words are the AI's: × with its Esc cap, which discards them.
struct QuickReplyAIPill: View {
    @Bindable var model: QuickReplyModel
    @Bindable var app: AppModel
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var ai: QuickReplyAIState { model.ai }
    private var tinted: Bool { ai.shown || ai.phase == .asking || ai.isWriting }
    private var ease: Animation? { WhooshMotion.animation(WhooshMotion.hover, reduceMotion: reduceMotion) }

    var body: some View {
        HStack(spacing: WhooshSpacing.s2) {
            Button { _ = model.aiKey(app) } label: { pill }
                .buttonStyle(.plain)
                .disabled(model.outboxId != nil)
                .help(ai.shown ? QuickReplyCopy.aiRewriteHelp : QuickReplyCopy.aiWriteHelp)
                .accessibilityLabel(ai.shown ? QuickReplyCopy.aiMarkLabel : QuickReplyCopy.aiWriteHelp)
                .whooshAx(QuickReplyIds.ai)
            if ai.shown && !ai.isWriting { discard.transition(.opacity) }
        }
        .animation(ease, value: tinted)
        .animation(ease, value: ai.shown && !ai.isWriting)
    }

    private var pill: some View {
        HStack(spacing: WhooshSpacing.s6) {
            Image(systemName: "sparkles")
                .font(.system(size: WhooshTypography.caption.size + 1, weight: .semibold))
                .symbolEffect(.variableColor.iterative.reversing, options: .repeating, isActive: ai.isWriting && !reduceMotion)
                .accessibilityHidden(true)
            if tinted {
                Text(QuickReplyAIRegistry.pillWords(ai)).whooshText(WhooshTypography.caption, emphasized: true)
                    .lineLimit(1).fixedSize()
                    .contentTransition(reduceMotion ? .identity : .opacity)
                    .transition(.opacity)
            }
            KeyCap(QuickReplyCopy.aiCap)
        }
        .foregroundStyle(tinted ? WhooshPalette.accent : WhooshPalette.secondaryLabel)
        .padding(.leading, tinted ? WhooshSpacing.s8 : WhooshSpacing.s4).padding(.trailing, WhooshSpacing.s4)   // the board's 9 / 5
        .frame(height: WhooshSpacing.s24)
        .background { Capsule().fill(WhooshPalette.accent.opacity(tinted ? 0.16 : 0)) }
        .contentShape(Capsule())
        .modifier(HoverAck(radius: WhooshSpacing.s12))
    }

    private var discard: some View {
        Button { _ = model.aiDiscard(app) } label: {
            HStack(spacing: WhooshSpacing.s4) {
                Image(systemName: "xmark").font(.system(size: WhooshTypography.caption2.size + 1, weight: .bold))
                    .foregroundStyle(WhooshPalette.secondaryLabel).accessibilityHidden(true)
                KeyCap(QuickReplyCopy.escapeCap)
            }
            .padding(.horizontal, WhooshSpacing.s4).frame(height: WhooshSpacing.s24)
            .contentShape(Capsule())
            .modifier(HoverAck(radius: WhooshSpacing.s12))
        }
        .buttonStyle(.plain)
        .help(QuickReplyCopy.aiDiscard).accessibilityLabel(QuickReplyCopy.aiDiscard)
        .whooshAx(QuickReplyIds.aiDiscard)
    }
}

/// ONE instruction field before the draft (board ai-replies-r4 design 1: "Consigne", a 24 pt field with a hairline).
/// Return writes, Esc closes it and gives the owner's words back. It takes the keys as it appears.
struct QuickReplyInstructionRow: View {
    let model: QuickReplyModel
    @Bindable var ai: QuickReplyAIState
    @Bindable var app: AppModel
    @FocusState private var focused: Bool
    init(model: QuickReplyModel, app: AppModel) { self.model = model; self.ai = model.ai; self.app = app }

    var body: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Text(QuickReplyCopy.aiInstruction).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                .lineLimit(1).fixedSize()
            HStack(spacing: WhooshSpacing.s6) {
                TextField(QuickReplyCopy.aiInstructionPlaceholder, text: $ai.instruction)
                    .textFieldStyle(.plain).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.label)
                    .focused($focused)
                    .onSubmit { _ = model.aiKey(app) }
                KeyCap(QuickReplyCopy.returnCap)
            }
            .padding(.leading, WhooshSpacing.s8).padding(.trailing, WhooshSpacing.s2)
            .frame(height: WhooshSpacing.s24)
            .background(WhooshPalette.field, in: RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
            .overlay {
                RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous)
                    .strokeBorder(focused ? WhooshPalette.focusHalo : WhooshPalette.separator, lineWidth: WhooshMetrics.separator)
            }
            .animation(WhooshMotion.hover, value: focused)
            .contentShape(Rectangle())
            .simultaneousGesture(TapGesture().onEnded { _ = model.aiFocus(.instruction) })   // the field keeps its own click
            .whooshAx(QuickReplyIds.aiInstruction)
        }
        .onAppear { focused = true }
        .onChange(of: focused) { _, on in if on { ai.focus = .instruction } }
        .onChange(of: ai.focus) { _, f in focused = f == .instruction }
    }
}

/// The field under an AI draft (board ai-replies-r5 "On a thread"; owner: "I like better the updates/changes here"): say
/// what to change, Return, and the draft is rewritten in place. It dims while the rewrite is on its way; typing never waits.
struct QuickReplyChangeRow: View {
    let model: QuickReplyModel
    @Bindable var ai: QuickReplyAIState
    @Bindable var app: AppModel
    @FocusState private var focused: Bool
    init(model: QuickReplyModel, app: AppModel) { self.model = model; self.ai = model.ai; self.app = app }

    var body: some View {
        HStack(spacing: WhooshSpacing.s6) {
            TextField(QuickReplyCopy.aiChangePlaceholder, text: $ai.change)
                .textFieldStyle(.plain).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label)
                .focused($focused)
                .onSubmit { _ = model.aiRequestChange(app) }
            if !ai.change.isEmpty { KeyCap(QuickReplyCopy.returnCap).transition(.opacity) }
        }
        .padding(.horizontal, WhooshSpacing.s10).padding(.vertical, WhooshSpacing.s6)
        .background(WhooshPalette.fill, in: RoundedRectangle(cornerRadius: WhooshRadius.row, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: WhooshRadius.row, style: .continuous)
                .strokeBorder(WhooshPalette.focusHalo.opacity(focused ? 1 : 0), lineWidth: WhooshMetrics.separator)
        }
        .opacity(ai.isWriting ? 0.6 : 1)
        .animation(WhooshMotion.hover, value: focused)
        .animation(WhooshMotion.hover, value: ai.isWriting)
        .animation(WhooshMotion.hover, value: ai.change.isEmpty)
        .contentShape(Rectangle())
        .simultaneousGesture(TapGesture().onEnded { _ = model.aiFocus(.change) })
        .onChange(of: focused) { _, on in if on { ai.focus = .change } }
        .onChange(of: ai.focus) { _, f in focused = f == .change }
        .whooshAx(QuickReplyIds.aiChange)
    }
}

/// The core's refusal of an AI command, in its words, under the field; ⌘J tries again.
struct QuickReplyAIFailure: View {
    let words: String
    var body: some View {
        HStack(spacing: WhooshSpacing.s6) {
            Image(systemName: "exclamationmark.circle.fill").foregroundStyle(WhooshPalette.warning).accessibilityHidden(true)
            Text(words).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.label).lineLimit(2)
            Spacer(minLength: WhooshSpacing.s8)
            KeyCap(QuickReplyCopy.aiCap)
        }
        .transition(.opacity)
    }
}

/// The landing light: when an AI draft lands or is rewritten, the field glows once in the accent and settles, within
/// MOTION_MAX_MS (a rise of HOVER_TINT_MS, a fall of LAND_STATE_MS). Keyed on `landed`: no timer, nothing runs at rest.
struct AILandingLight: ViewModifier {
    let trigger: Int
    func body(content: Content) -> some View {
        content.keyframeAnimator(initialValue: 0.0, trigger: trigger) { view, glow in
            view.background {
                RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous).fill(WhooshPalette.accent.opacity(0.12 * glow))
                    .padding(-WhooshSpacing.s4)
            }
        } keyframes: { _ in
            LinearKeyframe(1.0, duration: Double(WhooshConstants.HOVER_TINT_MS) / 1000)
            CubicKeyframe(0.0, duration: Double(WhooshConstants.LAND_STATE_MS) / 1000)
        }
    }
}
