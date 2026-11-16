import Foundation
import Testing
@testable import WhooshUI

// The search seam of docs/CAPABILITY-CONTRACT.md C-J2.1-C-J2.5: `Command.search(query:)` answers through the `.search` stream with
// `CoreEvent.searchResults(query:hits:)`; `Command.saveSmartFolder(name:ruleWords:)` creates the folder.
// At this commit FixtureCore refuses both ("the fixture core does not serve this command"), and the Rust core is not wired,
// so these tests fail with that reason. They pass when a core serves search. Ids are those of the design fixture (msg-01 holds 0.84 EUR).

private final class Collector: EventSink, @unchecked Sendable {
    private let lock = NSLock()
    private var all: [CoreEvent] = []
    func onEvent(event: CoreEvent) { lock.withLock { all.append(event) } }
    func hits(for query: String) -> [[SearchHit]] {
        lock.withLock { all.compactMap { if case .searchResults(let q, let h) = $0, q == query { return h } else { return nil } } }
    }
}

private func core() throws -> FixtureCore {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    return try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock())
}

// swift-tests lane: FixtureCore.deliver (FixtureCore.swift, `static func deliver`) hands an event to the sink synchronously only when the caller is on the
// main thread, else through the main queue; a plain `@Test` runs on a pool thread and read the sink before the event arrived (main76: heard 0 / 1 of 2).
// `@MainActor` puts the test on the main thread, which is where the app subscribes.
@MainActor @Test func exactFigureRanksRecordOneFirst() throws {
    let c = try core()
    let sink = Collector()
    _ = c.subscribe(stream: .search, sink: sink)
    let q = "\"0.84 EUR\""
    _ = try c.run(command: .search(query: q))
    let first = try #require(sink.hits(for: q).first)
    #expect(first.first?.messageId == "msg-01")
    #expect(first.first?.snippet.contains("0.84 EUR") == true)
}

@MainActor @Test func theSameSearchTwiceGivesTheSameHits() throws {
    let c = try core()
    let sink = Collector()
    _ = c.subscribe(stream: .search, sink: sink)
    let q = "supplier price quote spring"
    _ = try c.run(command: .search(query: q))
    _ = try c.run(command: .search(query: q))
    let runs = sink.hits(for: q)
    #expect(runs.count == 2)
    #expect(runs.first == runs.last)
}

@Test func aSearchIsKeptInHistoryOnThisDevice() throws {
    let c = try core()
    let q = "supplier price quote spring"
    _ = try c.run(command: .search(query: q))
    // swift-tests lane: a search is KEPT by the separate `recordSearch` command the search bar sends on Enter (SearchMacSave.swift:100; FixtureSearch.swift:22 "Enter on a search: newest first");
    // `.search` only answers. The old test never sent it (main76: the history still held the two seeded searches).
    _ = try c.run(command: .recordSearch(query: q))
    let recent = try #require(c.state(path: "search_history/recent"))
    #expect(recent.contains(q))
}

@MainActor @Test func anEmptyQueryIsRefusedInWordsOrFindsNothing() throws {
    let c = try core()
    let sink = Collector()
    _ = c.subscribe(stream: .search, sink: sink)
    do {
        _ = try c.run(command: .search(query: ""))
        #expect(sink.hits(for: "").allSatisfy { $0.isEmpty })
    } catch let e as CoreError {
        guard case .Invalid(let field, _) = e else { Issue.record("an empty query must fail as Invalid, got \(e)"); return }
        #expect(field == "query")
    }
}

@Test func savingASmartFolderReturnsItsId() throws {
    let c = try core()
    // swift-tests lane: "Supplier quotes" is one of the folders the design fixture already holds (docs/design/fixtures/mac-main-sample.json:576), so saving it again is
    // refused ("the name exists already", FixtureCore.swift saveSmartFolder). The test saves a name the fixture does not hold; the positive control reads the seeded one.
    #expect(c.state(path: "smart_folder/Supplier quotes/exists") == "true", "control: the fixture's own folder reads as existing")
    #expect(c.state(path: "smart_folder/Spring price quotes/exists") == "false", "control: the new name is free before the save")
    let out = try c.run(command: .saveSmartFolder(name: "Spring price quotes", ruleWords: "from Linea Supply before June has attachment"))
    #expect(out.createdId != nil)
    #expect(c.state(path: "smart_folder/Spring price quotes/exists") == "true")
}

@Test func aSecondFolderWithTheSameNameIsRefused() throws {
    let c = try core()
    // swift-tests lane: a name the fixture does not already hold (see savingASmartFolderReturnsItsId), so the FIRST save is the one that succeeds.
    _ = try c.run(command: .saveSmartFolder(name: "Spring price quotes", ruleWords: "a"))
    #expect(throws: CoreError.self) { try c.run(command: .saveSmartFolder(name: "Spring price quotes", ruleWords: "b")) }
}

// Round 3 (S1): the device keeps SEARCH_RECENT_MAX recent searches, newest first (docs/CONSTANTS.md; contract delta 8 adds the record).
private func floodQueries() throws -> [String] {
    struct E: Decodable { let expect: X; struct X: Decodable { let recent_flood_queries: [String] } }
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    return try JSONDecoder().decode(E.self, from: Data(contentsOf: root.appendingPathComponent("fixtures/tests-j2.json"))).expect.recent_flood_queries
}

@Test func theDeviceKeepsOnlyTheNewestRecentSearches() throws {
    let c = try core()
    let all = try floodQueries()
    #expect(all.count == WhooshConstants.SEARCH_RECENT_MAX + 2)
    for q in all { _ = try c.run(command: .search(query: q)); _ = try c.run(command: .recordSearch(query: q)) }   // Enter = search + recordSearch (see aSearchIsKeptInHistoryOnThisDevice)
    let recent = try #require(c.state(path: "search_history/recent"))
    for q in all.suffix(WhooshConstants.SEARCH_RECENT_MAX) { #expect(recent.contains(q), "\(q) is among the newest \(WhooshConstants.SEARCH_RECENT_MAX)") }
    for q in all.prefix(2) { #expect(!recent.contains(q), "\(q) is older than the cap and must be gone") }   // control: the line above proves the probe sees real entries
}
