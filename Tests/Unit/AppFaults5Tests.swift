import Foundation
import Testing
@testable import WhooshUI

// Three faults of build main81 (docs/lanes/app-faults-5.md). Each test reaches the decision without a window.
// Case 13 (the message block's frame for `measure`) has no unit test: the frame exists only in a mounted view; the judge is
// checks/rig-hidden-check.py `hidden-web-text` on a hidden test copy.

// Case 1: R on a thread with no card and no draft still scrolls to the new reply card (the room, the last block).
@MainActor @Test func theSlabOpeningWithNoCardRevealsTheRoom() {
    #expect(QuickReplyThreadCards.slabTarget(lastCardId: nil) == ThreadCardIds.room)   // MUTANT: no id when there is no card
    #expect(QuickReplyThreadCards.slabTarget(lastCardId: "out-1") == "out-1")
}

@MainActor @Test func theStackGrowingWithNoCardAndNoDraftRevealsTheRoom() {
    #expect(QuickReplyThreadCards.growTarget(lastCardId: nil, firstDraftId: nil) == ThreadCardIds.room)
    #expect(QuickReplyThreadCards.growTarget(lastCardId: nil, firstDraftId: "d-1") == "d-1")
    #expect(QuickReplyThreadCards.growTarget(lastCardId: "out-1", firstDraftId: "d-1") == "out-1")
}

// Case 10: Command-O with no chip selected hands the open message's files to the chips' opener.
@MainActor @Test func commandOHandsTheOpenMessagesFilesToTheAttachmentOpener() {
    #expect(ReaderKeys.handsToAttachments(names: ["a.pdf"], index: 2, openIndex: 2))   // MUTANT: ⌘O stops at the reader's own log
    #expect(!ReaderKeys.handsToAttachments(names: [], index: 2, openIndex: 2))
    #expect(!ReaderKeys.handsToAttachments(names: ["a.pdf"], index: 0, openIndex: 2))
}
