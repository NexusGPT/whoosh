import Foundation
import Testing
@testable import WhooshUI

// A search lists a row for EVERY hit the engine answers, whatever folder holds the mail (SPEC A141, lane rows-screen): the rows come from one
// `rowsForMessages` read per answer, never from the inbox tabs' first page. Run with WHOOSH_FIXTURE_CORE=1 like the neighbours.
// Each test names, in its comment, the broken copy that must turn it red.

@MainActor private func app() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    setenv("WHOOSH_FIXTURE_PATH", repo.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    return m
}

private func hit(_ id: String) -> SearchHit {
    SearchHit(messageId: id, score: 1, snippet: "", matchedIn: .subjectWords(ranges: []), attachmentName: nil)
}

/// Counts the reads of rows by ids and forwards them to the core.
private final class Reads: @unchecked Sendable {
    private let lock = NSLock()
    private var calls: [[String]] = []
    var all: [[String]] { lock.withLock { calls } }
    func read(_ core: WhooshCore) -> ([String]) -> [InboxRow] {
        { ids in self.lock.withLock { self.calls.append(ids) }; return core.rowsForMessages(messageIds: ids) }
    }
}

@MainActor private func mac(_ m: AppModel, corpus: [InboxRow]? = nil) -> SearchMacModel {
    let s = SearchMacModel.shared
    reset(s)
    s.corpus = corpus ?? SearchMacModel.everyRow(m)
    s.active = true
    return s
}

@MainActor private func reset(_ s: SearchMacModel) {
    s.active = false; s.launched = false; s.loading = false; s.selected = 0; s.outcome = SearchEngine.Outcome(); s.edited = [:]
    s.engineAsk = nil; s.hitRowsRead = nil; s.stageRead = nil; s.stage = ""; s.stageReason = ""; s.pillNames = [:]; s.refusal = nil
    s.hits = []; s.hitsFor = ""; s.awaiting = ""; s.hitCache = [:]; s.hitRowsCache = [:]
    s.launchedText = ""; s.shownText = ""; s.held = ""; s.folderRefusals = [:]; s.folderWordsPending = []
}

@MainActor private func search(_ s: SearchMacModel, answering ids: [String]) {
    s.engineAsk = { w in s.receive(w, ids.map(hit)) }
    s.text = "workshop"
    #expect(s.launch())
    s.loading = false
}

// broken copy: fromHits is given `corpus` again (the sent seed is in no inbox row, so the hit is dropped)
@MainActor @Test func aHitForASentSeedMailIsListedWithItsSubject() throws {
    let m = try app(); let s = mac(m); defer { reset(s) }
    search(s, answering: ["sent-seed-1"])
    let r = try #require(s.results.first, "the sent mail has no row on the screen")
    #expect(r.row.messageId == "sent-seed-1")
    #expect(r.row.subject == "Draft schedule for the workshop")
}

// broken copy: the rows are looked up in `corpus` (emptied here, as it is for a mail beyond the first 50 rows of a tab or in another folder)
@MainActor @Test func aHitForAMailNoInboxRowHoldsIsListed() throws {
    let m = try app(); let id = try #require(m.rows.first).messageId
    let s = mac(m, corpus: []); defer { reset(s) }
    search(s, answering: [id])
    #expect(s.results.map(\.row.messageId) == [id])
}

// broken copy: the rows are sorted by message id (or the ids go to the read in a set), so the engine's order is lost
@MainActor @Test func theRowsKeepTheEnginesOrder() throws {
    let m = try app(); let s = mac(m); defer { reset(s) }
    let inbox = try #require(m.rows.first).messageId
    let order = ["sent-seed-3", inbox, "sent-seed-1"]
    search(s, answering: order)
    #expect(s.results.map(\.row.messageId) == order)
}

// broken copy: the read is made once per hit, or again on each pass over the results
@MainActor @Test func theScreenReadsTheRowsWithOneCallPerAnswer() throws {
    let m = try app(); let s = mac(m); defer { reset(s) }
    let reads = Reads(); s.hitRowsRead = reads.read(m.core)
    let order = ["sent-seed-2", try #require(m.rows.first).messageId, "sent-seed-1"]
    search(s, answering: order)
    _ = s.results; _ = s.results.count; s.runSearch()
    #expect(reads.all == [order])
}

// broken copy: the unknown id is kept as a placeholder row, or the read of an unknown id crashes the list
@MainActor @Test func aHitForAnUnknownMailIsDroppedAndTheOthersStay() throws {
    let m = try app(); let s = mac(m); defer { reset(s) }
    search(s, answering: ["no-such-mail", "sent-seed-2"])
    #expect(s.results.map(\.row.messageId) == ["sent-seed-2"])
}

// broken copy: the test engine's rowsForMessages answers from the inbox (`rows(_:)`) instead of all its messages
@Test func theTestEnginesRowsForMessagesAnswerFromEveryMessageInTheIdsOrder() throws {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let c = try FixtureCore(url: repo.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock())
    let rows = c.rowsForMessages(messageIds: ["sent-seed-2", "nope", "sent-seed-1"])
    #expect(rows.map(\.messageId) == ["sent-seed-2", "sent-seed-1"])
    #expect(rows.first?.threadId == "thr-sent-seed-2")
}

// broken copy: SearchPhoneSearch.build passes `corpus` to fromHits again
@MainActor @Test func theIphoneSearchListsTheSameSentMail() throws {
    let m = try app()
    let p = SearchPhoneModel.shared
    _ = p.close(); defer { _ = p.close(); p.hitRowsRead = nil }
    let reads = Reads(); p.hitRowsRead = reads.read(m.core)
    m.typed = "workshop"
    _ = p.open()
    m.typed = "workshop"
    p.awaiting = "workshop"
    p.receive("workshop", [hit("sent-seed-1")])
    #expect(p.results.map(\.row.messageId) == ["sent-seed-1"])
    #expect(p.results.first?.row.subject == "Draft schedule for the workshop")
    #expect(reads.all == [["sent-seed-1"]])
}
