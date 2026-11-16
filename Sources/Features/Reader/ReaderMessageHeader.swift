import SwiftUI
#if os(macOS)
import AppKit
#endif

/// SPEC A70 (owner 22:00): every message's one-line header (sender · time) carries a chevron beside the sender; the
/// chevron or ⇧H on the marked message expands it to From, To, Cc (when the core serves it) and the full date with its
/// zone, and collapses it back. Per message, remembered while the thread is shown. The state flips on the input's frame;
/// the rows then unfold on the system spring (WhooshMotion.unfold), nothing waits for it.
struct MessageHeader: View {
    let index: Int
    let fact: ReaderFact?
    /// the author line when the core holds no facts for this message
    let fallbackAuthor: String
    var openSender: (() -> Void)?
    var timeHidden = false
    @State private var state = ReaderState.shared  // local-ui: the per-message header state, observed
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var open: Bool { fact.map { state.headerIsOpen($0.id) } ?? false }

    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s4) {
            BlockMeta(author: fact?.from ?? fallbackAuthor, time: fact?.time, address: open ? nil : fact?.address,
                      open: openSender, timeHidden: timeHidden || open,
                      chevron: fact.map { f in HeaderChevron(index: index, open: open) { toggle(f.id) } })
                .contentShape(Rectangle())
                .onTapGesture { if let fact { toggle(fact.id) } }  // the line itself: the sender and the chevron keep their own clicks
                .whooshAx("mac-main.reader.header.line.\(index)")
            if open, let fact { ExpandedHeader(fact: fact).transition(ReaderHeaderMotion.rows) }
        }
    }

    private func toggle(_ id: String) {
        withAnimation(ReaderHeaderMotion.animation(reduceMotion: reduceMotion)) { state.toggleHeader(id) }
    }
}

/// The header's motion (round 10, A70): the details fade in 4 pt above their place and settle (opacity and an offset, nothing
/// else), over ROW_COLLAPSE_MS (120, under the 180 ms bound). The state flips on the input's frame; the message body below
/// follows the same curve, so it never jumps. Reduced Motion: no animation, the state still flips.
enum ReaderHeaderMotion {
    static let ms = WhooshConstants.ROW_COLLAPSE_MS
    static let properties = "opacity+transform"
    static var rows: AnyTransition { .opacity.combined(with: .offset(y: -WhooshSpacing.s4)) }
    static func animation(reduceMotion: Bool) -> Animation? { WhooshMotion.animation(WhooshMotion.archiveCollapse, reduceMotion: reduceMotion) }
    /// For a key, which has no environment: the system setting itself.
    #if os(macOS)
    @MainActor static var keyAnimation: Animation? { animation(reduceMotion: NSWorkspace.shared.accessibilityDisplayShouldReduceMotion) }
    #else
    @MainActor static var keyAnimation: Animation? { animation(reduceMotion: UIAccessibility.isReduceMotionEnabled) }
    #endif
}

/// The small chevron beside the sender: right when collapsed, down when open; tooltip with the bound key.
struct HeaderChevron: View {
    let index: Int
    let open: Bool
    let toggle: () -> Void
    var body: some View {
        Button(action: toggle) {
            Image(systemName: "chevron.right")
                .font(.system(size: WhooshTypography.caption.size - 2, weight: .semibold))
                .rotationEffect(.degrees(open ? 90 : 0))
                .foregroundStyle(WhooshPalette.secondaryLabel)
                .frame(width: WhooshMetrics.keyCap, height: WhooshTypography.caption.lineHeight)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(ReaderWords.headerTooltip(open: open))
        .accessibilityLabel(ReaderWords.headerTooltip(open: open))
        .whooshAx("mac-main.reader.header.\(index)")
    }
}

/// The expanded header: From (name and address), To, Cc when served, Date (full, with the zone).
struct ExpandedHeader: View {
    let fact: ReaderFact
    var body: some View {
        Grid(alignment: .leadingFirstTextBaseline, horizontalSpacing: WhooshSpacing.s8, verticalSpacing: WhooshSpacing.s2) {
            ForEach(ReaderWords.headerRows(fact), id: \.label) { row in
                GridRow {
                    Text(row.label).foregroundStyle(WhooshPalette.secondaryLabel).gridColumnAlignment(.trailing)
                    Text(row.value).foregroundStyle(WhooshPalette.label).textSelection(.enabled)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        .whooshText(WhooshTypography.caption)
        .frame(maxWidth: WhooshMetrics.readerMeasure, alignment: .leading)
        .accessibilityIdentifier("mac-main.reader.header.details")
    }
}

extension ReaderWords {
    static let headerCommand = "toggle-message-header"
    /// Why the Cc row is absent today; the probe says it so a check can tell "not served" from "dropped".
    static let ccNotServed = "no cc field yet (contract delta 12)"

    /// "Expand message header  ⇧H" / "Collapse message header  ⇧H": the active preset's key, never a literal.
    @MainActor
    static func headerTooltip(open: Bool) -> String {
        let label = open ? Words.text("Collapse message header") : Words.text("Expand message header")
        guard let key = ShortcutEngine.shared.chord(for: headerCommand) else { return label }
        return label + "  " + key
    }

    /// Tooltips that carry a key (A39): the rule that unfolds the earlier messages and the chips Tab walks. The key is the
    /// active preset's, never a literal.
    @MainActor static var earlierTooltip: String { withKey(Words.text("Expand all messages"), "expand-all-messages") }
    @MainActor static var linkTooltip: String { withKey(Words.text("Next link or date"), "cycle-links") }
    /// The command's row chord, else the chord the reader owns by context (Tab walks the chips only inside the reader; the
    /// same key is the split and the list indent elsewhere, so the preset keeps it as a context key, not a row).
    @MainActor private static func withKey(_ label: String, _ command: String) -> String {
        guard let key = ReaderKeyLabel.readerChord(for: command) else { return label }
        return label + "  " + key
    }

    /// "Name <address>": the core's From, and each recipient as it is served.
    static func headerRows(_ f: ReaderFact) -> [(label: String, value: String)] {
        var rows: [(label: String, value: String)] = [(Words.text("From"), f.address.isEmpty || f.from == f.address ? f.from : "\(f.from) <\(f.address)>")]
        if !f.to.isEmpty { rows.append((Words.text("To"), f.to.joined(separator: ", "))) }
        if let cc = f.cc, !cc.isEmpty { rows.append((Words.text("Cc"), cc.joined(separator: ", "))) }
        rows.append((Words.text("Date"), fullDate(f.receivedAtMs)))
        if !f.attachments.isEmpty { rows.append((Words.text("Attachments"), attachments(f.attachments.count))) }
        return rows
    }
}
