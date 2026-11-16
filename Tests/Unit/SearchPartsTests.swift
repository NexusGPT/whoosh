import Foundation
import Testing
@testable import WhooshUI

// SPEC A150 (the list in three parts: matches, related, one closed line "All other mail"), A152 (the passage on the row: text, attachment, thread) and A153
// (the sender-domain filter), owner 2026-11-11: "it's subject+body+attachment and they should be all related"; "+thread too btw"; "in search we should also be
// able to filter based on domain of sender btw"; card answers "Same, plus a closed \"All other mail\" line" and "P1 The passage on the row (Recommended)".
// The drawing is docs/design/boards/search-r3-parts/designs.js. Run with WHOOSH_FIXTURE_CORE=1 like the neighbours. Each test names its one-line broken copy.
//
// The engine's new case is `MatchedIn.related` (a mail with none of the typed letters, clearly above the search's average closeness); `.meaningOnly` is "all
// other mail". The answer comes in the engine's order, matches then related then the rest, and nothing here re-orders it.

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
    s.restOpen = false; s.onMore = false
}

private func row(_ id: String, thread: String? = nil, sender: String = "Claire Martin", address: String = "claire.martin@lorvan-hotels.example",
                 subject: String = "Planning for the opening", at: Int64 = 0) -> InboxRow {
    InboxRow(messageId: id, threadId: thread ?? "t-\(id)", accountId: "acc-01", senderName: sender, subject: subject, preview: "Preview that holds nothing typed", receivedAtMs: at,
             unread: false, pinned: false, labelNames: [], attachmentCount: 0, threadCount: 1, senderAddress: address)
}

private func hit(_ r: InboxRow, _ m: MatchedIn, snippet: String = "", file: String? = nil, note: String? = nil) -> SearchHit {
    SearchHit(messageId: r.messageId, score: 1, snippet: snippet, matchedIn: m, attachmentName: file, attachmentNote: note)
}

private func answer(_ typed: String, _ rows: [InboxRow], _ hits: [SearchHit], heads: [String: InboxRow] = [:]) -> [SearchResult] {
    SearchEngine.fromHits(SearchQuery.typed(typed), hits: hits, rows: rows, beforeMs: { _ in 0 }, names: { _ in [] }, engineFiltered: true, heads: heads).results
}

private let match = MatchedIn.subjectWords(ranges: [])

/// A model that holds one answer: two matches (a, b), one related mail (c) and two of the rest (d, e), in the engine's order.
@MainActor private func threeParts(_ stage: String = "ranked") -> (SearchMacModel, [InboxRow]) {
    let rs = (0..<5).map { row("m\($0)", subject: $0 < 2 ? "Revised quote \($0)" : "Other \($0)") }
    let m = SearchMacModel()
    m.launched = true; m.stage = stage
    m.outcome.results = answer("revised", rs, [hit(rs[0], match), hit(rs[1], match), hit(rs[2], .related, snippet: "the total stays under what we discussed"),
                                                hit(rs[3], .meaningOnly), hit(rs[4], .meaningOnly)])
    m.settleLine()
    return (m, rs)
}

// MARK: the three parts

@MainActor @Test func theListIsInThreePartsFromOneAnswerAndTheRestStaysBehindTheClosedLine() {
    // broken copy: SearchMacModel.results returns `allResults` whether or not `restOpen`, or fromHits maps `.related` to `.rest` (part = .rest) so no row is related
    let (m, rs) = threeParts()
    #expect(m.allResults.map(\.part) == [.match, .match, .related, .rest, .rest])
    #expect(m.results.map(\.row.messageId) == rs.prefix(3).map(\.messageId))              // the rest is not listed while the line is closed
    #expect(m.matchedConversations == 2 && m.relatedCount == 1 && m.restCount == 2 && m.upperCount == 3 && m.showsMore)
    #expect(m.toggleRest())
    #expect(m.results.map(\.row.messageId) == rs.map(\.messageId))                         // opened: the rest follows in the engine's order
    #expect(m.results.map(\.close) == [false, false, true, true, true])
}

@MainActor @Test func theDividerSaysRelatedOnlyWhenThereIsRelatedMailAndTheOldCloseInMeaningLineIsGone() {
    // broken copy: SearchResultsList.dividerTitle returns "Related" for the default stage whatever `related` says, or goes back to "Close in meaning"
    let (m, _) = threeParts()
    let with = LanguagePin.with("en") { SearchResultsList.titles(related: m.relatedCount, rest: m.restCount, stage: m.stage, tailPending: m.tailPending) }
    #expect(with == ["Related"])
    let none = LanguagePin.with("en") { SearchResultsList.titles(related: 0, rest: 4, stage: "ranked") }
    #expect(none.isEmpty)
}

@MainActor @Test func theClosedLineOpensAndClosesByEnterAndIsClosedAgainByANewSearch() throws {
    // broken copy: SearchMacActs.launch() does not set `restOpen = false`, or `enter()` has no `if onMore { return toggleRest() }` (Enter then opens a mail)
    let m = try app(); let s = screen(m); defer { reset(s) }
    var seen = Set<String>()
    let rs = Array(s.corpus.filter { seen.insert($0.threadId).inserted }.prefix(5))
    try #require(rs.count == 5, "the fixture mailbox needs five threads")
    s.engineAsk = { w in s.receive(w, [hit(rs[0], match), hit(rs[1], match), hit(rs[2], .related), hit(rs[3], .meaningOnly), hit(rs[4], .meaningOnly)]) }
    s.text = "zzzword"
    #expect(s.launch())
    s.loading = false
    #expect(!s.restOpen && !s.onMore && s.results.count == 3)
    #expect(s.move(1) && s.selected == 1)
    #expect(s.move(1) && s.selected == 2)                       // the last related row
    #expect(s.move(1) && s.onMore && s.selected == 2)           // Down from it reaches the closed line; the reader keeps the result it shows
    #expect(s.enter() && s.restOpen && s.onMore)                // Enter on the line opens the rest
    #expect(s.results.count == 5)
    #expect(s.move(1) && !s.onMore && s.selected == 3)          // Down from the open line is the first of the rest
    #expect(s.move(-1) && s.onMore)                             // and Up goes back to the line
    #expect(s.enter() && !s.restOpen && s.results.count == 3)   // the same key closes it
    #expect(s.enter() && s.restOpen)                            // open again, then a new search
    s.text = "zzzword other"
    #expect(s.launch())
    #expect(!s.restOpen && !s.onMore)                           // every new search closes the line
}

@MainActor @Test func theClosedLineIsClickableAndAClickOpensAndClosesIt() {
    // broken copy: SearchMacFeature has no "mac-search.more" click target, or toggleRest does not flip `restOpen`
    let (m, _) = threeParts()
    #expect(m.toggleRest() && m.restOpen && m.onMore)
    #expect(m.toggleRest() && !m.restOpen)
    #expect(SearchMacFeature.clickTargets()["mac-search.more"] != nil)
}

@MainActor @Test func withNothingMatchedAndNothingRelatedTheListSaysNoMatchAndOffersTheClosedLine() {
    // broken copy: SearchMacModel.settleLine does not select the line when nothing stands above it, or countLine(matches:related:) has no (0, 0) arm returning ""
    let rs = (0..<3).map { row("m\($0)") }
    let m = SearchMacModel()
    m.launched = true; m.stage = "ranked"
    m.outcome.results = answer("skiing", rs, rs.map { hit($0, .meaningOnly) })
    m.settleLine()
    #expect(m.upperCount == 0 && m.results.isEmpty && m.restCount == 3 && m.showsMore && m.onMore)
    #expect(m.selectedResult == nil)                                                    // no result is selected: the line is
    #expect(LanguagePin.with("en") { SearchMacModel.countLine(matches: 0, related: 0) }.isEmpty)   // no count line
    #expect(LanguagePin.with("en") { Words.text("No mail holds these words, and none is close to them.") } == "No mail holds these words, and none is close to them.")
}

// MARK: the count line

@MainActor @Test func theCountLineReadsMatchesAndRelatedInEachForm() {
    // broken copy: countLine(matches:related:) has no `(_, 0)` arm (it prints "3 matches · 0 related"), no `(0, _)` arm (it prints "0 matches · 4 related"), or no singular arms
    LanguagePin.with("en") {
        #expect(SearchMacModel.countLine(matches: 3, related: 4) == "3 matches · 4 related")
        #expect(SearchMacModel.countLine(matches: 1, related: 4) == "1 match · 4 related")
        #expect(SearchMacModel.countLine(matches: 3, related: 1) == "3 matches · 1 related")
        #expect(SearchMacModel.countLine(matches: 1, related: 1) == "1 match · 1 related")
        #expect(SearchMacModel.countLine(matches: 3, related: 0) == "3 matches")
        #expect(SearchMacModel.countLine(matches: 0, related: 4) == "No match · 4 related")
        #expect(SearchMacModel.countLine(matches: 0, related: 1) == "No match · 1 related")
        #expect(SearchMacModel.countLine(matches: 0, related: 0) == "")
    }
}

@MainActor @Test func theCountLineDoesNotCountTheRestAndTheHeaderRowServesTheThreeNumbers() throws {
    // broken copy: the header row serves `matches` as `results.count`, or the count line adds " · \(restCount)"
    let m = try app(); let (s, _) = (screen(m), 0); defer { reset(s) }
    let (built, _) = threeParts()
    s.outcome = built.outcome; s.launched = true; s.stage = "ranked"; s.loading = false
    let headerRows = try #require(SearchMacRows.all["mac-search.header"])   // two statements: the one-expression form crashes the compiler (signal 6)
    let served = try LanguagePin.with("en") { try headerRows(m) }
    #expect(served.first?["count"] as? String == "2 matches · 1 related")
    #expect(served.first?["matches"] as? Int == 2 && served.first?["related"] as? Int == 1 && served.first?["rest"] as? Int == 2)
}

// MARK: the passage on the row

@MainActor @Test func aResultFoundInTheTextHasAThirdLineWithTheTypedLettersInBold() {
    // broken copy: SearchHits.found returns nil for `evidence.place == .text`, or marks nothing (`marks = []`)
    let r = row("m1", subject: "Planning for the opening")
    let rs = answer("revised price", [r], [hit(r, .textWords(ranges: []), snippet: "the revised price per room is in the last column")])
    let f = rs[0].found
    #expect(rs[0].part == .match && f?.source == .text && f?.quoted == false)
    #expect(f.map { x in x.marks.map { String(x.passage[$0]) } } == ["revised", "price"])
}

@MainActor @Test func aResultFoundInAnAttachmentHasAPaperclipLineWithTheFileNameThenThePassage() {
    // broken copy: found() ignores `h.attachmentName` (source stays .text), or puts the passage before the file name
    let r = row("m1", subject: "Planning for the opening")
    let rs = answer("revised price", [r], [hit(r, .attachmentContent, snippet: "… the revised price per room is in the last column …", file: "rooms-2026.xlsx")])
    let f = rs[0].found
    #expect(f?.source == .attachment && f?.fileName == "rooms-2026.xlsx" && f?.lead == "rooms-2026.xlsx")
    #expect(f?.passage.contains("revised price") == true && f?.quoted == false)
}

@MainActor @Test func aRelatedMailShowsItsPassageInQuotesAndARelatedAttachmentKeepsItsNote() {
    // broken copy: found() sets `quoted = false` for the related part, or drops the engine's note ("page 2") from the file's line
    let r = row("m1", subject: "Budget for the pilot")
    let text = answer("revised price", [r], [hit(r, .related, snippet: "can we go through the numbers on Thursday")])
    #expect(text[0].part == .related && text[0].found?.source == .text && text[0].found?.quoted == true && text[0].found?.marks.isEmpty == true)
    let file = answer("revised price", [r], [hit(r, .related, snippet: "we lowered the amount for the second phase", file: "offer-v3.pdf", note: "page 2")])
    #expect(file[0].found?.source == .attachment && file[0].found?.fileName == "offer-v3.pdf · page 2" && file[0].found?.quoted == true)
}

@MainActor @Test func aResultFromAnEarlierMailOfTheThreadShowsAnArrowWithThatMailsSenderAndDate() {
    // broken copy: fromHits ignores `heads` (the row is the earlier mail and the third line has no sender), or found() reads the conversation row's sender
    let head = row("m9", thread: "t1", sender: "Claire Martin", subject: "Call next week?", at: 1_700_000_000_000)
    let earlier = row("m1", thread: "t1", sender: "Marc Petit", subject: "Call next week?", at: 1_699_000_000_000)
    let rs = answer("revised price", [earlier], [hit(earlier, .related, snippet: "the total stays under what we discussed")], heads: ["t1": head])
    #expect(rs.count == 1 && rs[0].row.messageId == "m9" && rs[0].hitMessageId == "m1")   // the conversation's own row, the passage's mail kept
    let f = rs[0].found
    #expect(f?.source == .thread && f?.sender == "Marc Petit" && f?.who?.hasPrefix("Marc Petit · ") == true && f?.quoted == true)
    let same = answer("revised price", [head], [hit(head, .related, snippet: "the total stays under what we discussed")], heads: ["t1": head])
    #expect(same[0].found?.source == .text)                                              // the conversation's own mail: no arrow
}

@MainActor @Test func aResultFoundInItsSubjectOrItsSenderStaysTwoLinesWithTheLettersInBold() {
    // broken copy: found() builds a line from the snippet for every match, whatever `evidence.place` says
    let a = row("a", subject: "Revised quote for the Lorvan project")
    let b = row("b", sender: "Revised Print", subject: "Planning")
    let rs = answer("revised", [a, b], [hit(a, match, snippet: "the revised price"), hit(b, match, snippet: "the revised price")])
    #expect(rs[0].evidence.place == .subject && rs[0].found == nil && !rs[0].subjectMarks.isEmpty)
    #expect(rs[1].evidence.place == .sender && rs[1].found == nil && !rs[1].senderMarks.isEmpty)
}

@MainActor @Test func aMailOfTheRestHasNoPassageLine() {
    // broken copy: found() skips its `guard part != .rest`
    let r = row("m1", subject: "Lunch menu")
    let rs = answer("revised", [r], [hit(r, .meaningOnly, snippet: "revised words inside")])
    #expect(rs[0].part == .rest && rs[0].found == nil)
}

@MainActor @Test func theRowsTheChecksReadServeThePartThePassageAndItsSource() throws {
    // broken copy: SearchMacRows `mac-search.results` omits "part", "passage_line", "source" or "source_name"; or the more row omits its count / open state
    let m = try app(); let s = screen(m); defer { reset(s) }
    let a = row("a", subject: "Planning"), b = row("b", subject: "Budget")
    let head = row("h", thread: "t-c", sender: "Claire Martin", subject: "Call", at: 5)
    let early = row("c", thread: "t-c", sender: "Marc Petit", subject: "Call", at: 4)
    let d = row("d", subject: "Lunch")
    s.launched = true; s.loading = false; s.stage = "ranked"; s.restOpen = true
    s.outcome.results = answer("revised price", [a, b, early, d],
                               [hit(a, .attachmentContent, snippet: "the revised price per room", file: "rooms-2026.xlsx"), hit(b, .related, snippet: "numbers on Thursday"),
                                hit(early, .related, snippet: "the total stays"), hit(d, .meaningOnly)], heads: ["t-c": head])
    s.settleLine()
    let resultRows = try #require(SearchMacRows.all["mac-search.results"])   // two statements: the one-expression form crashes the compiler (signal 6)
    let served = try LanguagePin.with("en") { try resultRows(m) }
    #expect(served.map { $0["part"] as? String } == ["match", "related", "related", "rest"])
    #expect(served.map { $0["source"] as? String } == ["attachment", "text", "thread", nil])
    #expect(served[0]["source_name"] as? String == "rooms-2026.xlsx" && served[2]["source_name"] as? String == "Marc Petit")
    #expect((served[0]["passage_line"] as? String)?.contains("revised price") == true && served[3]["passage_line"] is NSNull)
    let moreRows = try #require(SearchMacRows.all["mac-search.more"])   // two statements: the one-expression form crashes the compiler (signal 6)
    let more = try LanguagePin.with("en") { try moreRows(m) }
    #expect(more.count == 1 && more[0]["count"] as? Int == 1 && more[0]["open"] as? Bool == true && more[0]["selected"] as? Bool == false)
}

// MARK: the stages with no model

@MainActor @Test func whileTheModelHasNotAnsweredThereIsNoRelatedPartAndTheRankingLineStandsInItsPlace() {
    // broken copy: SearchMacModel.allResults does not drop non-matches in stage "words", or titles() returns "Related" for it
    let (m, _) = threeParts("words")
    #expect(m.results.count == 2 && m.relatedCount == 0 && m.restCount == 0 && m.tailPending && !m.showsMore)   // no related mail, no closed line: the rest is not ranked yet
    let t = LanguagePin.with("en") { SearchResultsList.titles(related: m.relatedCount, rest: m.restCount, stage: m.stage, tailPending: m.tailPending) }
    #expect(t == ["Ranking the other mails by meaning…"])
}

@MainActor @Test func withNoModelNothingIsRelatedTheDateLineStandsAboveTheClosedLineThatHoldsTheRest() {
    // broken copy: titles() returns [] for "no_model", or showsMore is false for it
    for stage in ["no_model", "model_failed"] {
        let rs = (0..<3).map { row("m\($0)", subject: $0 == 0 ? "Revised quote" : "Other \($0)") }
        let m = SearchMacModel()
        m.launched = true; m.stage = stage
        m.outcome.results = answer("revised", rs, [hit(rs[0], match), hit(rs[1], .meaningOnly), hit(rs[2], .meaningOnly)])
        m.settleLine()
        #expect(m.results.count == 1 && m.relatedCount == 0 && m.restCount == 2 && m.showsMore && !m.tailPending)
        let t = LanguagePin.with("en") { SearchResultsList.titles(related: m.relatedCount, rest: m.restCount, stage: m.stage, tailPending: m.tailPending) }
        #expect(t == ["Other mails, by date"])
        #expect(LanguagePin.with("en") { SearchMacModel.countLine(matches: m.matchedConversations, related: m.relatedCount) } == "1 match")
    }
}

@MainActor @Test func theBottomBarSaysShowOrHideAllOtherMailWhileTheClosedLineIsSelected() throws {
    // broken copy: MacFootHints.searchSet is missing, or its Enter hint reads "Show all other mail" whether or not the line is open
    let a = try app(); let s = screen(a); defer { reset(s) }
    let (built, _) = threeParts()
    s.outcome = built.outcome; s.launched = true; s.stage = "ranked"; s.loading = false; s.settleLine()
    s.selected = s.upperCount - 1                                      // on the last related row
    #expect(s.move(1) && s.onMore)
    let enterWords = { () -> String? in LanguagePin.with("en") { MacFootHints.hints(a).first { $0.command == "enter" }?.words } }
    #expect(enterWords() == "Show all other mail")
    #expect(s.toggleRest())
    #expect(enterWords() == "Hide all other mail")
}

// MARK: the sender-domain filter

private let lorvan = SenderDomain(domain: "lorvan-hotels.example", people: 3, mails: 9)

@MainActor private func domainSource(_ domains: [SenderDomain]) -> SearchSuggest.Source {
    var s = SearchSuggest.Source(people: [PersonSuggestion(name: "Claire Martin", address: "claire.martin@lorvan-hotels.example", fromCount: 5, toCount: 1)],
                                 labels: [], folders: [], recents: [])
    s.domains = domains
    return s
}

@MainActor @Test func aWordThatStartsASenderDomainOffersFromDomainRightUnderSearchAllMailWithThePeopleCount() {
    // broken copy: SearchSuggest.build adds the domain row after the people rows, or never adds it, or leaves `detail` empty
    let rows = LanguagePin.with("en") { SearchSuggest.build(draft: "lorvan", chips: [], source: domainSource([lorvan]), recentMax: 4) }
    #expect(rows.first?.kind == .search && rows.dropFirst().first?.kind == .domain)
    let d = rows[1]
    #expect(d.title == "lorvan-hotels.example" && d.detail == "3 people" && d.bold == 0..<6 && d.group == 1)
    #expect(d.chip?.kind == .domain && d.chip?.value == "lorvan-hotels.example")
    let at = SearchSuggest.build(draft: "@lou", chips: [], source: domainSource([lorvan]), recentMax: 4)
    #expect(at.contains { $0.kind == .domain } && at.first { $0.kind == .domain }?.replaces == "@lou")   // "@" and letters: the @ goes with the typed word
    let none = SearchSuggest.build(draft: "claire", chips: [], source: domainSource([lorvan]), recentMax: 4)
    #expect(!none.contains { $0.kind == .domain })                                      // the letters must START the domain
    let one = LanguagePin.with("en") { SearchSuggest.build(draft: "lou", chips: [], source: domainSource([SenderDomain(domain: "lorvan.example", people: 1, mails: 2)]), recentMax: 4) }
    #expect(one.first { $0.kind == .domain }?.detail == "1 person")
}

@MainActor @Test func tabOnTheDomainRowMakesThePillFromAtDomainWithItsColonFormAndItCanBeRemoved() throws {
    // broken copy: SearchChip.canonical for .domain writes `from:"<domain>"` (a person), or SearchQuery's colon parser does not read `from:@` as .domain
    let m = try app(); let s = screen(m); defer { reset(s) }
    s.text = "lorvan"
    let row = LanguagePin.with("en") { SearchSuggest.build(draft: "lorvan", chips: [], source: domainSource([lorvan]), recentMax: 4) }[1]
    s.accept(row)
    #expect(s.text == "from:@lorvan-hotels.example")
    let chips = s.query.chips
    #expect(chips.map(\.kind) == [.domain] && chips[0].canonical == "from:@lorvan-hotels.example" && chips[0].value == "lorvan-hotels.example")
    #expect(LanguagePin.with("en") { chips[0].title } == "From @lorvan-hotels.example")
    #expect(SearchEngine.engineQuery(SearchQuery.typed("from:@lorvan-hotels.example revised price")) == "from:@lorvan-hotels.example revised price")   // what the engine is asked, and what a smart folder keeps
    #expect(SearchQuery.typed(#"from:"Claire Martin""#).chips.map(\.kind) == [.from])                                                          // a person's pill is still a person
    #expect(s.removeChip(0) && s.text.isEmpty)
}

@MainActor @Test func theTestEngineKeepsExactlyTheMailsOfTheDomainAndNotItsLookAlikes() throws {
    // broken copy: FixtureSearch.passes has no `.domain` arm (every mail passes), or senderIsAt uses `hasSuffix(domain)` without the "@" or "." before it
    let rows = [FixtureSearchBook.Row(id: "a", subject: "Revised quote", body: "x", sender: "Claire", attachments: [], receivedMs: 3, from: "claire@lorvan-hotels.example"),
                FixtureSearchBook.Row(id: "b", subject: "Revised quote", body: "x", sender: "Hugo", attachments: [], receivedMs: 2, from: "hugo@mail.lorvan-hotels.example"),
                FixtureSearchBook.Row(id: "c", subject: "Revised quote", body: "x", sender: "Fake", attachments: [], receivedMs: 1, from: "fake@notlorvan-hotels.example")]
    let hits = try FixtureSearchBook().run("from:@lorvan-hotels.example revised", rows: rows, everyMail: true)
    #expect(hits.map(\.messageId) == ["a", "b"])
    #expect(SearchEngine.senderIsAt(domain: "lorvan-hotels.example", address: "claire@lorvan-hotels.example"))
    #expect(!SearchEngine.senderIsAt(domain: "lorvan-hotels.example", address: "fake@notlorvan-hotels.example"))
}

@MainActor @Test func theTestEngineAnswersTheSenderDomainsOfItsMail() throws {
    // broken copy: FixtureCore.senderDomains returns [] or ignores the prefix
    let m = try app(); let s = screen(m); defer { reset(s) }
    let address = try #require(s.corpus.map(\.senderAddress).first { $0.contains("@") }, "the fixture mailbox has a sender with an address")
    let domain = String(address[address.index(after: address.lastIndex(of: "@")!)...]).lowercased()
    let found = m.core.senderDomains(prefix: String(domain.prefix(3)))
    let d = try #require(found.first { $0.domain == domain }, "the domain of a sender is offered for its first letters")
    #expect(d.people >= 1 && d.mails >= d.people)
    #expect(m.core.senderDomains(prefix: "zzzzzzzz").isEmpty)
}

// MARK: the test engine marks related mail by its own rule

@MainActor @Test func theTestEngineListsMatchesThenRelatedThenTheRestAndServesTheRelatedCount() throws {
    // broken copy: FixtureSearch.run never replaces `.meaningOnly` with `.related`, or sorts the related ones after the rest
    let rows = [FixtureSearchBook.Row(id: "m0", subject: "Our supplier", body: "x", sender: "Someone", attachments: [], receivedMs: 9),
                FixtureSearchBook.Row(id: "r1", subject: "Notes", body: "linea printwerk atelier quote", sender: "Someone", attachments: [], receivedMs: 1),
                FixtureSearchBook.Row(id: "e2", subject: "Notes", body: "linea", sender: "Someone", attachments: [], receivedMs: 3),
                FixtureSearchBook.Row(id: "e3", subject: "Notes", body: "offer", sender: "Someone", attachments: [], receivedMs: 2),
                FixtureSearchBook.Row(id: "e4", subject: "Lunch", body: "noon", sender: "Someone", attachments: [], receivedMs: 8)]
    let book = FixtureSearchBook()
    let hits = try book.run("supplier", rows: rows, everyMail: true)
    #expect(hits.map(\.messageId) == ["m0", "r1", "e2", "e3", "e4"])                      // the engine's order: the match, the related mail, the rest
    let kinds = hits.map { h -> String in FixtureSearchBook.matchedWord(h.matchedIn) }
    #expect(kinds == ["subject", "related", "meaning", "meaning", "meaning"])             // rule: closeness at least 1.25 times the mean of the mails found by meaning alone
    #expect(book.value(["search", "last", "related"]) as? Int == 1)
}
