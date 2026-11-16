import Foundation
import Testing
@testable import WhooshUI

// The suggestions under the field and the pills (SPEC A143, the picked look S1, lane search-screen part 3). Run with WHOOSH_FIXTURE_CORE=1 like the neighbours.
//
// The people are asked of the core by first letters (`PeopleRead`); here the answer is given to `SearchSuggest.build` directly, or read from the test
// engine's pure function. No test sets a search text that wakes `SearchQueryVector`, and none asks the meaning model: the screen model's `text` is its
// held buffer, and the test engine is called with no meaning list.

@MainActor private func app() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    setenv("WHOOSH_FIXTURE_PATH", repo.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    return m
}

@MainActor private func screen(_ m: AppModel) -> SearchMacModel {
    let s = SearchMacModel.shared
    reset(s)
    s.corpus = SearchMacModel.everyRow(m)
    s.active = true
    return s
}

@MainActor private func reset(_ s: SearchMacModel) {
    s.active = false; s.launched = false; s.loading = false; s.selected = 0; s.outcome = SearchEngine.Outcome(); s.edited = [:]
    s.engineAsk = nil; s.stageRead = nil; s.stage = ""; s.stageReason = ""; s.pillNames = [:]; s.refusal = nil; s.hits = []; s.hitsFor = ""; s.awaiting = ""; s.hitCache = [:]
    s.launchedText = ""; s.shownText = ""; s.held = ""; s.folderRefusals = [:]; s.folderWordsPending = []
}

private let claire = PersonSuggestion(name: "Claire Martin", address: "claire.martin@lorvan-hotels.example", fromCount: 24, toCount: 11)
private let dubois = PersonSuggestion(name: "Claire Dubois", address: "c.dubois@mint-brussels.example", fromCount: 3, toCount: 0)

private func source(people: [PersonSuggestion] = [claire, dubois], recents: [String] = ["claire quote"]) -> SearchSuggest.Source {
    var s = SearchSuggest.Source(people: people, labels: ["Claires"], folders: [], recents: recents)
    s.subject = { _ in "Call with Claire on Thursday" }
    return s
}

private func message(_ id: String, from: String, name: String, to: [String] = [], outgoing: Bool = false) -> FixtureFile.Message {
    var m = FixtureFile.Message(id: id, sender: name, from: from, account: "acc-01", subject: "s", preview: "p", time: "09:12", unread: false, pinned: false,
                                attachments: nil, thread_count: nil, trackers_blocked: nil, tab: nil, body: nil, quoted: nil)
    m.to = to; m.outgoing = outgoing
    return m
}

private func row(_ id: String, _ subject: String, from: String = "", sender: String = "Someone", to: [String] = [], attachments: [String] = [], at ms: Int64 = 0,
                 body: String = "") -> FixtureSearchBook.Row {
    FixtureSearchBook.Row(id: id, subject: subject, body: body, sender: sender, attachments: attachments, receivedMs: ms, from: from, to: to)
}

// MARK: the suggestions

@Test func theSuggestionKindsComeInTheDrawnOrder() {
    let rows = SearchSuggest.build(draft: "claire", chips: [], source: source(), recentMax: 3)
    #expect(rows.map(\.kind) == [.search, .from, .to, .from, .subject, .attachment, .label, .recent])   // fails on the old head: person and sender rows
    #expect(rows.map(\.group) == [0, 1, 1, 1, 1, 1, 1, 2])                                                // a thin gap after the search row and before the recents
}

@Test func onePersonGetsAFromRowAndAToRowWithTheirAddressAndABoldStart() throws {
    let rows = SearchSuggest.build(draft: "claire", chips: [], source: source(), recentMax: 3)
    let from = try #require(rows.first { $0.kind == .from && $0.title == "Claire Martin" })
    let to = try #require(rows.first { $0.kind == .to && $0.title == "Claire Martin" })
    #expect(from.detail == claire.address && to.detail == claire.address)
    #expect(from.bold == 0..<6 && to.bold == 0..<6)                                                       // the typed letters
    #expect(from.chip?.kind == .from && from.chip?.value == claire.address)          // a person's pill is sent as the address
    #expect(to.chip?.kind == .to && to.chip?.value == claire.address)
    #expect(!rows.contains { $0.kind == .to && $0.title == "Claire Dubois" })                             // nothing was ever sent to her: From only
    #expect(rows.contains { $0.kind == .from && $0.title == "Claire Dubois" })
}

@Test func aPersonSeenOnlyInSentMailIsOfferedAsTo() throws {
    let karen = PersonSuggestion(name: "Karen Bosch", address: "karen@linea.example", fromCount: 0, toCount: 2)
    let rows = SearchSuggest.build(draft: "kar", chips: [], source: source(people: [karen]), recentMax: 3)
    #expect(rows.contains { $0.kind == .to && $0.title == "Karen Bosch" })
    #expect(!rows.contains { $0.kind == .from })
    // and the test engine counts them from the messages: the recipients of sent mail, the senders of received mail
    let messages = [message("m1", from: "claire.martin@lorvan-hotels.example", name: "Claire Martin"),
                    message("m2", from: "claire.martin@lorvan-hotels.example", name: "Claire Martin"),
                    message("m3", from: "me@home.example", name: "Me", to: ["claire.martin@lorvan-hotels.example", "karen@linea.example"], outgoing: true)]
    let found = FixtureCore.people(in: messages, prefix: "kar")
    #expect(found == [PersonSuggestion(name: "Karen", address: "karen@linea.example", fromCount: 0, toCount: 1)])
    let two = FixtureCore.people(in: messages, prefix: "cla")
    #expect(two == [PersonSuggestion(name: "Claire Martin", address: "claire.martin@lorvan-hotels.example", fromCount: 2, toCount: 1)])
    #expect(FixtureCore.people(in: messages, prefix: "martin").map(\.address) == ["claire.martin@lorvan-hotels.example"])   // a word of the name starts with it
}

@Test func thePeopleReadDecodesWhatTheEngineServes() throws {
    let json = #"[{"name":"Claire Martin","address":"claire.martin@lorvan-hotels.example","from_count":24,"to_count":11}]"#
    let list = try JSONDecoder().decode([PersonSuggestion].self, from: Data(json.utf8))
    #expect(list == [claire])
    #expect(PeopleRead.path("claire") == "people/prefix/claire")
}

@MainActor @Test func tabOnAToRowMakesThePillAndTheQueryStringCarriesIt() throws {
    let m = try app(); let s = screen(m); defer { reset(s) }
    let karen = PersonSuggestion(name: "Karen Bosch", address: "karen@linea.example", fromCount: 0, toCount: 2)
    let rows = SearchSuggest.build(draft: "revised kar", chips: [], source: source(people: [karen]), recentMax: 3)
    let to = try #require(rows.first { $0.kind == .to })
    s.text = "revised kar"
    s.accept(to)
    let q = SearchQuery.typed(s.text)
    #expect(q.chips.map(\.kind) == [.to])
    #expect(q.words == ["revised"])
    let sent = SearchEngine.engineQuery(q)
    #expect(sent == #"to:"karen@linea.example" revised"#)
}

@MainActor @Test func typingFromAndAPersonStaysWordsUntilTabTakesTheRow() throws {
    let m = try app(); let s = screen(m); defer { reset(s) }
    s.text = "from clai"
    #expect(s.query.chips.isEmpty)                                                                        // no pill by typing
    let rows = SearchSuggest.build(draft: "from clai", chips: [], source: source(), recentMax: 3)
    let from = try #require(rows.first { $0.kind == .from })
    #expect(from.replaces == "from clai")                                                                 // taking the row replaces "from clai", not only "clai"
    s.accept(from)
    #expect(s.query.chips.map(\.kind) == [.from] && s.query.words.isEmpty)
    #expect(s.pillNames[claire.address] == "Claire Martin")                                               // the pill draws the name, the query carries the address
}

@Test func typingHasAttachmentOrBeforeJuneMakesNoPillAndOffersTheSuggestion() throws {
    #expect(SearchQuery.typed("has attachment").chips.isEmpty)                                            // fails on the old head: a pill
    #expect(SearchQuery.typed("before june").chips.isEmpty)
    #expect(SearchQuery.typed("after june").chips.isEmpty)
    #expect(SearchQuery.typed("has attachment").words == ["has", "attachment"])
    LanguagePin.with("en") {
        let attach = SearchSuggest.build(draft: "has attachment", chips: [], source: source(people: []), recentMax: 3).first { $0.kind == .filter }
        #expect(attach?.chip?.kind == .attachment && attach?.replaces == "has attachment")
        #expect(attach?.chip?.canonical == "has:attachment")
        let before = SearchSuggest.build(draft: "invoices before june", chips: [], source: source(people: []), recentMax: 3).first { $0.chip?.kind == .before }
        #expect(before?.replaces == "before june")
        #expect(before?.chip?.canonical.hasPrefix("before:") == true && before?.chip?.canonical.hasSuffix("-06-01") == true)
        let after = SearchSuggest.build(draft: "after june", chips: [], source: source(people: []), recentMax: 3).first { $0.chip?.kind == .after }
        #expect(after?.chip?.canonical.hasPrefix("after:") == true && after?.chip?.canonical.hasSuffix("-07-01") == true)   // after the month: the next month's first day
        let year = SearchQuery.english.component(.year, from: Date())
        func sent(_ draft: String, _ kind: SearchChip.Kind) -> String? {
            SearchSuggest.build(draft: draft, chips: [], source: source(people: []), recentMax: 3).first { $0.chip?.kind == kind }?.chip?.canonical
        }
        #expect(sent("before june", .before) == "before:\(year)-06-01")
        #expect(sent("after june", .after) == "after:\(year)-07-01")
        #expect(sent("after 3 june", .after) == "after:\(year)-06-04")
        #expect(sent("before 3 june", .before) == "before:\(year)-06-03")
    }
}

@MainActor @Test func tabOnTheBeforeRowMakesTheDatePillAndTheRestStaysWords() throws {
    let m = try app(); let s = screen(m); defer { reset(s) }
    let row = LanguagePin.with("en") {
        SearchSuggest.build(draft: "invoices before june", chips: [], source: source(people: []), recentMax: 3).first { $0.chip?.kind == .before }
    }
    let before = try #require(row)
    s.text = "invoices before june"
    s.accept(before)
    let q = SearchQuery.typed(s.text)
    #expect(q.chips.map(\.kind) == [.before] && q.words == ["invoices"])
    #expect(q.chips.first?.when != nil)
    let sent = try #require(SearchEngine.engineQuery(q))
    #expect(sent.hasPrefix("before:") && sent.hasSuffix(" invoices"))
}

@Test func everyPillReadsBackFromItsColonFormAndIsSentInThatForm() throws {
    let text = #"from:"Claire Martin" to:"Karen Bosch" subject:"Lorvan" attachment:"quote.pdf" label:"Work" before:2026-06-01 after:2026-01-15 has:attachment revised price"#
    let q = SearchQuery.typed(text)
    #expect(Set(q.chips.map(\.kind)) == [.from, .to, .subject, .attname, .label, .before, .after, .attachment])
    #expect(q.words == ["revised", "price"])
    let sent = try #require(SearchEngine.engineQuery(q))
    for form in [#"from:"Claire Martin""#, #"to:"Karen Bosch""#, #"subject:"Lorvan""#, #"attachment:"quote.pdf""#, #"label:"Work""#, "before:2026-06-01", "after:2026-01-15", "has:attachment"] {
        #expect(sent.contains(form), "\(form) in \(sent)")
    }
    #expect(sent.hasSuffix("revised price"))                                                              // the remaining words follow the pills
}

// MARK: the test engine filters on the same forms, then ranks

@Test func aFromPillFiltersExactlyThenTheWordsRankWhatIsKept() throws {
    let rows = [row("a", "Quote A", from: "orders@linea.example", sender: "Linea Supply", at: 3),
                row("b", "Holiday", from: "orders@linea.example", sender: "Linea Supply", at: 2),
                row("c", "Quote C", from: "x@other.example", sender: "Other", at: 4)]
    let hits = try FixtureSearchBook().run(#"from:"Linea" quote"#, rows: rows, everyMail: true)
    #expect(hits.map(\.messageId) == ["a", "b"])                       // c is filtered out although it has the word; a is exact, b follows
    if case .meaningOnly = hits[1].matchedIn {} else { Issue.record("b holds no typed word: it is listed after the exact match as meaning only") }
    let pillsOnly = try FixtureSearchBook().run(#"from:"Linea""#, rows: rows, everyMail: true)
    #expect(pillsOnly.map(\.messageId) == ["a", "b"] && pillsOnly.allSatisfy { if case .meaningOnly = $0.matchedIn { return false } else { return true } })
}

@Test func theOtherPillsFilterOnTheTestEngineToo() throws {
    let day: Int64 = 86_400_000
    let june = SearchQuery.date(day: "2026-06-01").map { Int64($0.timeIntervalSince1970 * 1000) } ?? 0
    let rows = [row("old", "Old", to: ["Karen Bosch karen@linea.example"], attachments: ["quote.pdf"], at: june - day),
                row("new", "New", to: ["Someone else@x.example"], at: june + day)]
    let to = try FixtureSearchBook().run(#"to:"Karen""#, rows: rows, everyMail: true)
    #expect(to.map(\.messageId) == ["old"])
    let before = try FixtureSearchBook().run("before:2026-06-01", rows: rows, everyMail: true)
    #expect(before.map(\.messageId) == ["old"])
    let after = try FixtureSearchBook().run("after:2026-06-01", rows: rows, everyMail: true)
    #expect(after.map(\.messageId) == ["new"])
    let has = try FixtureSearchBook().run("has:attachment", rows: rows, everyMail: true)
    #expect(has.map(\.messageId) == ["old"])
    let named = try FixtureSearchBook().run(#"attachment:"quote""#, rows: rows, everyMail: true)
    #expect(named.map(\.messageId) == ["old"])
    let subject = try FixtureSearchBook().run(#"subject:"New""#, rows: rows, everyMail: true)
    #expect(subject.map(\.messageId) == ["new"])
}

// MARK: the count line and the notes

@MainActor @Test func theCountLineHasTheTwoNumbers() throws {
    let m = try app(); let s = screen(m); defer { reset(s) }
    var seen = Set<String>()
    let rows = Array(s.corpus.filter { seen.insert($0.threadId).inserted }.prefix(5))
    try #require(rows.count == 5, "the fixture mailbox needs five threads")
    func hit(_ r: InboxRow, close: Bool) -> SearchHit {
        SearchHit(messageId: r.messageId, score: 1, snippet: "", matchedIn: close ? .meaningOnly : .subjectWords(ranges: []), attachmentName: nil)
    }
    s.engineAsk = { w in s.receive(w, [hit(rows[0], close: false), hit(rows[1], close: false), hit(rows[2], close: true), hit(rows[3], close: true), hit(rows[4], close: true)]) }
    s.text = "zzzword"
    #expect(s.launch())
    s.loading = false
    // changed with SPEC A150 (commit c19eb14d7, 2026-11-11): the list is matches, related, then ONE closed line; the three meaning-only hits carry no
    // related mark, so they are the rest: not listed (closeCount, the listed non-matches, is 0) and counted behind the closed line (restCount 3)
    #expect(s.exactCount == 2 && s.closeCount == 0 && s.restCount == 3 && s.relatedCount == 0)
    // changed on the owner's words, 2026-11-11: "makes no sense to have 1 match every time" (the line read "1 match · 182 close in meaning"):
    // the count line says the matches alone, and the other mails are told apart by the divider, with no second number (SPEC A149)
    let line = LanguagePin.with("en") { SearchMacModel.countLine(s.matchedConversations) }
    #expect(line == "2 matches")
    let header = try #require(SearchMacRows.all["mac-search.header"])
    let served = try LanguagePin.with("en") { try header(m) }
    #expect(served.first?["count"] as? String == "2 matches")
    #expect(served.first?["close_line"] == nil)
    #expect(served.first?["rest"] as? Int == 3 && served.first?["related"] as? Int == 0)   // A150: the three other mails are the closed line's count
}

@MainActor @Test func whileTheIndexIsBeingBuiltOneLineSaysHowFarItIs() {
    var building = MeaningIndexBuilder.Status()
    building.indexed = 1240; building.queued = 2360
    // the numbers are written in the app's language too: build the expected ones under the same pin
    let (line, done, whole) = LanguagePin.with("en") { (SearchResultsList.indexLine(building), IndexProgress.number(1240), IndexProgress.number(3600)) }
    #expect(line == "The meaning index is still being built: \(done) of \(whole) mails.")
    #expect(SearchResultsList.indexLine(MeaningIndexBuilder.Status()) == nil)                              // nothing queued: no line
}

@MainActor @Test func aRefusedSearchSaysTheSentenceWithTheEnginesReason() throws {
    let m = try app(); let s = screen(m); defer { reset(s) }
    s.engineAsk = { _ in throw CoreError.Unexpected(reasonWords: "the meaning index is not built") }
    s.text = "zzzword"
    #expect(s.launch())
    s.loading = false
    #expect(s.refusal == "the meaning index is not built" && s.results.isEmpty)
    let served = try #require(SearchMacRows.all["mac-search.refused"])
    let served1 = try served(m)
    #expect(served1.first?["words"] as? String == "the meaning index is not built")
}

// MARK: the people reads of the test engine (the engine's three paths)

@Test func theTestEngineServesThePeopleReadsInTheEnginesShapes() throws {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let c = try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock())
    let prefix = try #require(c.state(path: "people/prefix/lin"))
    let list = try JSONDecoder().decode([PersonSuggestion].self, from: Data(prefix.utf8))
    let linea = try #require(list.first { $0.name == "Linea Supply" })
    #expect(list.count <= 8 && linea.fromCount > 0 && linea.toCount == 0)
    let one = try #require(c.state(path: "people/address/\(linea.address)"))
    #expect(try JSONDecoder().decode(PersonSuggestion.self, from: Data(one.utf8)) == linea)
    #expect(c.state(path: "people/address/nobody@nowhere.example") == "null")
    let count = try #require(c.state(path: "people/count").flatMap { Int($0) })
    #expect(count >= list.count)
    #expect(c.state(path: "people/prefix/zzzzqq") == "[]")
}

@Test func theOwnersOwnAddressesAreNeverListedAndSentMailCountsAsTo() {
    let messages = [message("m1", from: "me@home.example", name: "Me"),
                    message("m2", from: "me@home.example", name: "Me", to: ["karen@linea.example"], outgoing: true),
                    message("m3", from: "karen@linea.example", name: "Karen Bosch")]
    let found = FixtureCore.people(in: messages, owner: ["me@home.example"], prefix: "m")
    #expect(found.isEmpty)
    let karen = FixtureCore.people(in: messages, owner: ["me@home.example"], prefix: "bos")
    #expect(karen == [PersonSuggestion(name: "Karen Bosch", address: "karen@linea.example", fromCount: 1, toCount: 1)])   // both: From first
    #expect(SearchSuggest.build(draft: "bos", chips: [], source: source(people: karen), recentMax: 3).map(\.kind).prefix(3) == [.search, .from, .to])
}

// MARK: the engine's two answers (search/last/stage)

private final class Stage: @unchecked Sendable {
    private let lock = NSLock(); private var v = ""
    var value: String { get { lock.withLock { v } } set { lock.withLock { v = newValue } } }
}

@MainActor private func rows4(_ s: SearchMacModel) throws -> [InboxRow] {
    var seen = Set<String>()
    let rows = s.corpus.filter { seen.insert($0.threadId).inserted }
    try #require(rows.count >= 4, "the fixture mailbox needs four threads")
    return Array(rows.prefix(4))
}
private func hit(_ r: InboxRow, close: Bool) -> SearchHit {
    SearchHit(messageId: r.messageId, score: 1, snippet: "", matchedIn: close ? .meaningOnly : .subjectWords(ranges: []), attachmentName: nil)
}

@MainActor @Test func inTheWordsStageTheOtherMailsWaitUnderALineAndTheRankedAnswerSwapsThemInKeepingTheSelection() throws {
    let m = try app(); let s = screen(m); defer { reset(s) }
    let r = try rows4(s)
    let stage = Stage(); stage.value = "words"
    s.stageRead = { stage.value }
    s.engineAsk = { w in s.receive(w, [hit(r[0], close: false), hit(r[1], close: false), hit(r[2], close: true), hit(r[3], close: true)]) }
    s.text = "zzzword"
    #expect(s.launch())
    s.loading = false
    #expect(s.stage == "words" && s.tailPending)
    #expect(s.results.map(\.row.messageId) == [r[0].messageId, r[1].messageId])      // no tail rows yet (fails on the old head: the date-ordered tail was listed)
    #expect(s.closeCount == 0)
    let waiting = LanguagePin.with("en") { SearchResultsList.titles(related: s.relatedCount, rest: s.restCount, stage: s.stage, tailPending: s.tailPending) }
    #expect(waiting == ["Ranking the other mails by meaning…"])
    s.selected = 1
    // the second answer: the meaning order is in
    stage.value = "ranked"
    s.receive("zzzword", [hit(r[0], close: false), hit(r[1], close: false), hit(r[3], close: true), hit(r[2], close: true)])
    #expect(s.stage == "ranked" && !s.tailPending)
    // A150 (SPEC.md A150 (3)): the ranked mails are behind the closed line (N = 2) until it is opened, then they follow in the engine's order
    #expect(s.results.map(\.row.messageId) == [r[0], r[1]].map(\.messageId) && s.restCount == 2 && s.showsMore)
    #expect(s.toggleRest())
    #expect(s.results.map(\.row.messageId) == [r[0], r[1], r[3], r[2]].map(\.messageId))
    #expect(s.selected == 1 && s.selectedResultId == r[1].messageId)
    let ranked = LanguagePin.with("en") { SearchResultsList.titles(related: s.relatedCount, rest: s.restCount, stage: s.stage, tailPending: s.tailPending) }
    #expect(ranked.isEmpty)   // no related mail: the old "Close in meaning" line is gone
}

@MainActor @Test func whenNoMeaningOrderExistsTheOtherMailsAreListedByDateUnderThatLine() throws {
    let m = try app(); let s = screen(m); defer { reset(s) }
    let r = try rows4(s)
    for stage in ["no_model", "model_failed"] {
        s.stageRead = { stage }
        s.hitsFor = ""; s.hitCache = [:]
        s.engineAsk = { w in s.receive(w, [hit(r[0], close: false), hit(r[2], close: true), hit(r[3], close: true)]) }
        s.text = "zzzword"
        #expect(s.launch())
        s.loading = false
        // A150: with no meaning order nothing is related; the date line stands above the closed line, which counts the two other mails (they were listed before)
        #expect(s.results.count == 1 && s.restCount == 2 && s.showsMore && !s.tailPending)
        let title = LanguagePin.with("en") { SearchResultsList.titles(related: s.relatedCount, rest: s.restCount, stage: s.stage, tailPending: s.tailPending) }
        #expect(title == ["Other mails, by date"])
    }
}

@MainActor @Test func pillsAndNoWordsAnswerOnceAllExactWithNoDivider() throws {
    let m = try app(); let s = screen(m); defer { reset(s) }
    let r = try rows4(s)
    s.stageRead = { "filtered" }
    s.engineAsk = { w in s.receive(w, [hit(r[0], close: false), hit(r[1], close: false)]) }
    s.text = #"from:"x@y.example""#
    #expect(s.launch())
    s.loading = false
    #expect(s.exactCount == 2 && s.closeCount == 0)
    #expect(SearchResultsList.titles(related: s.relatedCount, rest: s.restCount, stage: s.stage, tailPending: s.tailPending).isEmpty)
}

@MainActor @Test func theStageIsReadFromItsJsonStringAndTheTestEngineSaysWhichStageItIsIn() throws {
    #expect(SearchMacModel.decoded("\"words\"") == "words")
    #expect(SearchMacModel.decoded("ranked") == "ranked")
    #expect(SearchMacModel.decoded(nil) == "")
    let rows = [row("a", "Quote", at: 1), row("b", "Other", at: 2)]
    let book = FixtureSearchBook()
    _ = try book.run("quote", rows: rows, meaning: [(id: "b", score: 0.4)], everyMail: true)
    #expect(book.value(["search", "last", "stage"]) as? String == "ranked")
    _ = try book.run("quote", rows: rows, everyMail: true)
    #expect(book.value(["search", "last", "stage"]) as? String == "no_model")
    _ = try book.run(#"subject:"Quote""#, rows: rows, everyMail: true)
    #expect(book.value(["search", "last", "stage"]) as? String == "filtered")
}

@MainActor @Test func theMissingMeaningOrderIsSaidInTheAppsLanguageNeverInTheEnginesEnglish() {
    let fr = LanguagePin.with("fr") { SearchMacModel.stageSentence("no_model") }
    #expect(fr == "Le modèle de recherche n’est pas sur ce Mac. Les mots sont cherchés ; le sens demande le modèle.")
    let en = LanguagePin.with("en") { (SearchMacModel.stageSentence("model_failed"), SearchMacModel.stageSentence("failed"), SearchMacModel.stageSentence("ranked")) }
    #expect(en.0 == "The meaning search could not run this time. Exact matches first, then the other mails by date.")
    #expect(en.1 == "The search could not run." && en.2 == nil)
}


// MARK: the test doors of the checks (fixture search-hold, fixture search-refuse, the sent-mail seed, core run search, a typed letter's probe)

private func sampleCore() throws -> FixtureCore {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    return try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock())
}

private final class Answers: EventSink, @unchecked Sendable {
    private let lock = NSLock(); private var all: [[SearchHit]] = []
    func onEvent(event: CoreEvent) { if case .searchResults(_, let h) = event { lock.withLock { all.append(h) } } }
    var count: Int { lock.withLock { all.count } }
    var lists: [[SearchHit]] { lock.withLock { all } }
}
private func wait(_ done: () -> Bool) { for _ in 0..<300 where !done() { Thread.sleep(forTimeInterval: 0.01) } }

/// Everything a read could count: tab figures, mailbox figures (sorted: a dictionary has no order), each tab's inbox ids, the contacts.
private func figures(_ c: FixtureCore) -> [String] {
    let counts = c.counts()
    var out = counts.tabs.map { "tab \($0.tabId) \($0.unread) \($0.waiting)" }
    out += counts.mailboxes.map { "mailbox \($0.key) \($0.value)" }.sorted()
    for t in counts.tabs.map(\.tabId) { out.append("inbox \(t) " + c.inbox(query: InboxQuery(tabId: t, accountId: nil, labelId: nil)).map(\.messageId).joined(separator: ",")) }
    out.append("contacts " + c.contacts().map(\.name).joined(separator: ","))
    out.append("message_count " + (c.state(path: "index/message_count") ?? ""))
    return out
}

@Test func theSentMailSeedAddsARecipientButChangesNoInboxTabOrUnreadFigure() throws {
    // one core, read before and after the seed is taken out of it: two cores built in one process need not hold the same corpus
    let c = try sampleCore()
    let seeded = c.lock.withLock { c.messages.filter { $0.id.hasPrefix("sent-seed-") }.count }
    #expect(seeded == 3)
    let json = c.state(path: "people/prefix/fay")
    let list = try JSONDecoder().decode([PersonSuggestion].self, from: Data((json ?? "[]").utf8))
    #expect(list == [PersonSuggestion(name: "Fay Lund", address: "fay.lund@fixture-example.com", fromCount: 0, toCount: 3)])
    var with = figures(c)
    c.removeSentSeed()   // the seed only: a second resetMessages would also rebuild the flow rows and contacts the launch seeded after it
    #expect(c.lock.withLock { c.messages.filter { $0.id.hasPrefix("sent-seed-") }.count } == 0)
    var without = figures(c)
    // the one figure that moves is the count of messages the index holds: the seed is mail
    func number(_ line: String) -> Int { Int(line.dropFirst("message_count ".count)) ?? -1 }
    #expect(number(with.removeLast()) == number(without.removeLast()) + 3)
    #expect(with == without, "no inbox, tab, mailbox, unread or contact figure moves")
    #expect(c.state(path: "people/prefix/fay") == "[]")
}

@Test func aCorpusLoadedFromAFileOrTheFullHistoryKeepsItsOwnMessageCount() throws {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let c = try sampleCore()
    try c.loadCorpus(from: root.appendingPathComponent("fixtures/tests-j2.json"))
    #expect(c.state(path: "index/message_count") == "11")
    #expect(c.lock.withLock { c.messages.contains { $0.id.hasPrefix("sent-seed-") } } == false)
}

@Test func searchRefuseRefusesEverySearchWithItsWordsUntilOff() throws {
    let c = try sampleCore()
    c.fixtureSearchRefuse("the index is down")
    do { _ = try c.run(command: .search(query: "quote")); Issue.record("the search was not refused") }
    catch CoreError.Invalid(_, let words) { #expect(words == "the index is down") }
    c.fixtureSearchRefuse(nil)
    #expect(throws: Never.self) { _ = try c.run(command: .search(query: "quote")) }
}

@Test func searchHoldSendsTheFirstAnswerNowAndTheRankedSecondOnlyWhenReleased() throws {
    let c = try sampleCore()
    let saved = FixtureCore.meaningRanker
    defer { FixtureCore.meaningRanker = saved }
    FixtureCore.meaningRanker = { _, ids, _ in ids.map { (id: $0, score: Float(0.5)) } }   // a ranker that answers: the final stage is "ranked"; no model is loaded
    let answers = Answers()
    let sub = c.subscribe(stream: .search, sink: answers)
    defer { c.unsubscribe(subscription: sub) }
    c.fixtureSearchHold(true)
    _ = try c.run(command: .search(query: "quote"))
    wait { answers.count >= 1 }
    #expect(answers.count == 1)
    #expect(c.state(path: "search/last/stage") == "\"words\"")
    let first = try #require(answers.lists.first)
    var seenOther = false
    for h in first {
        if case .meaningOnly = h.matchedIn { seenOther = true } else { #expect(!seenOther, "an exact match came after the other mails") }
    }
    Thread.sleep(forTimeInterval: 0.1)
    #expect(answers.count == 1, "the second answer waits for off")
    c.fixtureSearchHold(false)
    wait { answers.count >= 2 }
    #expect(answers.count == 2)
    #expect(c.state(path: "search/last/stage") == "\"ranked\"")
    #expect(answers.lists.last?.count == first.count)
}

@MainActor @Test func theSocketVerbsRunSearchAsGivenAndTypeRunsTheProbe() throws {
    let m = try app(); _ = m
    let empty = ControlVerbs.handle(#"{"verb":"core","args":["run","search"]}"#)
    #expect(empty.contains("type something to search for"), "the engine's refusal of empty words comes back in its words: \(empty)")
    let ok = ControlVerbs.handle(#"{"verb":"core","args":["run","search","zzzqqq"]}"#)
    #expect(ok.contains("zzzqqq") && !ok.contains("\"ok\":false"), "\(ok)")
    let typed = ControlVerbs.handle(#"{"verb":"type","args":["a"]}"#)
    #expect(!typed.contains("\"ok\":false"), "type still answers ok through the probe: \(typed)")
}
