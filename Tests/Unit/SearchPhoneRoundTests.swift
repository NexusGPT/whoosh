import Foundation
import Testing
@testable import WhooshUI

// The phone search follows the Mac's rule of 2026-11-11 (SPEC A149 to A153, docs/surfaces/mac-search.md "Round 3"): the thin line "Related", "No match" with the closed
// line kept, the passage line with its source, the typed letters in bold, the sender-domain filter, the mail that holds the passage. Written from the Mac's own
// functions and the design fixture, never from a built screen. Each test names the broken copy that must turn it red (the exact edits are in checks/proofs/g2/9f990234.json).

private let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
@MainActor private func appModel() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    setenv("WHOOSH_FIXTURE_PATH", root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    return m
}
@MainActor private func rows(_ ax: String) throws -> [ControlRow] {
    // two statements: calling the result of #require in one expression crashes the compiler (signal 6)
    let source = try #require(Registrations.rowSource(ax), "no rows are registered at \(ax) yet")
    return try source(try appModel())
}
/// six rows from the fixture, as results: one match, two related, three behind the closed line
@MainActor private func rig(_ stage: String = "ranked") throws -> (SearchPhoneModel, [SearchResult]) {
    let m = try appModel()
    let s = SearchPhoneModel.shared
    _ = s.close(); _ = s.open()
    s.restOpen = false; s.stage = stage
    var made = Array(SearchMacModel.everyRow(m).prefix(6)).map { SearchPhoneModel.wrapped($0) }
    #expect(made.count == 6)
    made[1].part = .related; made[2].part = .related
    for i in 3..<6 { made[i].part = .rest }
    return (s, made)
}
@MainActor private func typedWords(_ text: String) throws -> SearchPhoneModel {
    let m = try appModel()
    let s = SearchPhoneModel.shared
    _ = s.close(); _ = s.open()
    m.typed = text
    s.settle()
    return s
}
private func span(_ s: String, _ word: String) -> [Range<String.Index>] { s.range(of: word).map { [$0] } ?? [] }

// ---- the thin line "Related", "No match"
@MainActor @Test func theRelatedLineStandsWhereRelatedMailStandsAndSaysWhatTheEngineIsDoingWhileItRanks() throws {
    let (s, made) = try rig()
    defer { _ = s.close() }
    s.applyParts(made)
    #expect(s.dividerTitle == Words.text("Related"), "related mail is listed: \(String(describing: s.dividerTitle))")
    let line = try #require(try rows("iphone-search.divider").first, "the divider is served as a row")
    #expect(line["title"] as? String == Words.text("Related") && line["before"] as? Int == 1, "it stands after the one match")
    s.applyParts(made.map { var x = $0; if x.part == .related { x.part = .rest }; return x })
    #expect(s.dividerTitle == nil, "no related mail, no line")
    s.stage = "words"
    s.applyParts(made)
    #expect(s.tailPending && s.dividerTitle == Words.text("Ranking the other mails by meaning…"), "the first answer waits for its ranking: \(String(describing: s.dividerTitle))")
    #expect(s.results.map(\.part) == [.match], "no other mail is listed meanwhile")
}

@MainActor @Test func theListSaysNoMatchWhenNothingMatchesAndNothingIsRelatedAndKeepsTheClosedLine() throws {
    let (s, made) = try rig()
    defer { _ = s.close() }
    s.phase = .results
    s.applyParts(made.map { var x = $0; x.part = .rest; return x })
    #expect(s.showsNoMatch && s.showsMore, "no match, no related mail: the sentence stands and the closed line stays under it")
    #expect(s.countLine == Words.text("No match"))
    let empty = try #require(try rows("iphone-search.empty").first)
    #expect(empty["title"] as? String == Words.text("No match") && empty["line"] as? String == Words.text("No mail holds these words, and none is close to them."))
    s.applyParts(made)
    #expect(!s.showsNoMatch, "a match and related mail are listed")
    s.stage = "words"
    s.applyParts(made.map { var x = $0; x.part = x.part == .match ? .rest : x.part; return x })
    #expect(!s.showsNoMatch, "while the ranking waits the list does not say No match")
}

@MainActor @Test func thePhoneGroupingAndItsWordsAreTheMacsOwnFunctionsOnTheSameResults() throws {
    let (s, made) = try rig()
    defer { _ = s.close() }
    for stage in ["ranked", "words", "no_model", "model_failed"] {
        s.stage = stage
        s.applyParts(made)
        #expect(s.dividerTitle == SearchResultsList.titles(related: s.relatedCount, rest: s.restCount, stage: stage, tailPending: s.tailPending).first, "stage \(stage)")
        #expect(s.countLine == (SearchMacModel.countLine(matches: s.matchedCount, related: s.relatedCount).isEmpty ? SearchMacModel.countLine(0)
                                : SearchMacModel.countLine(matches: s.matchedCount, related: s.relatedCount)), "stage \(stage)")
    }
    s.stage = "no_model"
    s.applyParts(made)
    #expect(s.dividerTitle == Words.text("Other mails, by date"), "no meaning order: the line says so, as on the Mac")
}

// ---- the passage line and the bold letters
@MainActor @Test func aResultFoundInAnAttachmentTheTextOrTheThreadShowsItsPassageAndItsSource() throws {
    let (s, made) = try rig()
    defer { _ = s.close() }
    var m = made
    let text = "the unit price at 12,000 is 0.84 EUR"
    m[0].found = SearchFound(source: .attachment, passage: text, marks: span(text, "0.84"), quoted: false, fileName: "quote-Q1-print-run.pdf")
    m[1].found = SearchFound(source: .thread, passage: "price by Friday", marks: [], quoted: true, who: "Marc Petit \u{B7} 12 Oct", sender: "Marc Petit")
    m[2].found = SearchFound(source: .text, passage: "delivery in week 4", marks: [], quoted: true)
    s.applyParts(m)
    let r = try rows("iphone-search.results")
    #expect(r.compactMap { $0["found"] as? String } == ["attachment", "thread", "text"], "each row names where its passage is from: \(r.map { $0["found"] ?? "" })")
    #expect(r[0]["found_line"] as? String == "quote-Q1-print-run.pdf \(text)", "the file's name, then the passage: \(r[0]["found_line"] ?? "")")
    #expect(r[1]["found_line"] as? String == "Marc Petit \u{B7} 12 Oct \u{201C}price by Friday\u{201D}", "the earlier mail's sender and date, the passage in quotes: \(r[1]["found_line"] ?? "")")
    #expect(r[2]["found_line"] as? String == "\u{201C}delivery in week 4\u{201D}")
    s.applyParts(made)
    #expect(try rows("iphone-search.results").allSatisfy { ($0["found"] as? String) == "" && ($0["found_line"] as? String) == "" }, "a result found in its subject or sender has no third line")
}

@MainActor @Test func theTypedLettersOfAMatchAreBoldInTheSenderTheSubjectAndThePassageAndARelatedMailShowsNone() throws {
    let (s, made) = try rig()
    defer { _ = s.close() }
    var m = made
    let row = m[0].row
    let word = String(row.senderName.prefix(2)), sub = String(row.subject.prefix(3))
    let text = "a passage with \(word) inside"
    m[0] = SearchResult(row: row, score: 1, passage: "", highlights: [], marks: [], subjectMarks: span(row.subject, sub), attachmentName: nil, attachmentMarks: [], matchedIn: "subject",
                        part: .match, found: SearchFound(source: .text, passage: text, marks: span(text, word), quoted: false), senderMarks: span(row.senderName, word))
    m[1].senderMarks = span(m[1].row.senderName, String(m[1].row.senderName.prefix(2)))
    s.applyParts(m)
    #expect(SearchPhoneFound.boldWords(m[0]) == [word, sub, word], "the sender's letters, the subject's, the passage's: \(SearchPhoneFound.boldWords(m[0]))")
    let r = try rows("iphone-search.results")
    #expect(r[0]["bold"] as? [String] == [word, sub, word])
    #expect(r[1]["bold"] as? [String] == [], "a related mail shows no bold letters")
}

// ---- the sender-domain filter (SPEC A153)
@MainActor @Test func typingADomainOffersFromDomainWithItsPeopleAndChoosingItMakesThePillFromAtDomain() throws {
    let s = try typedWords("linea")
    defer { _ = s.close() }
    let offer = try #require(s.domainOffers.first, "typing the start of a sender's domain offers it")
    #expect(offer.title == "linea-supply-example.com" && offer.detail == Words.text("1 person"), "\(offer.title) · \(offer.detail)")
    let r = try #require(try rows("iphone-search.domain").first)
    #expect(r["title"] as? String == Words.text("From domain") && r["domain"] as? String == "linea-supply-example.com" && r["people"] as? String == Words.text("1 person"))
    #expect(Registrations.click("iphone-search.domain.0") != nil, "the offer is a registered control")
    #expect(s.chooseDomain(0))
    let chip = try #require(try rows("iphone-search.filters").first { $0["kind"] as? String == "domain" }, "the pill is under the field")
    #expect(chip["label"] as? String == Words.text("From %@", "@linea-supply-example.com") && chip["on"] as? Bool == true, "\(chip["label"] ?? "")")
    #expect(s.text.isEmpty && s.domainOffers.isEmpty, "the typed word became the pill: '\(s.text)'")
}

@MainActor @Test func theDomainPillKeepsOnlyMailFromThatDomainAndTakingItOffWidensTheList() throws {
    let s = try typedWords("linea")
    defer { _ = s.close() }
    #expect(s.chooseDomain(0))
    #expect(!s.results.isEmpty && s.results.allSatisfy { $0.row.senderAddress.lowercased().hasSuffix("@linea-supply-example.com") }, "only that domain: \(s.results.map(\.row.senderAddress))")
    s.app.typed = "supply"
    s.settle()
    let narrow = s.results.count
    #expect(narrow > 0 && s.results.allSatisfy { $0.row.senderAddress.lowercased().hasSuffix("@linea-supply-example.com") })
    #expect(s.clearFilter("domain"))
    #expect(s.filters.domain == nil && s.results.count >= narrow, "the pill is gone and the list is as wide as before or wider: \(s.results.count) of \(narrow)")
    s.app.typed = ""
    s.settle()
    #expect(s.phase == .idle, "no words and no pill: the search is at rest")
}

@MainActor @Test func theDomainPillTravelsInTheRuleOfASavedSmartFolder() throws {
    let s = try typedWords("linea")
    defer { _ = s.close() }
    #expect(s.chooseDomain(0))
    #expect(s.ruleWords == "from:@linea-supply-example.com", "the rule carries the pill like the Mac's colon form: \(s.ruleWords)")
    #expect(s.startSaving())
    s.name = "Linea domain"
    #expect(s.confirmName())
    #expect(s.savedAs == "Linea domain", "\(s.fieldSays)")
    #expect(try #require(s.app.core.smartFolders().first { $0.name == "Linea domain" }).name == "Linea domain")
}

// ---- opening a result, the skeleton hold
@MainActor @Test func openingAResultNamesTheMailThatHoldsThePassage() throws {
    let (s, made) = try rig()
    defer { _ = s.close() }
    var m = made
    let other = m[3].row.messageId
    m[0].hitMessageId = other
    s.applyParts(m)
    #expect(s.openResult(0))
    #expect(s.openedMessageId == m[0].row.messageId, "the thread opens on the result's row")
    #expect(s.openedHitMessageId == other, "and the state names the mail that holds the passage: \(String(describing: s.openedHitMessageId))")
    #expect(try rows("iphone-search.state").first?["opened_hit"] as? String == other)
    m[1].hitMessageId = ""
    s.applyParts(m)
    #expect(s.openResult(1) && s.openedHitMessageId == m[1].row.messageId, "no earlier mail named: the result's own mail")
}

@MainActor @Test func theSkeletonHoldKeepsTheFirstAnswerOffTheListUntilItIsReleased() throws {
    let m = try appModel()
    let s = SearchPhoneModel.shared
    _ = s.close(); _ = s.open()
    #expect(s.hold())
    m.typed = "quote"
    s.settle()
    #expect(s.phase == .searching && s.results.isEmpty, "held: the skeleton stands, no row is listed: \(s.phase)")
    #expect(try rows("iphone-search.skeleton").first?["loading"] as? Bool == true)
    #expect(s.release())
    #expect(s.phase == .results && !s.results.isEmpty, "released: the answer is listed on the same turn: \(s.phase)")
    #expect(try rows("iphone-search.skeleton").first?["loading"] as? Bool == false)
    _ = s.close()
}
