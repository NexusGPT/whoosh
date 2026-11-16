import SwiftUI
#if os(macOS)
import AppKit
#endif

/// THE editor of every long text of the Replies pages (the prompt, the retrieval prompt, my voice, the AI card's description, each fixed note):
/// one component, one behaviour, always live. It wears the rest look the owner approved (`.doc`): the same card, the same pills, the same grey
/// notes, before and while typing. Under it, one plain line when needed (not saved, an unknown « @mot », the revert question) and, only while
/// the text differs from the saved one, « Rétablir » and « Enregistrer ⌘S ».
struct RepliesTextEditor: View {
    let key: RepliesEditorKey
    var placeholder: String? = nil
    var minHeight: CGFloat = 44
    var maxHeight: CGFloat = 420
    @State private var store = RepliesStore.shared

    var body: some View {
        let e = store.editor(key)
        let P = RepliesAx.id("editor", key.raw)
        VStack(alignment: .leading, spacing: WhooshSpacing.s6) {
            ZStack(alignment: .topLeading) {
                #if os(macOS)
                RepliesTextArea(state: e, maxHeight: maxHeight, layoutTick: e.layoutTick)
                    .frame(height: e.textHeight > 0 ? min(max(e.textHeight, 20), maxHeight) : nil)   // round 24: the laid-out height, at the real width
                    .frame(minHeight: minHeight)
                #endif
                if e.text.isEmpty, let placeholder {
                    Text(placeholder).font(.system(size: 13.5)).foregroundStyle(WhooshPalette.tertiaryLabel)
                        .padding(.leading, 5).allowsHitTesting(false)
                }
            }
            .padding(.horizontal, 11).padding(.vertical, 12)
            .background(RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous).fill(WhooshPalette.fill))
            .overlay(RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: WhooshMetrics.separator))
            .clipShape(RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous))   // nothing is drawn outside the card
            .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { e.cardFrame = $0 }
            .overlay(alignment: .topLeading) {
                // the @ list opens under the « @ », over whatever is below the card (it is the one thing allowed outside it)
                if e.menuQuery != nil {
                    RepliesAtList(state: e).fixedSize()
                        .offset(x: 11 + max(0, e.caret.minX - 4), y: 12 + e.caret.maxY + 4)
                        .transition(.opacity)
                }
            }
            .whooshAx(P)
            RepliesEditorFoot(state: e, P: P)
        }
        .animation(.easeOut(duration: WhooshMotion.hoverTint), value: e.dirty)
        .animation(.easeOut(duration: WhooshMotion.hoverTint), value: e.menuQuery != nil)
    }
}

/// The line under an editor and its two buttons; nothing at all while the text is the saved one and says nothing wrong.
struct RepliesEditorFoot: View {
    let state: RepliesEditorState
    let P: String
    @State private var store = RepliesStore.shared

    var body: some View {
        let line = RepliesWords.editorLine(state)
        if line != nil || state.dirty {
            HStack(alignment: .firstTextBaseline, spacing: WhooshSpacing.s8) {
                if let line {
                    Text(line).whooshText(WhooshTypography.caption)
                        .foregroundStyle(state.unknown.isEmpty || state.confirmRevert ? WhooshPalette.secondaryLabel : WhooshPalette.warning)
                        .fixedSize(horizontal: false, vertical: true).whooshAx("\(P).line")
                }
                Spacer(minLength: 0)
                if state.dirty {
                    SettingsButton(title: RepliesWords.revert, key: "esc", ax: "\(P).revert") {
                        if state.confirmRevert { state.revert() } else { state.confirmRevert = true }
                    }
                    SettingsButton(title: Words.text("Save"), key: "⌘S", primary: true, ax: "\(P).save") { store.save(state.key) }
                }
            }
            .padding(.horizontal, WhooshSpacing.s4)
            .transition(.opacity)
        }
    }
}

/// The list « @ » opens at the caret: the variables whose name starts with the letters typed; ↑ ↓ choose, Return places, Esc closes.
struct RepliesAtList: View {
    let state: RepliesEditorState
    @State private var store = RepliesStore.shared

    var body: some View {
        let items = state.items(store.variables)
        VStack(alignment: .leading, spacing: 0) {
            if items.isEmpty {
                Text(RepliesWords.noVariable).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).padding(8)
            }
            ForEach(Array(items.enumerated()), id: \.element.id) { i, v in RepliesAtRow(v: v, on: i == state.menuIndex) }
        }
        .padding(4)
        .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(WhooshPalette.panel).shadow(color: .black.opacity(0.18), radius: 8, y: 3))
        .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: 0.5))
        .whooshAx(RepliesAx.id("editor", state.key.raw, "menu"))
    }
}

/// One variable of the @ list: the chosen one on the accent, its name and its meaning.
struct RepliesAtRow: View {
    let v: ReplyPromptVariable
    let on: Bool
    var body: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Text("@" + RepliesVariables.name(v)).font(.system(size: 13, weight: .medium))
                .foregroundStyle(on ? AnyShapeStyle(WhooshPalette.onAccent) : AnyShapeStyle(WhooshPalette.accent))
            Text(RepliesVariables.meaning(v)).whooshText(WhooshTypography.caption).lineLimit(1)
                .foregroundStyle(on ? AnyShapeStyle(WhooshPalette.onAccent) : AnyShapeStyle(WhooshPalette.secondaryLabel))
        }
        .padding(.horizontal, 8).frame(height: 24).frame(minWidth: 220, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 5, style: .continuous).fill(WhooshPalette.accent).opacity(on ? 1 : 0))
    }
}

#if os(macOS)
/// The text view inside SwiftUI, in its own NSScrollView (the composer's pattern, RichTextEditor.swift): SwiftUI places and sizes the
/// scroll view; the text view is its document, so it may grow and shrink freely without ever leaving the card. The card is as tall as the
/// text up to `maxHeight`, then the text scrolls inside it. (Round 5 put a self-resizing NSTextView straight in SwiftUI's frame: it
/// resized itself outside the frame SwiftUI gave it, so its text was drawn below its own card, the lead's main74 shots.)
struct RepliesTextArea: NSViewRepresentable {
    /// The card's ceiling when the page names none: above it the text scrolls inside the card.
    static let ceiling: CGFloat = 420
    let state: RepliesEditorState
    var maxHeight: CGFloat = RepliesTextArea.ceiling
    /// hid-ai round 20 (main111 d58d8ad5 p8, state-replies-panes-prompt-dark-700x480.png: the prompt card too short and scrolled, its heading and note line
    /// cut, where the light picture at the same size is whole): the card's height is measured once by SwiftUI (sizeThatFits) at the width it proposes; a text
    /// replaced from outside (reload) or a card later given another width kept the old height, and the text scrolled inside a card too short for it. A new
    /// value here makes SwiftUI measure again (RepliesEditorCoordinator.remeasure).
    var layoutTick = 0

    func makeCoordinator() -> RepliesEditorCoordinator { RepliesEditorCoordinator(state: state) }

    func makeNSView(context: Context) -> NSScrollView {
        let scroll = NSScrollView()
        scroll.drawsBackground = false; scroll.borderType = .noBorder
        scroll.hasVerticalScroller = true; scroll.autohidesScrollers = true; scroll.scrollerStyle = .overlay
        let tv = RepliesTextView(frame: NSRect(x: 0, y: 0, width: 300, height: 20))
        let c = context.coordinator
        tv.owner = c
        tv.isRichText = true; tv.importsGraphics = false; tv.allowsUndo = true; tv.drawsBackground = false
        tv.isContinuousSpellCheckingEnabled = true; tv.isAutomaticQuoteSubstitutionEnabled = false
        // hid-ai round 14: on a TEST COPY only, no inline prediction, no automatic correction, no completion. The prediction's bubble (NSCorrectionPanel)
        // takes the next events for itself (`_interceptEvents`); the rig's key verb types between short run-loop turns and sends no further event, so after
        // « Bonjour » the panel waited for ever and every later read timed out (13 settingsreplies-check cases; main105 sample: KeyEventVerb.run >
        // runUntilDate > NSTextCheckingController handleCompletionFromCandidates > NSSpellChecker _showInlinePredictionForReplacingRange > NSCorrectionPanel
        // _interceptEvents > nextEventMatchingMask). The owner's copy keeps the system's settings: a person's next key reaches the panel and goes on.
        if RigNet.rig(env: nil) {
            tv.inlinePredictionType = .no; tv.isAutomaticSpellingCorrectionEnabled = false; tv.isAutomaticTextCompletionEnabled = false
        }
        tv.textContainerInset = .zero; tv.textContainer?.lineFragmentPadding = 5
        tv.minSize = .zero; tv.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        tv.isVerticallyResizable = true; tv.isHorizontallyResizable = false; tv.autoresizingMask = [.width]
        tv.textContainer?.containerSize = NSSize(width: 300, height: CGFloat.greatestFiniteMagnitude)
        tv.textContainer?.widthTracksTextView = true
        tv.layoutManager?.allowsNonContiguousLayout = true   // a key lays out what changed, never the whole text
        tv.delegate = c
        scroll.documentView = tv
        c.textView = tv
        c.watchWidth(of: scroll)
        c.reload()
        RepliesEditorViews.live[state.key] = tv
        return scroll
    }

    func updateNSView(_ scroll: NSScrollView, context: Context) {
        if context.coordinator.applied != state.revision { context.coordinator.reload() }
    }

    /// As tall as the text at this width, never more than `maxHeight` (the rest scrolls inside the card).
    func sizeThatFits(_ proposal: ProposedViewSize, nsView scroll: NSScrollView, context: Context) -> CGSize? {
        guard let w = proposal.width, w > 0, let tv = scroll.documentView as? NSTextView, let lm = tv.layoutManager, let tc = tv.textContainer else { return nil }
        if abs(tc.containerSize.width - w) > 0.5 { tc.containerSize = NSSize(width: w, height: CGFloat.greatestFiniteMagnitude) }
        lm.ensureLayout(for: tc)   // the whole height is needed here (layout is otherwise non-contiguous)
        let h = ceil(lm.usedRect(for: tc).height)
        let coordinator = context.coordinator
        coordinator.measuredWidth = w
        coordinator.fitsWhole = h <= maxHeight
        return CGSize(width: w, height: min(max(h, 20), maxHeight))
    }

    static func dismantleNSView(_ scroll: NSScrollView, coordinator: RepliesEditorCoordinator) {
        if let tv = scroll.documentView as? RepliesTextView, RepliesEditorViews.live[coordinator.key] === tv { RepliesEditorViews.live[coordinator.key] = nil }
    }
}

/// The editors on screen, by key: the socket's doors and rows reach the live text view through here (a page not shown has none).
@MainActor
enum RepliesEditorViews {
    static var live: [RepliesEditorKey: RepliesTextView] = [:]
    /// The editor that holds the keys: its text view is its window's first responder.
    static var focused: RepliesTextView? { live.values.first { $0.window?.firstResponder === $0 } }

    /// Where the text is DRAWN (its used rectangle, clipped to what its scroll view shows), in the window's content coordinates (top-left).
    static func textFrame(_ k: RepliesEditorKey) -> CGRect? {
        guard let tv = live[k], let lm = tv.layoutManager, let tc = tv.textContainer, let content = tv.window?.contentView else { return nil }
        let used = lm.usedRect(for: tc).offsetBy(dx: tv.textContainerOrigin.x, dy: tv.textContainerOrigin.y).intersection(tv.visibleRect)
        guard !used.isNull else { return nil }
        let r = content.convert(used, from: tv)
        return content.isFlipped ? r : CGRect(x: r.minX, y: content.bounds.height - r.maxY, width: r.width, height: r.height)
    }
}
#endif
