import SwiftUI

/// An earlier message: its author and time from `message(id)`, then its first words; a message the core does not
/// hold yet says so and loads in place. When it holds the selected search result's passage (A152) it is shown whole and the passage is marked with the
/// drawing's tint (docs/design/boards/search-r3-parts/designs.js `.pm`: the accent colour at 26 percent behind the passage, the text colour unchanged); an
/// attachment passage shows that file's tile with the accent ring the app uses for a focused tile.
struct EarlierMessageBlock: View {
    let index: Int
    let fact: ReaderFact?
    /// O or ⇧O: the message shows whole instead of its first two lines.
    var expanded = false
    /// N and P stand on this message.
    var marked = false
    var links: ReaderLinkRow?
    /// The selected result's passage when this message holds it.
    var passage: ReaderPassage?
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        ReaderBlock(marked: marked, ax: "mac-main.reader.message.\(index)") {
            MessageHeader(index: index, fact: fact, fallbackAuthor: Words.text("Message %@", index + 1))
            text
                .whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.secondaryLabel)
                .lineLimit(expanded ? nil : 2).fixedSize(horizontal: false, vertical: true)
            #if os(macOS)
            if let f = fact, let words = passage?.file(in: f.attachments) { PassageTile(messageId: f.id, index: f.attachments.firstIndex(of: words) ?? 0, words: words) }
            #endif
            if let links { links }
        }
    }

    /// The first words, or the whole text from the core when the message is expanded and the core holds its body.
    private var words: String {
        guard let fact else { return ReaderWords.notDownloaded }
        return Self.shownText(fact, expanded: expanded)
    }

    /// What the block draws for a message: its whole text when expanded and stored, else its first words, else "Not downloaded yet".
    @MainActor static func shownText(_ fact: ReaderFact, expanded: Bool) -> String {
        if expanded, let whole = ReaderFact.wholeText(fact.id) { return whole }
        return fact.preview.isEmpty ? ReaderWords.notDownloaded : fact.preview
    }

    /// The text, with the passage tinted when this message holds it; the ranges are found once per answer (`PassageMemo`), not per draw.
    private var text: Text {
        let shown = words
        guard let passage, let fact else { return Text(shown) }
        let ranges = passage.ranges(inEarlier: fact.id, shown: shown)
        guard !ranges.isEmpty else { return Text(shown) }
        var a = AttributedString(shown)
        for r in ranges { if let ar = Range(r, in: a) { a[ar].backgroundColor = WhooshPalette.accent.color(scheme).opacity(PassageMark.tint) } }
        return Text(a)
    }
}

#if os(macOS)
/// The attachment of an earlier message that holds the passage: the app's own chip, with the ring the chip draws when it is the focused one (the same stroke).
struct PassageTile: View {
    let messageId: String
    let index: Int
    let words: String
    var body: some View {
        let split = AttachmentName.split(words)
        let ref = AttachmentRef(messageId: messageId, index: index, name: split.file)
        AttachmentChip(ref: ref, sizeWords: split.size.map { "\((split.file as NSString).pathExtension.uppercased()) · \($0)" })
            .frame(maxWidth: WhooshMetrics.blockWidth / 2, alignment: .leading)
            .padding(.top, WhooshSpacing.s8)
            .accessibilityIdentifier("mac-main.reader.passage.tile")
    }
}
#endif

/// `.erl` the "N earlier messages" rule: hairlines either side; a click unfolds them on the system spring.
struct EarlierRule: View {
    let count: Int
    let open: Bool
    let toggle: () -> Void
    var body: some View {
        Button(action: toggle) {
            HStack(spacing: WhooshSpacing.s10) {
                Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator)
                Text(ReaderWords.earlier(count)).whooshText(WhooshTypography.caption).fixedSize()
                Image(systemName: "chevron.down").rotationEffect(.degrees(open ? 180 : 0))
                    .font(.system(size: WhooshTypography.caption.size - 2, weight: .semibold))
                Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator)
            }
            .foregroundStyle(WhooshPalette.secondaryLabel)
            .frame(height: WhooshTypography.caption.lineHeight + WhooshSpacing.s8)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(ReaderWords.earlierTooltip)
        .accessibilityIdentifier("mac-main.reader.earlier")
    }
}
