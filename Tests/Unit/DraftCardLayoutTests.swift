import SwiftUI
import Testing
@testable import WhooshUI

// SPEC A147 (owner 2026-11-11, "weird visual bug"): in a thread with a saved draft the message card, the draft card and the reply
// field the owner's own blocks share their left and right edges (a received message keeps the author offset, mirrored), and the unfocused reply field is the board's lifted glass, not the blue `mine` tint.
// Board: thread-cards B, full-width cards (docs/surfaces/mac-quickreply.md picked-cards); the look is cards-s3, "Lifted until it lands".
// The values are read from ThreadFootLayout, the one place the three blocks (ReaderBlock, ThreadDraftCard, ThreadOutgoingCard) take
// their insets and fill from. MUTANTS: each test names, in its trailing comment, the one-line broken copy that must turn it red.

@MainActor struct DraftCardLayoutTests {
    private typealias F = ThreadFootLayout

    /// The owner's own blocks (outgoing card, draft card, reply field) start at the same x.
    @Test func ownBlocksShareTheirLeftInset() {
        let left = [F.Block.outgoing, .draft, .reply].map { F.insets($0).leading }
        #expect(Set(left).count == 1, "left insets differ: \(left)")
        #expect(left.first == F.authorOffset)
    }   // broken copy: `insets(.draft)` returns `(0, 0)` (the draft card flush left again, the reply field inset)

    /// ... and end at the same x.
    @Test func ownBlocksShareTheirRightInset() {
        let right = [F.Block.outgoing, .draft, .reply].map { F.insets($0).trailing }
        #expect(Set(right).count == 1, "right insets differ: \(right)")
        #expect(right.first == 0)
    }   // broken copy: `insets(.reply)` returns `(authorOffset, authorOffset)`

    /// A received message keeps the picked "aligned by author" offset, mirrored: flush left, the offset short at the right.
    @Test func receivedMessageHasTheMirroredInsets() {
        #expect(F.insets(.message).leading == 0)
        #expect(F.insets(.message).trailing == F.authorOffset)
        #expect(F.insets(.message).leading == F.insets(.draft).trailing && F.insets(.message).trailing == F.insets(.draft).leading)
    }   // broken copy: `insets(.message)` returns `(0, 0)` (the offset removed from messages)

    /// The offset is the reader's own 32 pt, one constant.
    @Test func authorOffsetIsTheReadersThirtyTwo() {
        #expect(F.authorOffset == WhooshSpacing.s32)
    }   // broken copy: `authorOffset = WhooshSpacing.s16`

    /// The unfocused reply field under a draft card is the lifted glass (the open slab's own material), like the draft card.
    @Test func unfocusedReplyFieldIsLiftedGlassNotTheBlueTint() {
        #expect(F.fill(.reply) == .lifted)
        #expect(F.fill(.reply) == F.fill(.draft))
        #expect(F.fill(.reply) != F.fill(.message))
    }   // broken copy: `fill(.reply)` returns `.flat`, or ReaderBlock fills `mine` with `WhooshPalette.mine` again

    /// A received message stays flat with its hairline.
    @Test func receivedMessageStaysFlat() {
        #expect(F.fill(.message) == .flat)
    }   // broken copy: `fill(.message)` returns `.lifted`

    /// The reply field is the slab's shape: one radius, so the closed field and the box it opens into are one object.
    @Test func replyFieldTakesTheSlabsRadius() {
        #expect(F.replyFieldRadius == WhooshGlass.Lift.panelRadius)
    }   // broken copy: `replyFieldRadius = WhooshRadius.sheet`
}
