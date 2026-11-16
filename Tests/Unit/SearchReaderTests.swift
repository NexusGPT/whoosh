import Foundation
import Testing
@testable import WhooshUI

// The reader shows the selected search result (SPEC A138, lane search-reader). Run with WHOOSH_FIXTURE_CORE=1 like the neighbours.
// They set the shared search model's state directly: the app model reads that one. Each test puts it back at its end.

@MainActor private func app() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    setenv("WHOOSH_FIXTURE_PATH", repo.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    try #require(m.rows.count >= 3, "the fixture inbox needs three rows")
    return m
}

private func result(_ row: InboxRow) -> SearchResult {
    SearchResult(row: row, score: 1, passage: "", highlights: [], marks: [], subjectMarks: [], attachmentName: nil, attachmentMarks: [], matchedIn: "subject")
}

/// A search that has landed, with the first rows of the inbox as its results in reverse order, so no result is the inbox row that is focused.
@MainActor private func landed(_ m: AppModel, count: Int = 2) -> SearchMacModel {
    let s = SearchMacModel.shared
    m.focusIndex = 0
    s.active = true; s.launched = true; s.loading = false; s.selected = 0
    s.outcome.results = Array(m.rows.dropFirst().prefix(count)).map(result)
    return s
}

@MainActor private func reset(_ s: SearchMacModel) {
    s.active = false; s.launched = false; s.loading = false; s.selected = 0; s.outcome = SearchEngine.Outcome()
}

@MainActor @Test func aSearchThatLandsMakesTheReaderTargetTheFirstResult() throws {
    let m = try app(); let s = landed(m); defer { reset(s) }
    #expect(m.shownMessageId == m.rows[1].messageId)
    #expect(m.shownMessageId != m.focusedMessageId)
    #expect(ReaderFocus().target(m) == m.rows[1].messageId)   // fails on the old code: the target was the focused inbox row
}

@MainActor @Test func anArrowMoveOnTheResultsMakesTheReaderTargetTheNextOne() throws {
    let m = try app(); let s = landed(m); defer { reset(s) }
    #expect(s.move(1))
    #expect(m.shownMessageId == m.rows[2].messageId)          // fails on the old code: still the inbox row
    #expect(s.move(-1))
    #expect(m.shownMessageId == m.rows[1].messageId)
}

@MainActor @Test func aSearchWithNoResultGivesTheReaderNoTarget() throws {
    let m = try app(); let s = landed(m, count: 0); defer { reset(s) }
    #expect(m.focusedMessageId != nil)
    #expect(m.shownMessageId == nil)                          // fails on the old code: the inbox row
    let f = ReaderFocus(); f.ensure(m); f.catchUp(m)
    #expect(f.target(m) == nil)
}

@MainActor @Test func aSearchStillLoadingLeavesTheReaderOnWhatItShowed() throws {
    let m = try app(); let s = landed(m); defer { reset(s) }
    s.loading = true
    #expect(s.holdsReader)
    #expect(m.shownMessageId == m.focusedMessageId)           // no flash to a result or to the empty state before the answer
    s.loading = false
    #expect(!s.holdsReader)
}

@MainActor @Test func escapeGivesTheReaderTheInboxRowBack() throws {
    let m = try app(); let s = landed(m); defer { reset(s) }
    let f = ReaderFocus(); f.ensure(m); f.catchUp(m)
    #expect(s.close())
    #expect(!s.active)
    #expect(m.shownMessageId == m.focusedMessageId)
    f.catchUp(m)
    #expect(f.target(m) == m.focusedMessageId)                // fails on the old code: the target never left the inbox row, so this and the lines above cannot tell; the first test is the red one
}

// Rewritten for A148 (lane reader-result). It was `enterOnAResultStillLeavesTheSearchAndTheReaderTakesThatMail` and asserted the search closed
// and the inbox focus moved to the result; the owner rejects that (2026-11-11): "when I go enter on this email , it doesn't open it, it just goes
// straight to no search". Enter on a result now opens it in the reader with the search and its results kept; the inbox focus does not move.
// broken copy: openResult ends with `leave()` as before (the search closes)
@MainActor @Test func enterOnAResultOpensThatMailAndKeepsTheResults() throws {
    let m = try app(); let s = landed(m); defer { reset(s); m.surface = "mac-main"; MacShellState.shared.focusedPane = .list }
    let id = m.rows[1].messageId, focused = m.focusedMessageId
    #expect(s.openResult())
    #expect(s.active && s.showsResults)
    #expect(s.returnToSearch)
    #expect(m.surface == ReaderState.surface)
    #expect(m.shownMessageId == id)
    #expect(m.focusedMessageId == focused)
}

@MainActor @Test func theSelectedResultIdFollowsTheSelectionOnlyWhileTheSearchIsOpen() throws {
    let m = try app(); let s = landed(m); defer { reset(s) }
    #expect(s.selectedResultId == m.rows[1].messageId)
    s.active = false
    #expect(!s.showsResults)
    #expect(m.shownMessageId == m.focusedMessageId)
}
