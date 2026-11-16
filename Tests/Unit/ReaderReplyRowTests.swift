import SwiftUI
import Testing
@testable import WhooshUI

// Item 44036170 (the draft card and the reply field wear one fill): the row `mac-main.reader.reply` says what the closed reply field is
// drawn with, so a hidden check reads "lifted" from the app. MUTANTS: each test names, in its trailing comment, the broken copy that must turn it red.

@MainActor struct ReaderReplyRowTests {
    /// The closed field serves one row naming its fill, the fill `ReaderBlock` draws it by.
    @Test func theClosedReplyFieldServesOneRowNamingItsLiftedGlass() throws {
        let rows = ReplyBlock.rows(quickOpenHere: false)
        let row = try #require(rows.first, "no row for the closed reply field")
        #expect(rows.count == 1)
        #expect(row["ax"] as? String == "mac-main.reader.reply")
        #expect(row["material"] as? String == "lifted")
        #expect(ThreadFootLayout.fill(.reply) == .lifted)
    }   // broken copy: ReaderReply.swift `material(_:)` returns "flat" always; or ThreadFootLayout.fill(.reply) returns .flat (the row and the picture agree, the check sees flat)

    /// While the quick reply is open on the thread the field is not drawn (ReaderPane), so no row answers: a check never reads a field that is not there.
    @Test func noRowAnswersWhileTheQuickReplyIsOpenOnTheThread() {
        #expect(ReplyBlock.rows(quickOpenHere: true).isEmpty)
    }   // broken copy: `rows(quickOpenHere:)` ignores its argument and always returns the row

    /// The word is the fill's own: a flat fill reads "flat", never "lifted".
    @Test func theMaterialWordFollowsTheFill() {
        #expect(ReplyBlock.material(.flat) == "flat")
        #expect(ReplyBlock.material(.lifted) == "lifted")
    }   // broken copy: `fill == .lifted ? "lifted" : "flat"` -> `"lifted"`
}
