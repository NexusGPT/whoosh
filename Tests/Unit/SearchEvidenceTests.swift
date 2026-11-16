import Foundation
import Testing
@testable import WhooshUI

// SPEC A149: every search result SHOWS why it matched, and the count line says how many mails match.
// The owner, 2026-11-11: "makes no sense that this match consultant" (a match whose subject and sender do not show the word),
// "makes no sense to have 1 match every time" (the line read "1 match · 182 close in meaning").
// Each test names its one-line broken copy: the line that, changed in the source, turns the test red.

private func row(_ id: String, thread: String? = nil, sender: String = "Linea Supply", address: String = "orders@linea.example", subject: String = "Quote") -> InboxRow {
    InboxRow(messageId: id, threadId: thread ?? "t-\(id)", accountId: "acc-01", senderName: sender, subject: subject, preview: "Preview line that holds nothing typed", receivedAtMs: 0,
             unread: false, pinned: false, labelNames: [], attachmentCount: 0, threadCount: 1, senderAddress: address)
}

private func hit(_ r: InboxRow, snippet: String = "", _ m: MatchedIn = .subjectWords(ranges: [])) -> SearchHit {
    SearchHit(messageId: r.messageId, score: 1, snippet: snippet, matchedIn: m, attachmentName: nil)
}

private func answer(_ typed: String, _ rows: [InboxRow], _ hits: [SearchHit]) -> [SearchResult] {
    SearchEngine.fromHits(SearchQuery.typed(typed), hits: hits, rows: rows, beforeMs: { _ in 0 }, names: { _ in [] }, engineFiltered: true).results
}

/// the bold pieces of the first result, as plain text, from the one place the row serves them
@MainActor private func bold(_ r: SearchResult) -> [String] { r.evidence.bold(in: SearchMacRows.evidenceSource(r)) }

@MainActor @Test func aTypedWordInsideALongerWordInTheSubjectIsBold() {
    // broken copy: SearchLetters.ranges matches at a word start only (`wordRanges(stem(w), in:)` instead of `text.range(of:)`)
    let r = row("m1", subject: "Re: brightconsultants.example")
    let rs = answer("consultant", [r], [hit(r)])
    #expect(rs.count == 1 && rs[0].evidence.place == .subject)
    #expect(bold(rs[0]) == ["consultant"])
    #expect(rs[0].subjectMarks.map { String(r.subject[$0]) } == ["consultant"])   // the subject the row draws carries the same bold
    #expect(rs[0].evidence.line.isEmpty)                                          // nothing else is drawn: the subject shows why
}

@MainActor @Test func aSenderNameMatchIsBoldInTheSenderAndTheSubjectStaysPlain() {
    // broken copy: SearchLetters.evidence has no `inName` step (the sender's name is skipped)
    let r = row("m1", sender: "Consultant Weekly", address: "news@weekly.example", subject: "Your digest")
    let rs = answer("consultant", [r], [hit(r)])
    #expect(rs[0].evidence.place == .sender)
    #expect(rs[0].senderMarks.map { String(r.senderName[$0]) } == ["Consultant"])
    #expect(rs[0].subjectMarks.isEmpty)
    #expect(bold(rs[0]) == ["Consultant"])
}

@MainActor @Test func aTextOnlyMatchShowsThePassageAroundTheLettersNotTheFirstLine() {
    // broken copy: `evidence` returns `snippet.prefix(...)` as the line instead of `passage(snippet, words:)`
    let r = row("m1", sender: "HARO", address: "haro@digest.example", subject: "Daily digest")
    let text = "Hello team, here is today's list of requests from reporters.\nPlease read the whole list before you answer anyone. " + String(repeating: "More words follow. ", count: 12) + "One reporter wants a consultant for a story about small firms."
    let rs = answer("consultant", [r], [hit(r, snippet: text)])
    #expect(rs[0].evidence.place == .text)
    let line = rs[0].evidence.line
    #expect(line.contains("consultant") && !line.contains("\n") && !line.hasPrefix("Hello team"))
    #expect(bold(rs[0]) == ["consultant"])
    #expect(rs[0].subjectMarks.isEmpty && rs[0].senderMarks.isEmpty)
}

@MainActor @Test func aPassageStartsJustBeforeTheLettersSoANarrowRowStillShowsThem() {
    // broken copy: `passage` keeps the whole text (lo = flat.startIndex), so the letters sit far out in the line a narrow row cuts
    let text = String(repeating: "filler ", count: 40) + "the consultant is named here"
    let p = SearchLetters.passage(text, words: ["consultant"])
    let line = p?.line ?? ""
    #expect(line.hasPrefix("…"))
    let at = line.distance(from: line.startIndex, to: line.range(of: "consultant")?.lowerBound ?? line.endIndex)
    #expect(at <= 25)
}

@MainActor @Test func accentsAndCaseDoNotHideAMatch() {
    // broken copy: the options of `ranges(_:in:)` lose `.diacriticInsensitive`
    let r = row("m1", subject: "Café du matin")
    let rs = answer("CAFE", [r], [hit(r)])
    #expect(bold(rs[0]) == ["Café"])
    let back = row("m2", subject: "Ecole d'été")
    #expect(bold(answer("école", [back], [hit(back)])[0]) == ["Ecole"])
    let name = row("m3", sender: "Élodie Martin", subject: "Hello")
    #expect(answer("elodie", [name], [hit(name)])[0].evidence.place == .sender)
}

@MainActor @Test func severalTypedWordsAreEachBoldWhereTheyOccur() {
    // broken copy: `ranges(_:in:)` looks for `words.first` only
    let r = row("m1", subject: "Linea quotes for the spring run")
    let rs = answer("quote linea", [r], [hit(r)])
    #expect(bold(rs[0]) == ["Linea", "quote"])
    // two words that overlap are one bold piece, not two stacked
    #expect(SearchLetters.ranges(["consult", "sultant"], in: "consultant").count == 1)
}

@MainActor @Test func aRowFoundByMeaningAloneHasNoBoldAndKeepsItsOwnRow() {
    // broken copy: fromHits computes the evidence for every label, not only `label != "meaning"`
    let r = row("m1", subject: "Our consultant plan")   // the typed letters are in it, but the engine found it by meaning
    let rs = answer("consultant", [r], [hit(r, snippet: "a consultant is named", .meaningOnly)])
    #expect(rs[0].close)
    #expect(rs[0].evidence == MatchEvidence())
    #expect(rs[0].subjectMarks.isEmpty && rs[0].senderMarks.isEmpty && rs[0].evidence.line.isEmpty)
}

@MainActor @Test func anAddressOnlyMatchShowsThePassageWhenItHoldsTheLettersAndTheAddressWhenItDoesNot() {
    // broken copy: SearchLetters.evidence drops the last `if !inAddress.isEmpty` fallback, so the second row has no evidence
    let r = row("m1", sender: "Info", address: "info@brightconsultants.example", subject: "Hello")
    let withText = answer("consultant", [r], [hit(r, snippet: "We are a consultant firm")])
    #expect(withText[0].evidence.place == .text && bold(withText[0]) == ["consultant"])
    let without = answer("consultant", [r], [hit(r, snippet: "Hello there")])
    #expect(without[0].evidence.place == .address)
    #expect(without[0].evidence.line == "info@brightconsultants.example" && bold(without[0]) == ["consultant"])
    // the look S1 shows no address in a row: it only draws the address when nothing else holds the letters; with `showsAddress` it comes before the text
    let e = SearchLetters.evidence(words: ["consultant"], subject: "Hello", senderName: "Info", senderAddress: "info@brightconsultants.example",
                                   snippet: "We are a consultant firm", showsAddress: true)
    #expect(e.place == .address)
}

@MainActor @Test func aMatchThatTheEngineNamesInANewWayStillShowsItsEvidence() {
    // broken copy: fromHits goes back to an exhaustive `switch h.matchedIn` that has no arm for the new case (it stops compiling), or drops an unknown
    // case into "meaning" (then this row would be `close` and bold nothing)
    // A new engine case cannot be built here; the nearest stand-in is the fixture's body match, `.subjectWords(ranges: [])` with the letters only in the text.
    let r = row("m1", sender: "HARO", address: "haro@digest.example", subject: "Daily digest")
    let rs = answer("consultant", [r], [hit(r, snippet: "…needs a consultant for a story…")])
    #expect(!rs[0].close && rs[0].matchedIn == "subject" && rs[0].evidence.place == .text)
}

@MainActor private func app() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    setenv("WHOOSH_FIXTURE_PATH", repo.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    return m
}

@MainActor @Test func theRowsTheChecksReadServeTheEvidenceOfEveryMatchingRow() throws {
    // broken copy: SearchMacRows `mac-search.results` omits "evidence_in" / "evidence_bold", or evidenceSource returns the subject for every place
    let a = row("a", subject: "Re: brightconsultants.example")
    let b = row("b", sender: "Consultant Weekly", subject: "Digest")
    let c = row("c", sender: "HARO", address: "h@d.example", subject: "Daily")
    let d = row("d", sender: "Nobody", subject: "Unrelated")
    let m = SearchMacModel.shared
    let before = (m.launched, m.outcome, m.stage, m.restOpen)
    defer { m.launched = before.0; m.outcome = before.1; m.stage = before.2; m.restOpen = before.3 }
    m.launched = true; m.stage = "ranked"
    m.restOpen = true   // A150: a mail found by meaning alone is behind the closed line until it is opened; the row for it is read open
    m.outcome.results = answer("consultant", [a, b, c, d], [hit(a), hit(b), hit(c, snippet: "needs a consultant today"), hit(d, snippet: "x", .meaningOnly)])
    // two statements: calling the macro's result in the same expression aborts the Swift type checker (signal 6, unit run of 2026-11-11 21:37)
    let serve = try #require(SearchMacRows.all["mac-search.results"])
    let served = try serve(app())
    #expect(served.map { $0["evidence_in"] as? String } == ["subject", "sender", "text", "none"])
    #expect(served.map { $0["evidence_bold"] as? [String] } == [["consultant"], ["Consultant"], ["consultant"], []])
    #expect(served.map { $0["close"] as? Bool } == [false, false, false, true])
    #expect(served.prefix(3).allSatisfy { !(($0["evidence_bold"] as? [String]) ?? []).isEmpty })   // every row of the match group serves evidence
}

@MainActor private func model(exact: Int, close: Int) -> SearchMacModel {
    let m = SearchMacModel()
    m.launched = true
    let hits = (0..<(exact + close)).map { i -> SearchResult in
        let r = row("m\(i)", subject: i < exact ? "consultant \(i)" : "other \(i)")
        return answer("consultant", [r], [hit(r, snippet: "", i < exact ? .subjectWords(ranges: []) : .meaningOnly)])[0]
    }
    m.outcome.results = hits
    return m
}

@MainActor @Test func theCountLineSaysTheMatchesAndNothingAboutTheOtherMails() {
    // broken copy: SearchMacModel.countLine appends `" · \(closeCount) close in meaning"`, or counts every result instead of the exact ones
    let three = model(exact: 3, close: 182)
    let line = LanguagePin.with("en") { SearchMacModel.countLine(three.matchedConversations) }
    #expect(line == "3 matches")
    #expect(!line.contains("·") && !line.contains("meaning") && !line.contains("182"))
}

@MainActor @Test func oneMatchReadsOneMatchAndNoMatchReadsNoMatchWhileTheOtherMailsStayListed() {
    // broken copy: countLine has no `n == 0` arm (it reads "0 matches"), or `matched(1)` reads "1 matches"
    let one = model(exact: 1, close: 5)
    let none = model(exact: 0, close: 4)
    let (a, b) = LanguagePin.with("en") { (SearchMacModel.countLine(one.matchedConversations), SearchMacModel.countLine(none.matchedConversations)) }
    #expect(a == "1 match")
    #expect(b == "No match")
    #expect(none.results.isEmpty && none.restCount == 4)   // A150 (SPEC.md A150, "One closed line "All other mail (N)""): the other mails are behind the closed line, counted there
}

@MainActor @Test func theFixtureEngineGivesAPassageAroundATextMatch() {
    // broken copy: FixtureSearch.run goes back to `String(r.body.prefix(SEARCH_SNIPPET_CHARS))` for every hit
    let body = String(repeating: "Opening words of the mail. ", count: 10) + "Please send the revised quotation by Friday."
    let p = FixtureSearchBook.passage(body, terms: ["quotation"]) ?? ""
    #expect(p.contains("quotation") && !p.hasPrefix("Opening words"))
    #expect(FixtureSearchBook.passage(body, terms: ["zzz"]) == nil)
}
