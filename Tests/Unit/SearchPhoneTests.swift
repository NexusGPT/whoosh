import Foundation
import Testing
@testable import WhooshUI

// Written from docs/JOURNEYS.md, docs/CAPABILITY-CONTRACT.md, docs/CONSTANTS.md and the fixture, never from a built screen.
// A test of a surface whose lane has not landed FAILS on purpose: the failure is the honest state until the lane registers
// its rows and clicks. Each test names, in its trailing comment, the mutation of the target that must turn it red.
// Run the registered-row tests with WHOOSH_FIXTURE_CORE=1 so the shared AppModel serves the design fixture.

private let repoRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func fixtureCore() throws -> (FixtureCore, FixtureClock) {
    let clock = FixtureClock()
    return (try FixtureCore(url: repoRoot.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: clock), clock)
}
private func fixtureJSON() throws -> [String: Any] {
    let data = try Data(contentsOf: repoRoot.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"))
    return try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
}
private func list(_ key: String) throws -> [[String: Any]] { try #require(try fixtureJSON()[key] as? [[String: Any]]) }
@MainActor private func model() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    setenv("WHOOSH_FIXTURE_PATH", repoRoot.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()   // launch state once per test (Tests/Support/SharedAppFresh.swift)
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    return m
}
@MainActor private func rows(_ ax: String) throws -> [ControlRow] {
    let src = try #require(Registrations.rowSource(ax), "no rows are registered at \(ax) yet")
    return try src(try model())
}
private func primary(_ c: FixtureCore) -> Int { Int(c.counts().tabs.first { $0.tabId == "primary" }?.unread ?? 0) }
private func primaryRows(_ c: FixtureCore) -> [InboxRow] { c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)) }

// J2.1-J2.3 parity (C-P-search). The meaning ranking lives in the core (Tests/Integration/SearchSeamTests.swift); here: the phone screen.
@MainActor @Test func theSearchScreenRegistersItsResultsChipsAndStatusRegions() throws {
    for ax in ["iphone-search.results", "iphone-search.filters", "iphone-search.state"] {
        #expect(Registrations.rowSource(ax) != nil, "no rows registered at \(ax)")            // mutant: a region is missing
    }
    #expect(Registrations.click("iphone-search.field.clear") != nil)
}
@MainActor @Test func resultsAreOneRowPerThreadWithTheAccountOfTheMessage() throws {
    let r = try rows("iphone-search.results")
    let threads = r.compactMap { $0["thread"] as? String }
    #expect(Set(threads).count == threads.count)                                              // mutant: a thread shows twice
    #expect(r.allSatisfy { ($0["account"] as? String)?.isEmpty == false })                    // mutant: results carry no account
}
@MainActor @Test func everyChipIsOneOfTheNamedFilterKinds() throws {
    let r = try rows("iphone-search.filters")
    #expect(!r.isEmpty)                                                                       // mutant: the chip row is not drawn
    #expect(r.allSatisfy { ["account", "sender", "date", "attachment"].contains($0["kind"] as? String ?? "") })   // mutant: a chip with an unknown kind
}

// ---- the phone's list follows the Mac's three parts (SPEC A150, docs/lanes/list-p1.md): matches, related mail, one closed line "All other mail"
// MUTANTS (red-first broken copies, exact edits in docs/lanes/gap-phone-rest.md):
//   thePhoneListShowsMatchesThenRelatedAndKeepsTheRestBehindTheClosedLine : SearchPhoneParts.swift `results = restOpen ? everyResult : everyResult.filter { $0.part != .rest }` -> `results = everyResult`
//   aTapOnTheClosedLineOpensTheRestAndASecondTapClosesIt                  : SearchPhoneParts.swift `restOpen.toggle()` -> `restOpen = false`
//   whileTheFirstAnswerWaitsForItsRankingNoOtherMailIsListedAndNoClosedLineShows : SearchPhoneParts.swift `!(stage == "words" && $0.part != .match)` -> `true`
//   theCountLineSaysMatchesAndRelatedInTheMacsWords                       : SearchPhoneParts.swift `SearchMacModel.countLine(matches: matchedCount, related: relatedCount)` -> `SearchMacModel.countLine(matches: everyResult.count, related: 0)`
@MainActor private func partsRig(_ stage: String = "ranked") throws -> (SearchPhoneModel, [SearchResult]) {
    let m = try model()
    let s = SearchPhoneModel.shared
    _ = s.open()
    s.restOpen = false; s.stage = stage
    let rows = Array(SearchMacModel.everyRow(m).prefix(6))
    #expect(rows.count == 6, "the fixture holds six rows to build a list from")
    var made = rows.map { SearchPhoneModel.wrapped($0) }
    made[1].part = .related; made[2].part = .related
    for i in 3..<6 { made[i].part = .rest }
    return (s, made)                                 // one match, two related, three rest
}

@MainActor @Test func thePhoneListShowsMatchesThenRelatedAndKeepsTheRestBehindTheClosedLine() throws {
    let (s, made) = try partsRig()
    defer { _ = s.close() }
    s.applyParts(made)
    #expect(s.results.count == 3, "matches and related mail are listed, the rest is not: \(s.results.count)")
    #expect(s.results.map(\.part) == [.match, .related, .related])
    #expect(s.upperCount == 3 && s.restCount == 3 && s.matchedCount == 1 && s.relatedCount == 2)
    #expect(s.showsMore, "the closed line is on the list")
    #expect(s.everyResult.count == 6, "the whole answer is kept behind the closed line")
}

@MainActor @Test func aTapOnTheClosedLineOpensTheRestAndASecondTapClosesIt() throws {
    let (s, made) = try partsRig()
    defer { _ = s.close() }
    s.applyParts(made)
    #expect(s.toggleRest())
    #expect(s.restOpen && s.results.count == 6, "open: the rest is listed under the closed line, in the engine's order")
    #expect(s.results.map(\.part) == [.match, .related, .related, .rest, .rest, .rest])
    #expect(s.toggleRest())
    #expect(!s.restOpen && s.results.count == 3, "a second tap closes it again")
    let one = made.filter { $0.part != .rest }
    s.applyParts(one)
    #expect(!s.showsMore && !s.toggleRest(), "no rest, no closed line, and a tap on it is refused")
}

@MainActor @Test func whileTheFirstAnswerWaitsForItsRankingNoOtherMailIsListedAndNoClosedLineShows() throws {
    let (s, made) = try partsRig("words")
    defer { _ = s.close() }
    s.applyParts(made)
    #expect(s.results.map(\.part) == [.match], "stage words: only the matches are listed, as on the Mac")
    #expect(!s.showsMore, "the rest is not known yet, so no closed line")
}

@MainActor @Test func theCountLineSaysMatchesAndRelatedInTheMacsWords() throws {
    let (s, made) = try partsRig()
    defer { _ = s.close() }
    s.applyParts(made)
    #expect(s.countLine == Words.text("1 match · %lld related", 2), "\(s.countLine)")
    let none = made.map { r -> SearchResult in var x = r; x.part = .rest; return x }
    s.applyParts(none)
    #expect(s.countLine == Words.text("No match"), "no match and nothing related: \(s.countLine)")
}

@MainActor @Test func theResultsRowsNameTheirPartAndTheClosedLineRowAgreesWithTheCounts() throws {
    let (s, made) = try partsRig()
    defer { _ = s.close() }
    s.applyParts(made)
    s.phase = .results   // the header is served only once an answer is in (SearchPhoneRows.swift iphone-search.header: phase results or none); the rig puts the parts in without a typed search
    let r = try rows("iphone-search.results")
    #expect(r.compactMap { $0["part"] as? String } == ["match", "related", "related"], "every listed row names its part")
    let head = try #require(try rows("iphone-search.header").first)
    #expect(head["matches"] as? Int == 1 && head["related"] as? Int == 2 && head["rest"] as? Int == 3)
    let more = try #require(try rows("iphone-search.more").first, "the closed line is served while there is a rest")
    #expect(more["count"] as? Int == 3 && more["open"] as? Bool == false && more["listed"] as? Int == 0)
    #expect(Registrations.click("iphone-search.more") != nil, "the closed line is a registered control")
}

// ---- the J2 steps of the phone, one test each, on the test engine and the phone's own row sources
// MUTANTS: SearchPhoneSearch.swift `outcome = SearchEngine.fromHits(` -> a hit list with every row twice (grouped), `filters.attachment` ignored in SearchPhoneFilters.swift (narrow), SearchPhoneSave.swift `.saveSmartFolder(name: n, ruleWords: rule)` -> `.saveSmartFolder(name: n, ruleWords: "")` (save)
@MainActor private func typed(_ text: String) throws -> SearchPhoneModel {
    let m = try model()
    let s = SearchPhoneModel.shared
    _ = s.close()
    _ = s.open()
    m.typed = text
    s.settle()
    return s
}

@MainActor @Test func j21TypingLetsTheResultsAppearWithoutReturnOneRowPerThreadWithItsAccount() throws {
    let s = try typed("quote")
    defer { _ = s.close() }
    #expect(s.phase == .results && !s.results.isEmpty, "rows after typing, no Return pressed: \(s.phase)")
    let threads = s.results.map(\.row.threadId)
    #expect(Set(threads).count == threads.count, "one row per thread")
    #expect(s.results.allSatisfy { !$0.row.accountId.isEmpty })
}

@MainActor @Test func j22TheAttachmentFilterNarrowsTheListAndTakingItOffWidensIt() throws {
    let s = try typed("quote")
    defer { _ = s.close() }
    let base = s.results.count
    #expect(s.toggleAttachment())
    #expect(s.results.allSatisfy { $0.row.attachmentCount > 0 }, "every row left has an attachment")
    #expect(s.results.count <= base)
    #expect(try rows("iphone-search.filters").first { $0["kind"] as? String == "attachment" }?["on"] as? Bool == true, "the chip shows the filter")
    #expect(s.toggleAttachment())
    #expect(s.results.count == base, "off: the list is as wide as before")
}

@MainActor @Test func j23AQuotedFigureRanksItsMessageFirst() throws {
    let s = try typed("\"0.84 EUR\"")
    defer { _ = s.close() }
    #expect(s.results.first?.row.messageId == "msg-01", "the quote that holds 0.84 EUR comes first: \(s.results.map(\.row.messageId))")
}

@MainActor @Test func j24SavingTheSearchMakesASmartFolderAndASecondOfTheSameNameIsRefused() throws {
    let s = try typed("quote")
    defer { _ = s.close() }
    #expect(s.startSaving())
    s.name = "Quote hunt"
    #expect(s.confirmName())
    #expect(s.savedAs == "Quote hunt", "saved: \(s.fieldSays)")
    #expect(try #require(s.app.core.smartFolders().first { $0.name == "Quote hunt" }, "the core holds the folder").name == "Quote hunt")
    #expect(s.startSaving())
    s.name = "Quote hunt"
    #expect(s.confirmName())
    #expect(s.savedAs == nil && !s.fieldSays.isEmpty, "the same name again is refused in words")
}
