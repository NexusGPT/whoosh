import Foundation
import Testing
@testable import WhooshUI

// Every action acts on the mail shown while search results show (SPEC A138, lane search-actions). Run with WHOOSH_FIXTURE_CORE=1 like the neighbours.
// They set the shared search model's state directly, as SearchReaderTests does; each test puts it back at its end.

@MainActor private func app() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    setenv("WHOOSH_FIXTURE_PATH", repo.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    try #require(m.rows.count >= 4, "the fixture inbox needs four rows")
    return m
}

private func result(_ row: InboxRow) -> SearchResult {
    SearchResult(row: row, score: 1, passage: "", highlights: [], marks: [], subjectMarks: [], attachmentName: nil, attachmentMarks: [], matchedIn: "subject")
}

/// A search that has landed with the inbox rows 1, 2 and 3 as its results; the inbox row that was open before the search is row 0.
@MainActor private func landed(_ m: AppModel, selecting: Int = 0) -> SearchMacModel {
    let s = SearchMacModel.shared
    m.focusIndex = 0
    s.active = true; s.launched = true; s.loading = false; s.selected = selecting
    s.outcome.results = Array(m.rows[1...3]).map(result)
    return s
}

/// A mail the core has that the inbox list on screen does not hold (another tab's row): what a result can be.
@MainActor private func elsewhere(_ m: AppModel) throws -> InboxRow {
    var found: InboxRow?
    for t in m.tabIds where t != m.tab {
        if found == nil { found = m.core.inbox(query: InboxQuery(tabId: t, accountId: nil, labelId: nil)).first(where: { r in !m.rows.contains { $0.messageId == r.messageId } }) }
    }
    return try #require(found, "the fixture needs a mail outside the open tab")
}

@MainActor private func reset(_ s: SearchMacModel) {
    s.active = false; s.launched = false; s.loading = false; s.selected = 0; s.outcome = SearchEngine.Outcome(); s.edited = [:]
    TriageActions.state.pending = nil
}

@MainActor @Test func markDoneWithResultsShowingArchivesTheSelectedResultAndNotTheInboxRow() throws {
    let m = try app(); let a = m.rows[0].messageId, b = m.rows[2].messageId
    let s = landed(m, selecting: 1); defer { reset(s) }
    #expect(m.shownMessageId == b)
    #expect(CommandRegistry.handler("archive")?(m) == true)
    #expect(!m.rows.contains { $0.messageId == b })           // fails on the old code: the focused inbox row A went and B stayed
    #expect(m.rows.contains { $0.messageId == a })
}

@MainActor @Test func theMailMarkedDoneLeavesTheResultsAndTheSelectionStaysInPlace() throws {
    let m = try app(); let c = m.rows[3].messageId
    let s = landed(m, selecting: 1); defer { reset(s) }
    #expect(CommandRegistry.handler("archive")?(m) == true)
    #expect(s.results.count == 2)                             // fails on the old code: the results kept the mail
    #expect(s.selected == 1)
    #expect(s.selectedResultId == c)                          // the reader follows the next result, now under the selection
    #expect(m.shownMessageId == c)
    #expect(s.notice.isEmpty)                                 // the person did it: no « no longer in your mail » note
}

@MainActor @Test func markDoneOnTheLastResultMovesTheSelectionUpOnePlace() throws {
    let m = try app(); let b = m.rows[2].messageId
    let s = landed(m, selecting: 2); defer { reset(s) }
    #expect(TriageActions.archive(m))
    #expect(s.results.count == 2)
    #expect(s.selected == 1)
    #expect(s.selectedResultId == b)
}

@MainActor @Test func anUndoAfterMarkDonePutsTheMailBackInTheInboxAndTheResultsWaitForTheNextSearch() throws {
    let m = try app(); let b = m.rows[2].messageId
    let s = landed(m, selecting: 1); defer { reset(s) }
    #expect(TriageActions.archive(m))
    #expect(TriageActions.undo(m))
    #expect(m.rows.contains { $0.messageId == b })
    #expect(s.results.count == 2)                             // simple on purpose: the list is not rebuilt, a new search finds it again
}

@MainActor @Test func toggleReadWithResultsShowingFlipsTheSelectedResultWhenItIsNotAmongTheInboxRows() throws {
    let m = try app(); let b = try elsewhere(m)
    let s = landed(m); defer { reset(s) }
    s.outcome.results = [result(b)]
    #expect(m.shownMessageId == b.messageId)
    #expect(m.shownRow?.messageId == b.messageId)
    let before = m.core.message(messageId: b.messageId)?.read
    #expect(TriageActions.toggleRead(m))                      // fails on the old code: the id was looked up in the inbox rows and not found, or the inbox row was flipped
    #expect(m.core.message(messageId: b.messageId)?.read == (before == false))
    #expect(TriageActions.toggleRead(m))                      // the second press flips it back: the shown row follows what the first did
    #expect(m.core.message(messageId: b.messageId)?.read == before)
}

@MainActor @Test func snoozeWithResultsShowingTargetsTheSelectedResultNotTheInboxSelection() throws {
    let m = try app(); let b = m.rows[2].messageId
    TriageActions.state.select([m.rows[0].messageId])         // an inbox selection left from before the search
    let s = landed(m, selecting: 1); defer { reset(s); TriageActions.state.clearSelection() }
    #expect(TriageActions.targets(m) == [b])                  // fails on the old code: the inbox selection
    #expect(TriageActions.openPicker(m, { .snooze(messageIds: $0) }, overlay: Overlay.snooze))
    #expect(TriageActions.state.pending == .snooze(messageIds: [b]))
    TriageActions.close(m)
    #expect(m.perform(.snooze(messageIds: [b], when: .tomorrowMorning)))
    #expect(!s.results.contains { $0.row.messageId == b })    // a snoozed mail leaves the results
}

// broken copy: Sources/Features/QuickReply/QuickReplyModel.swift, in `open(_ m:)` replace `guard let row = m.shownRow else { return false }` with
// `guard m.rows.indices.contains(m.focusIndex) else { return false }; let row = m.rows[m.focusIndex]` (the reply answers the inbox row behind the search, not the result)
@MainActor @Test func replyWithResultsShowingAnswersTheSelectedResultsThreadAndNotTheInboxRowBehindTheSearch() throws {
    let m = try app(); let b = try elsewhere(m)
    let s = landed(m); defer { reset(s) }
    s.outcome.results = [result(b)]
    let behind = m.rows[0]
    try #require(behind.messageId != b.messageId && behind.threadId != b.threadId, "the inbox row behind the search is another conversation than the result")
    let surface = m.surface; defer { m.surface = surface }
    #expect(m.shownMessageId == b.messageId)
    let q = QuickReplyModel()                                 // R: the quick reply opens under the shown mail
    #expect(q.open(m))
    #expect(q.replyMessageId == b.messageId)                  // fails on the old code: the reply answered the inbox row behind the search
    #expect(q.threadId == b.threadId)
    #expect(q.threadId != behind.threadId)
}

@MainActor @Test func theLabelDoorMovesTheSelectedResultWhenItIsNotAmongTheInboxRows() throws {
    let m = try app(); let b = try elsewhere(m)
    let label = try #require(m.core.labels().first { $0.kind == .owner })
    let s = landed(m); defer { reset(s) }
    s.outcome.results = [result(b), result(m.rows[1])]
    let focus = m.focusIndex
    #expect(LabelDoors.move(m, to: label.id))                 // fails on the old code: it moved the focused inbox row
    #expect(!s.results.contains { $0.row.messageId == b.messageId })
    #expect(m.focusIndex == focus)                            // the inbox focus is not the mail acted on
}

@MainActor @Test func theToolbarEnablesOnTheSelectedResultAndNotOnTheInboxRowBehindTheSearch() throws {
    let m = try app(); let b = try elsewhere(m)
    let s = landed(m); defer { reset(s) }
    s.outcome.results = [result(b)]
    let item = try #require(MacToolbarItem.all.first { $0.command == "archive" })
    #expect(MacToolbarRun.enabled(item, m))
    s.outcome.results = []
    #expect(!MacToolbarRun.enabled(item, m))                  // fails on the old code: enabled by the inbox row behind the search
}

@MainActor @Test func withNoSearchOpenEveryActionActsOnTheInboxRowAsBefore() throws {
    let m = try app(); let a = m.rows[0].messageId
    let s = SearchMacModel.shared; defer { reset(s) }
    m.focusIndex = 0
    #expect(!m.resultsShow)
    #expect(m.shownMessageId == a)
    #expect(m.shownRow?.messageId == a)
    #expect(TriageActions.targets(m) == [a])
    let before = m.core.message(messageId: a)?.read
    #expect(TriageActions.toggleRead(m))
    #expect(m.core.message(messageId: a)?.read == (before == false))
    #expect(CommandRegistry.handler("archive")?(m) == true)
    #expect(!m.rows.contains { $0.messageId == a })
}

@MainActor @Test func withASearchStillLoadingTheActionsKeepActingOnTheInboxRow() throws {
    let m = try app(); let a = m.rows[0].messageId
    let s = landed(m); defer { reset(s) }
    s.loading = true
    #expect(!m.resultsShow)
    #expect(TriageActions.targets(m) == [a])                  // the reader keeps what it showed, so an action does too
}
