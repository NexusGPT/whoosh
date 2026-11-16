import Foundation
import Testing
@testable import WhooshUI

// The reader always shows the selected search result, wherever the mail is, and Enter opens it with the results kept (docs/SPEC.md A148,
// lane reader-result). Owner 2026-11-11: "when I go enter on this email , it doesn't open it, it just goes straight to no search" and
// "we don't even see email". The search is `to:fay` on the test engine: its three hits are the sent seed (`sent-seed-1..3`), mail that is in
// NO inbox tab, so no result is among the loaded inbox rows (`m.rows`). Run with WHOOSH_FIXTURE_CORE=1 like the neighbours.
// Each test names, in its comment, the one-line broken copy that must turn it red.

@MainActor private func app() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    setenv("WHOOSH_FIXTURE_PATH", repo.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    return m
}

@MainActor private func reset(_ s: SearchMacModel, _ m: AppModel) {
    s.release()
    s.active = false; s.launched = false; s.loading = false; s.selected = 0; s.outcome = SearchEngine.Outcome(); s.edited = [:]
    s.engineAsk = nil; s.hitRowsRead = nil; s.stageRead = nil; s.stage = ""; s.stageReason = ""; s.refusal = nil
    s.hits = []; s.hitsFor = ""; s.awaiting = ""; s.hitCache = [:]; s.hitRowsCache = [:]
    s.launchedText = ""; s.shownText = ""; s.held = ""; s.folderRefusals = [:]; s.folderWordsPending = []
    s.returnToSearch = false; s.openedMessageId = nil; s.owns = false
    m.overlay = nil; m.surface = SurfaceSlot.macMain.id; m.typed = ""
    MacShellState.shared.focusedPane = .list
}

/// `to:fay` searched on the test engine through the real path (the core's Search stream), the answer in; the field does not hold the keys.
@MainActor private func searched(_ m: AppModel) throws -> SearchMacModel {
    let s = SearchMacModel.shared
    reset(s, m)
    s.corpus = SearchMacModel.everyRow(m)
    s.active = true
    s.subscribeSearch()
    s.text = "to:fay"
    #expect(s.launch())
    s.loading = false                                        // the skeleton has given way
    let ids = s.results.map(\.row.messageId)
    try #require(ids.count == 3 && ids.allSatisfy { $0.hasPrefix("sent-seed-") }, "to:fay lists the three sent-seed mails, got \(ids)")
    for id in ids { try #require(!m.rows.contains { $0.messageId == id }, "\(id) is in no loaded inbox row") }
    return s
}

/// The same, with the field holding the keys and its words the launched ones: what the owner has on screen before Enter on a result.
@MainActor private func searchedInField(_ m: AppModel) throws -> SearchMacModel {
    let s = try searched(m)
    s.claim()
    try #require(s.holds && m.surface == "mac-search" && !s.fieldMode, "the field holds the keys with the launched words")
    return s
}

// broken copy: ReaderThread.make looks the id up in `m.rows` only (`let r = m.rows.first(where: { $0.messageId == id })`): nil, "No conversation open"
@MainActor @Test func theReaderBuildsTheSelectedResultsThreadWhenTheMailIsInNoInboxRow() throws {
    let m = try app(); let s = try searched(m); defer { reset(s, m) }
    let id = try #require(s.selectedResultId)
    #expect(m.shownMessageId == id)
    let t = try #require(ReaderThread.make(m, messageId: m.shownMessageId, resting: true), "the reader has no thread for the selected result")
    #expect(t.messageId == id)
    #expect(t.threadId == "thr-" + id)
    #expect(ReaderFocus().thread(m, resting: true)?.messageId == id)
}

// broken copy: AppModel.knownRow without its last line (`return s.results.first { $0.row.messageId == messageId }?.row`): a result that is not
// the selected one (the pane still on it during a passing focus, ReaderState's header thread) has no row
@MainActor @Test func everyResultHasItsOwnRowForTheReaderNotOnlyTheSelectedOne() throws {
    let m = try app(); let s = try searched(m); defer { reset(s, m) }
    let other = s.results[1].row.messageId
    #expect(other != s.selectedResultId)
    #expect(m.knownRow(other)?.threadId == "thr-" + other)
    #expect(ReaderThread.make(m, messageId: other, resting: true)?.messageId == other)
    #expect(m.knownRow("no-such-mail") == nil)
}

// broken copy: openResult ends with `leave()` as before (the search closes and the reader takes the focused inbox row)
@MainActor @Test func enterOnAResultOpensThatMailInTheReaderAndTheResultsStay() throws {
    let m = try app(); let s = try searchedInField(m); defer { reset(s, m) }
    let id = try #require(s.selectedResultId), inboxFocus = m.focusedMessageId
    #expect(m.press("Enter"))
    #expect(m.surface == ReaderState.surface)
    #expect(s.active && s.showsResults && !s.holds)
    #expect(s.results.map(\.row.messageId).count == 3)
    #expect(s.selectedResultId == id)
    #expect(m.shownMessageId == id)
    #expect(ReaderState.open(m)?.messageId == id)
    #expect(m.focusedMessageId == inboxFocus)               // the inbox focus did not move to stand for the result
}

// broken copy: openResult does not set `selected = i` (a click on the third result opens the reader on the first one)
@MainActor @Test func aClickOnAnotherResultOpensThatOne() throws {
    let m = try app(); let s = try searched(m); defer { reset(s, m) }
    let third = s.results[2].row.messageId
    #expect(s.openResult(2))
    #expect(s.selected == 2)
    #expect(m.shownMessageId == third)
    #expect(ReaderState.open(m)?.messageId == third)
}

// broken copy: SearchMacModel.enter calls `openResult()` whenever `launched` (not only when the field's words are the launched ones)
@MainActor @Test func enterWithNewWordsInTheFieldRunsTheSearchAndOpensNothing() throws {
    let m = try app(); let s = try searchedInField(m); defer { reset(s, m) }
    s.text = "to:fay invoice"
    #expect(s.fieldMode)
    #expect(m.press("Enter"))
    #expect(m.surface == "mac-search")                      // the owner: "WHEN pressing cmd+k enter is to validate the search"
    #expect(s.launchedText == "to:fay invoice")
}

// broken copy: the reader's "next" and "previous" (ReaderKeys commands) are taken out: Down moves the inbox focus and the reader stays
@MainActor @Test func downInTheReaderMovesToTheNextResultAndTheReaderFollows() throws {
    let m = try app(); let s = try searchedInField(m); defer { reset(s, m) }
    let second = s.results[1].row.messageId, inboxFocus = m.focusIndex
    #expect(m.press("Enter"))
    #expect(m.press("Down"))
    #expect(s.selected == 1)
    #expect(m.surface == ReaderState.surface)
    #expect(m.shownMessageId == second)
    #expect(ReaderThread.make(m, messageId: m.shownMessageId, resting: true)?.messageId == second)
    #expect(m.focusIndex == inboxFocus)
    #expect(m.press("Up"))
    #expect(s.selected == 0)
}

// broken copy: ReaderKeys.back and the reader's Escape binding do not call `backToResults` (Esc lands on the inbox list, the field holds no keys)
@MainActor @Test func escapeFromTheOpenResultReturnsToTheResultsAndASecondEscapeClosesTheSearch() throws {
    let m = try app(); let s = try searchedInField(m); defer { reset(s, m) }
    #expect(m.press("Enter"))
    #expect(m.press("Down"))                                  // the selection the owner moved to in the reader is the one kept
    let id = try #require(s.selectedResultId)
    #expect(m.press("Escape"))
    #expect(m.surface == "mac-search")
    #expect(s.holds)                                          // the field as it was: its words, the keys
    #expect(s.text == "to:fay")
    #expect(s.showsResults && s.selected == 1 && s.selectedResultId == id)
    #expect(m.shownMessageId == id)
    #expect(m.press("Escape"))
    #expect(!s.active)
    #expect(m.surface == SurfaceSlot.macMain.id)
}

// broken copy: openResult ends with `leave()` as before (mark done then acts on the focused inbox row)
@MainActor @Test func markDoneOnTheOpenResultArchivesThatMailAndNotTheInboxRow() throws {
    let m = try app(); let s = try searchedInField(m); defer { reset(s, m) }
    let id = try #require(s.selectedResultId), inbox = m.rows.map(\.messageId)
    #expect(m.press("Enter"))
    #expect(CommandRegistry.handler("archive")?(m) == true)
    #expect(m.lastFailure == nil)
    #expect(m.core.state(path: "message/\(id)/archived") == "true")
    #expect(!s.results.contains { $0.row.messageId == id })  // the mail left the results
    #expect(m.rows.map(\.messageId) == inbox)                 // no inbox row was archived in its place
    #expect(m.shownMessageId == s.selectedResultId)           // the reader follows the next result
}
