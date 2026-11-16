import Foundation
import Testing
@testable import WhooshUI

// The search screen shows what the engine ranks and never hides a refusal; typed words never become a sender pill
// (SPEC A141 and A142, lane search-screen). Run with WHOOSH_FIXTURE_CORE=1 like the neighbours.
//
// The screen model is driven through its `engineAsk` seam and `receive`, never through the core's search arm and never through a typed key:
// the core's arm asks the meaning model (MeaningSearch) and a typed key wakes `SearchQueryVector`; either would make the test process load the real
// model on a Mac that has the weights. The test engine's own ranking is read from `FixtureSearchBook.run` directly, with no model.

@MainActor private func app() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    setenv("WHOOSH_FIXTURE_PATH", repo.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    return m
}

/// The shared search model with the whole mailbox loaded as its corpus, and everything the tests touch put back by `reset`.
@MainActor private func screen(_ m: AppModel) throws -> SearchMacModel {
    let s = SearchMacModel.shared
    reset(s)
    s.corpus = SearchMacModel.everyRow(m)
    s.active = true
    return s
}

@MainActor private func reset(_ s: SearchMacModel) {
    s.active = false; s.launched = false; s.loading = false; s.selected = 0; s.outcome = SearchEngine.Outcome(); s.edited = [:]
    s.engineAsk = nil; s.stageRead = nil; s.stage = ""; s.stageReason = ""; s.refusal = nil; s.hits = []; s.hitsFor = ""; s.awaiting = ""; s.hitCache = [:]
    s.launchedText = ""; s.shownText = ""; s.held = ""; s.folderRefusals = [:]; s.folderWordsPending = []
}

private final class Asked: @unchecked Sendable {
    private let lock = NSLock(); private var words: [String] = []
    func add(_ w: String) { lock.withLock { words.append(w) } }
    var all: [String] { lock.withLock { words } }
}

private func exact(_ row: InboxRow) -> SearchHit {
    SearchHit(messageId: row.messageId, score: 1, snippet: "", matchedIn: .subjectWords(ranges: []), attachmentName: nil)
}
private func close(_ row: InboxRow) -> SearchHit {
    SearchHit(messageId: row.messageId, score: 0.1, snippet: "", matchedIn: .meaningOnly, attachmentName: nil)
}

/// Rows of the mailbox that are four different threads (a result list holds one row per thread).
@MainActor private func fourThreads(_ s: SearchMacModel) throws -> [InboxRow] {
    var seen = Set<String>()
    let rows = s.corpus.filter { seen.insert($0.threadId).inserted }
    try #require(rows.count >= 4, "the fixture mailbox needs four threads")
    return Array(rows.prefix(4))
}

/// A plain word of a subject in the mailbox that the local engine finds: the old code answered it from there when the core refused.
@MainActor private func aWordTheLocalEngineFinds(_ s: SearchMacModel) throws -> String {
    var found: String?
    search: for r in s.corpus {
        for w in r.subject.split(whereSeparator: { !$0.isLetter }).map(String.init) where w.count >= 6 {
            let q = SearchQuery(w)
            guard q.chips.isEmpty, q.words == [w] else { continue }
            if !SearchEngine.run(q, over: s.corpus, beforeMs: SearchMacModel.monthStartMs, names: { _ in [] }).results.isEmpty { found = w; break search }
        }
    }
    return try #require(found, "no subject word in the fixture mailbox is found by the local engine")
}

// MARK: the screen

@MainActor @Test func aRefusedSearchSaysSoInOneSentenceAndListsNothing() throws {
    let m = try app(); let s = try screen(m); defer { reset(s) }
    let word = try aWordTheLocalEngineFinds(s)
    s.engineAsk = { _ in throw CoreError.Unexpected(reasonWords: "search is not built yet") }
    s.text = word
    #expect(s.launch())
    #expect(s.refusal == "search is not built yet")          // fails on the old code: the refusal was swallowed
    #expect(s.results.isEmpty)                               // fails on the old code: exact words over the loaded rows
    let says = LanguagePin.with("en") { s.says }
    #expect(says == "The search could not run.")
    let sentence = LanguagePin.with("en") { SearchMacModel.refusedSentence }
    #expect(sentence == "The search could not run.")
    let rows = try #require(SearchMacRows.all["mac-search.refused"])
    s.loading = false
    let said = try rows(m)
    #expect(said.first?["words"] as? String == "search is not built yet")
}

@MainActor @Test func aSearchTheEngineAnswersAfterARefusalClearsTheSentence() throws {
    let m = try app(); let s = try screen(m); defer { reset(s) }
    let word = try aWordTheLocalEngineFinds(s)
    let row = try fourThreads(s)[0]
    s.engineAsk = { _ in throw CoreError.Invalid(field: "query", reasonWords: "no") }
    s.text = word
    #expect(s.launch())
    #expect(s.refusal != nil)
    s.engineAsk = { w in s.receive(w, [exact(row)]) }
    #expect(s.launch())
    #expect(s.refusal == nil)
    #expect(s.results.map(\.row.messageId) == [row.messageId])
}

@MainActor @Test func aSecondAnswerForTheSameWordsReplacesTheFirstAndKeepsTheSelectionOnItsMail() throws {
    let m = try app(); let s = try screen(m); defer { reset(s) }
    let rows = try fourThreads(s)
    let word = try aWordTheLocalEngineFinds(s)
    s.engineAsk = { w in s.receive(w, [exact(rows[1])]) }    // the word hits
    s.text = word
    #expect(s.launch())
    s.loading = false                                        // the skeleton has given way
    #expect(s.results.map(\.row.messageId) == [rows[1].messageId])
    #expect(s.selectedResultId == rows[1].messageId)
    let shownBefore = m.shownMessageId
    // the full ranking: the exact ones, then the rest by meaning; the mail the owner is on moves down
    s.receive(word, [exact(rows[0]), exact(rows[1]), close(rows[2]), close(rows[3])])
    // A150 (SPEC.md: "(3) One closed line "All other mail (N)": Enter on it or a click opens the rest"): the two mails found by meaning alone are behind the closed
    // line until it is opened; they were listed under "Close in meaning" before
    #expect(s.results.map(\.row.messageId) == [rows[0], rows[1]].map(\.messageId) && s.restCount == 2)
    #expect(s.toggleRest())
    #expect(s.results.map(\.row.messageId) == rows.map(\.messageId))   // fails on the old code: the answer was dropped
    #expect(s.selected == 1)
    #expect(s.selectedResultId == rows[1].messageId)
    #expect(m.shownMessageId == shownBefore)                 // the reader did not change under the owner
    #expect(s.awaiting.isEmpty)
}

@MainActor @Test func anAnswerForOtherWordsIsDropped() throws {
    let m = try app(); let s = try screen(m); defer { reset(s) }
    let rows = try fourThreads(s)
    let word = try aWordTheLocalEngineFinds(s)
    s.engineAsk = { w in s.receive(w, [exact(rows[1])]) }
    s.text = word
    #expect(s.launch())
    s.loading = false
    s.receive("other words", [exact(rows[0]), exact(rows[2])])
    #expect(s.results.map(\.row.messageId) == [rows[1].messageId])
}

@MainActor @Test func theListHoldsEveryMailTheEngineSendsAndCountsOnlyTheExactOnes() throws {
    let m = try app(); let s = try screen(m); defer { reset(s) }
    let rows = try fourThreads(s)
    let word = try aWordTheLocalEngineFinds(s)
    s.engineAsk = { w in s.receive(w, [exact(rows[0]), exact(rows[1]), close(rows[2]), close(rows[3])]) }
    s.text = word
    #expect(s.launch())
    s.loading = false
    // A150 (SPEC.md A150: the rest lives "behind the closed line"; the divider "Close in meaning" is gone, "Related" stands only above related mail):
    // the list holds the two matches and the closed line counts the other two; opening it lists them
    #expect(s.allResults.count == 4 && s.results.count == 2 && s.restCount == 2)   // fails on the old code: only the mails that share a word
    #expect(s.exactCount == 2)
    #expect(s.toggleRest())
    #expect(s.results.map(\.close) == [false, false, true, true])
    let line = LanguagePin.with("en") { SearchMacModel.matched(s.exactCount) }
    #expect(line == "2 matches")
    let titles = LanguagePin.with("en") { SearchResultsList.titles(related: s.relatedCount, rest: s.restCount, stage: s.stage) }
    #expect(titles.isEmpty)                                  // no related mail: no "Related" line (the old "Close in meaning" is gone)
    let related = LanguagePin.with("en") { SearchResultsList.titles(related: 3, rest: 3, stage: "ranked") }
    #expect(related == ["Related"])
    let many = LanguagePin.with("en") { SearchResultsList.titles(related: 0, rest: 0, stage: "ranked") }
    #expect(many.isEmpty)                                    // every listed mail is exact: no divider
    let says = LanguagePin.with("en") { s.says }
    #expect(!says.contains("Nothing found"))
}

@MainActor @Test func aSearchWithNoExactMatchStillListsTheMailsAndCountsZeroMatches() throws {
    let m = try app(); let s = try screen(m); defer { reset(s) }
    let rows = try fourThreads(s)
    let word = try aWordTheLocalEngineFinds(s)
    s.engineAsk = { w in s.receive(w, rows.map(close)) }
    s.text = word
    #expect(s.launch())
    s.loading = false
    // A150: nothing matches and nothing is related, so the list says "No match" and offers the closed line, which counts the four mails (they were listed before)
    #expect(s.results.isEmpty && s.restCount == 4 && s.exactCount == 0 && s.onMore)
    let line = LanguagePin.with("en") { SearchMacModel.matched(0) }
    #expect(line == "0 matches")
}

// MARK: the test engine

private func row(_ id: String, _ subject: String, _ body: String, at ms: Int64, attachments: [String] = []) -> FixtureSearchBook.Row {
    FixtureSearchBook.Row(id: id, subject: subject, body: body, sender: "Someone", attachments: attachments, receivedMs: ms)
}

@Test func theTestEngineListsEveryMailExactFirstThenByMeaningThenByDate() throws {
    let rows = [row("old-none", "Holiday photos", "the beach", at: 1),
                row("new-none", "Lunch on Friday", "at noon", at: 9),
                row("close", "Garden notes", "tomatoes", at: 2),
                row("exact", "Quote for printing", "twelve thousand", at: 3)]
    let hits = try FixtureSearchBook().run("quote", rows: rows, meaning: [(id: "close", score: 0.02)], everyMail: true)
    #expect(hits.map(\.messageId) == ["exact", "close", "new-none", "old-none"])   // no cut by score: 0.02 is listed; no vector: newest first
    if case .meaningOnly = hits[1].matchedIn {} else { Issue.record("a mail found by meaning alone must say so") }
}

@Test func theTestEngineKeepsTheMatchingMailsAloneWhenNotAskedForEveryMail() throws {
    let rows = [row("a", "Quote for printing", "x", at: 3), row("b", "Holiday photos", "the beach", at: 1)]
    let hits = try FixtureSearchBook().run("quote", rows: rows, meaning: [(id: "b", score: 0.9)])
    #expect(hits.map(\.messageId) == ["a", "b"])             // b only through the meaning list, as before; a mail with nothing is left out
    let alone = try FixtureSearchBook().run("quote", rows: rows + [row("c", "Lunch", "noon", at: 2)], meaning: [])
    #expect(alone.map(\.messageId) == ["a"])
}

@Test func theTestEngineFindsConsultantsWithTheTypedWordConsultant() throws {
    let rows = [row("none", "Lunch", "noon", at: 9),
                row("subject", "Two consultants on Monday", "x", at: 1),
                row("body", "Planning", "The consultants will bring the plan", at: 2)]
    let hits = try FixtureSearchBook().run("consultant", rows: rows, everyMail: true)
    #expect(hits.map(\.messageId) == ["subject", "body", "none"])
    for h in hits.prefix(2) { if case .meaningOnly = h.matchedIn { Issue.record("\(h.messageId) has the word: it is no meaning-only hit") } }
    if case .meaningOnly = hits[2].matchedIn {} else { Issue.record("the mail with no shared word comes last and says meaning") }
}

@Test func theTestEngineStaysQuietOnAnEmptyQuery() {
    #expect(throws: CoreError.self) { try FixtureSearchBook().run("   ", rows: [], everyMail: true) }
}

// MARK: typed words stay words (A142)

@Test func theWordFromInASentenceMakesNoChip() {
    let q = SearchQuery("email from consultant")
    #expect(q.chips.isEmpty)                                 // fails on the old code: a chip "From consultant"
    #expect(q.words == ["email", "from", "consultant"])
    let first = SearchQuery("email from c")
    #expect(first.chips.isEmpty && first.words == ["email", "from", "c"])   // the letter the chip froze on
    let alone = SearchQuery("from Linea Supply")
    #expect(alone.chips.isEmpty && alone.words == ["from", "Linea", "Supply"])
}

@Test func theTypedColonFormStillMakesTheSenderChip() throws {
    let q = SearchQuery("from:laura")
    let chip = try #require(q.chips.first)
    #expect(q.chips.count == 1 && chip.kind == .from && chip.value == "laura")
    #expect(q.words.isEmpty)
    let mixed = SearchQuery("budget from:laura")
    #expect(mixed.chips.map(\.kind) == [.from] && mixed.words == ["budget"])
}

@MainActor @Test func aSmartFolderRuleIsReadLikeTypedTextAndListsTheExactGroupOnly() throws {
    let m = try app(); let s = try screen(m); defer { reset(s) }
    let rows = try fourThreads(s)
    let plain = SearchQuery("from Linea Supply")
    #expect(plain.chips.isEmpty && plain.words == ["from", "Linea", "Supply"])   // no stored-rule special case
    s.engineAsk = { w in s.receive(w, [exact(rows[0]), close(rows[1]), exact(rows[2])]) }
    #expect(s.threads(rule: "meetings") == [rows[0].threadId, rows[2].threadId])   // the mail found by meaning alone is not the folder's
}

@MainActor @Test func aSmartFolderWhoseRuleTheEngineRefusesListsNothingAndSaysSo() throws {
    let m = try app(); let s = try screen(m); defer { reset(s); s.folderRefusals = [:]; s.folderWordsPending = [] }
    let word = try aWordTheLocalEngineFinds(s)
    s.engineAsk = { _ in throw CoreError.Unexpected(reasonWords: "not built yet") }
    #expect(s.threads(rule: word).isEmpty)                   // old code: the local engine's threads
    #expect(s.refusedWords(rule: word) == "not built yet")
    #expect(s.refusedWords(rule: "\"a quoted figure\"") == nil)
}

@MainActor @Test func theLaunchedQueryIsTheWordsTheFieldShows() throws {
    let m = try app(); let s = try screen(m); defer { reset(s) }
    let asked = Asked()
    s.engineAsk = { w in asked.add(w) }
    s.text = "email from consultant"
    #expect(s.query.chips.isEmpty)
    #expect(s.launch())
    #expect(asked.all == ["email from consultant"])          // fails on the old code: from:"consultant" and the words "email"
    #expect(s.launchedQuery.chips.isEmpty)
}

@MainActor @Test func tabOnTheSubjectRowMakesTheChip() throws {
    let m = try app(); let s = try screen(m); defer { reset(s) }
    s.text = "laura"
    let row = try #require(s.suggestions.first { $0.kind == .subject })
    s.accept(row)
    #expect(SearchQuery.typed(s.text).chips.map(\.kind) == [.subject])
}

// MARK: the test core's search arm and the smart folders

private final class Heard: EventSink, @unchecked Sendable {
    private let lock = NSLock(); private var last: [SearchHit] = []
    func onEvent(event: CoreEvent) { if case .searchResults(_, let h) = event { lock.withLock { last = h } } }
    var hits: [SearchHit] { lock.withLock { last } }
}

@Test func aSmartFolderCountDoesNotGrowWithTheRankedTailOfTheSearch() throws {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let c = try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock())
    let saved = FixtureCore.meaningRanker
    defer { FixtureCore.meaningRanker = saved }
    FixtureCore.meaningRanker = { _, _, _ in [] }            // no model is loaded in a test
    let heard = Heard()
    let sub = c.subscribe(stream: .search, sink: heard)
    defer { c.unsubscribe(subscription: sub) }
    _ = try c.run(command: .search(query: "quote"))
    for _ in 0..<300 where heard.hits.isEmpty { Thread.sleep(forTimeInterval: 0.01) }   // the sink is told after the call returns
    let everyMail = heard.hits.count
    let matches = Int(c.ruleMatchCount(ruleWords: "quote"))
    #expect(matches > 0)
    #expect(matches < everyMail, "the search lists every mail; the folder counts matches only")
    #expect(c.ruleMatchCount(ruleWords: "quote") == UInt32(matches))
}
