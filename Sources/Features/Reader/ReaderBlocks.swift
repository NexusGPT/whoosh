import SwiftUI

/// `.bk` the one element of the conversation reader: a flat block with a hairline for a message; with `mine` the reply
/// field's lifted glass (ThreadFootLayout.fill(.reply)). Aligned by author: a message stops ThreadFootLayout.authorOffset
/// short of the right edge; the reply field takes the same edges as the draft and outgoing cards (SPEC A147).
struct ReaderBlock<Content: View>: View {
    var mine = false
    /// N and P stand on this message: an accent outline instead of the hairline.
    var marked = false
    /// Reported to the layout probe under this name ("message" for the open message).
    var frameName = "other"
    /// The probe id of the DRAWN card: applied before `threadFootEdges` and the full-width frame, so its frame is the card inside the author inset,
    /// not the padded column (the A147 edge cases compare cards). Same as the draft card's (ThreadDraftCard.swift, ef02c5475).
    var ax: String?
    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s4) { content }
            .padding(.top, WhooshSpacing.s8).padding(.bottom, WhooshSpacing.s10)
            .padding(.horizontal, WhooshSpacing.s14)
            .frame(maxWidth: .infinity, alignment: .leading)  // the block fills the pane; the text measure is capped inside it
            .background {
                let shape = RoundedRectangle(cornerRadius: mine ? ThreadFootLayout.replyFieldRadius : WhooshRadius.sheet, style: .continuous)
                if ThreadFootLayout.fill(mine ? .reply : .message) == .lifted { shape.fill(.clear).whooshLift(in: shape) } else {
                    shape.fill(WhooshPalette.background)
                        .overlay(shape.strokeBorder(marked ? WhooshPalette.accent : WhooshPalette.separator, lineWidth: marked ? WhooshSpacing.s2 : WhooshMetrics.separator))
                }
            }
            .readerFrame(frameName)
            .modifier(BlockAx(id: ax))
            .threadFootEdges(mine ? .reply : .message)
            .frame(maxWidth: .infinity, alignment: mine ? .trailing : .leading)
    }
}

/// `.whooshAx` when a block names its card; nothing otherwise.
private struct BlockAx: ViewModifier {
    let id: String?
    @ViewBuilder func body(content: Content) -> some View {
        if let id { content.whooshAx(id) } else { content }
    }
}

/// `.bm` the meta line: author 600 (a button when `open` is given: it opens the sender's card), the address under it,
/// time 400 at the far end.
struct BlockMeta: View {
    let author: String
    let time: String?
    var address: String?
    var open: (() -> Void)?
    /// The hover strip takes the right end of this line: the time steps aside while it shows.
    var timeHidden = false
    /// SPEC A70: the header's expand/collapse control, drawn right after the sender's name.
    var chevron: HeaderChevron?
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: WhooshSpacing.s6) {
                if let open {
                    Button(action: open) { name }
                        .buttonStyle(.plain)
                        .accessibilityIdentifier("mac-main.reader.sender")
                        .accessibilityLabel(author)
                } else { name }
                if let chevron { chevron }
                Spacer(minLength: WhooshSpacing.s8)
                if let time { Text(time).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel).opacity(timeHidden ? 0 : 1) }
            }
            if let address, !address.isEmpty, address != author {   // the address stands in for a missing name: it is not written twice
                Text(address).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                    .truncationMode(.middle)
            }
        }
        .lineLimit(1)
    }

    private var name: some View {
        Text(author).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.label)
    }
}

/// The open message: its meta line (the sender opens the card), the sign-in code chip when it carries one, the whole body
/// the core serves from the first moment (OpenBodyHost: quote fold, tracker count) or the preview and a loading line while it is not downloaded,
/// then one tile per attachment.
struct OpenMessageBlock: View {
    let thread: ReaderThread
    let copied: Bool
    let openSender: () -> Void
    let copyCode: () -> Void
    var stripVisible = false
    var quickOpen = false
    var marked = false
    var links: ReaderLinkRow?
    /// The selected result's passage when the open mail holds it: the text is marked in the body, an attachment passage rings its tile.
    var passage: ReaderPassage?
    var body: some View {
        ReaderBlock(marked: marked, frameName: "message", ax: "mac-main.reader.message.\(thread.earlierCount)") {
            MessageHeader(index: thread.earlierCount, fact: thread.open, fallbackAuthor: thread.sender, openSender: openSender, timeHidden: stripVisible)
                .contentShape(Rectangle())
                .onHover { ReaderState.shared.headerHover = $0 }
                .overlay(alignment: .topTrailing) { ReaderMessageStrip(visible: stripVisible, quickOpen: quickOpen) }
            if let code = thread.open.code { ReaderCodePill(code: code, copied: copied, copy: copyCode) }
            OpenBodyHost(thread: thread, passage: passage)
            if let links { links.modifier(UnderTextShown(messageId: thread.messageId)) }      // the blocks under the text wait for its height (BodySettle.swift)
            // attachments hook: the message's files and inline images as the AttachmentsMac gallery (a bare count keeps the tile).
            #if os(macOS)
            AttachmentSetView(thread: thread).modifier(UnderTextShown(messageId: thread.messageId))
            #endif
        }
    }
}
