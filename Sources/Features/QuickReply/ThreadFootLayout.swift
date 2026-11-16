import SwiftUI

/// The one place that says where the thread's blocks sit and how the reply field is filled (board thread-cards B, owner
/// pick 17:57 "B: full-width cards", kept with the reader's "aligned by author" offset: the owner's own blocks, the outgoing card,
/// the draft card and the reply field, share one left edge and one right edge; docs/surfaces/mac-quickreply.md). The reader's message card (ReaderBlock), the draft card
/// (ThreadDraftCard), the outgoing card (ThreadOutgoingCard) and the reply field (ReplyBlock) all read these values, so
/// they can never drift apart. Before this, the reply field was inset 32 pt
/// at the left ("aligned by author") while the draft card spanned the full width: SPEC A147.
enum ThreadFootLayout {
    /// How a block of the thread is filled.
    enum Fill: Equatable {
        /// The frosted lift of the picked look (cards-s3): what is still yours and not sent (the reply field, a draft).
        case lifted
        /// A message that is on the record: the pane's own page with a hairline.
        case flat
    }

    /// The blocks of the thread, bottom to top of the stack: a received message, my outgoing card, the draft card, the closed reply field.
    enum Block: CaseIterable { case message, outgoing, draft, reply }

    /// The reader's picked "aligned by author" offset (s32): a received message stops this far short of the column's right edge; the
    /// owner's own blocks start this far in from its left edge. One constant for every block, both sides.
    static let authorOffset: CGFloat = WhooshSpacing.s32

    /// The insets a block is drawn with. The owner's own blocks (outgoing, draft, reply field) share one pair, so they start and end
    /// at the same x (SPEC A147: the draft card was flush left while the reply field under it was inset); a received message has the mirror.
    static func insets(_ block: Block) -> (leading: CGFloat, trailing: CGFloat) {
        switch block {
        case .message: return (0, authorOffset)
        case .outgoing, .draft, .reply: return (authorOffset, 0)
        }
    }

    /// How each block is filled. The closed reply field and the draft card are lifted glass (lock amendment 00:35: "the reply box
    /// floats as a glass panel"; "every in-flight card ... uses the same lifted glass"), the open slab's own material, never the
    /// blue `mine` tint of a sent message. A received message is flat with its hairline; an outgoing card is lifted until it lands.
    static func fill(_ block: Block) -> Fill {
        switch block {
        case .message: return .flat
        case .outgoing, .draft, .reply: return .lifted
        }
    }

    /// Corner radius of the closed reply field: the slab's own (cards-s3 `.qr{border-radius:16px}`), so the field and the
    /// slab it opens into are one object.
    static let replyFieldRadius: CGFloat = WhooshGlass.Lift.panelRadius
}

extension View {
    /// Puts this block on the thread column's shared left and right edges (outside the card's own padding and lift).
    func threadFootEdges(_ block: ThreadFootLayout.Block) -> some View {
        let e = ThreadFootLayout.insets(block)
        return padding(.leading, e.leading).padding(.trailing, e.trailing)
    }
}
