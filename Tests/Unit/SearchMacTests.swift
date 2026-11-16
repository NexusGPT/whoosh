import Foundation
import Testing
@testable import WhooshUI

// Search (Mac), lane searchmac. Two kinds of test:
//  - the corpus tests prove fixtures/tests-j2.json can make the J2 checks fail: each decoy is really a decoy (they pass today);
//  - the registration tests read what SearchMacFeature registers with the socket (they fail today: the feature is a stub).
// The query parser has no Swift symbol yet. HANDBACK to the searchmac lane: expose a pure parser, for example
// `SearchQuery.parse(_ text: String) -> (chips: [Chip], residual: String, quoted: [String])`; fixtures/tests-j2.json
// `filter_cases` and `adversarial_queries` then drive it directly with one assertion per case.

private struct Message: Decodable {
    let id: String, thread: String, account: String, from: String, date: String, subject: String, body: String
    let attachments: [String]
}
private struct Corpus: Decodable {
    let messages: [Message]
    let expect: Expect
    struct Expect: Decodable {
        let exact_query: String
        let exact_matching_messages: [String]
        let exact_must_not_match: [String]
        let before_june_threads: [String]?
    }
    let before_june: String
}

private func corpus() throws -> Corpus {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    return try JSONDecoder().decode(Corpus.self, from: Data(contentsOf: root.appendingPathComponent("fixtures/tests-j2.json")))
}

/// The exact-figure rule of JOURNEYS J2.3: the quoted figure matches as a whole token, never inside a longer figure.
private func holdsExactly(_ figure: String, _ m: Message) -> Bool {
    let pattern = "(?<![\\w.,])" + NSRegularExpression.escapedPattern(for: figure) + "(?![\\w])"
    let text = m.subject + " " + m.body
    return text.range(of: pattern, options: .regularExpression) != nil
}

@Test func theExactFigureMatchesRecordOneAndNothingElse() throws {
    let c = try corpus()
    let figure = c.expect.exact_query.trimmingCharacters(in: CharacterSet(charactersIn: "\""))
    let hits = c.messages.filter { holdsExactly(figure, $0) }.map(\.id)
    #expect(hits == c.expect.exact_matching_messages)
}

@Test func theNearFiguresAreRealDecoys() throws {
    let c = try corpus()
    let figure = c.expect.exact_query.trimmingCharacters(in: CharacterSet(charactersIn: "\""))
    for id in c.expect.exact_must_not_match {
        let m = try #require(c.messages.first { $0.id == id })
        #expect(!holdsExactly(figure, m), "\(id) must not hold the exact figure")
    }
    // positive control for the line above: a plain substring search DOES find the 110.84 EUR decoy, so a substring bug would show
    let host = try #require(c.messages.first { $0.id == "msg-s07" })
    #expect((host.subject + host.body).contains(figure))
}

@Test func theFilterDecoysCoverEachFilterAlone() throws {
    let c = try corpus()
    let linea = c.messages.filter { $0.from.hasPrefix("orders@linea-supply") }
    #expect(linea.contains { $0.date.prefix(10) >= c.before_june.prefix(10) && !$0.attachments.isEmpty }, "needs: right sender, after June, with attachment")
    #expect(linea.contains { $0.date.prefix(10) < c.before_june.prefix(10) && $0.attachments.isEmpty }, "needs: right sender, before June, no attachment")
    #expect(linea.filter { $0.date.prefix(10) < c.before_june.prefix(10) && !$0.attachments.isEmpty }.map(\.id) == ["msg-01"])
}

@Test func recordOnesThreadHasSeveralMatchingMessages() throws {
    let c = try corpus()
    #expect(c.messages.filter { $0.thread == "thr-01" && ($0.subject + $0.body).lowercased().contains("quote") }.count >= 2)
}

// MARK: registration with the socket (docs/CONTROL-SOCKET.md): what the screen must expose for J2 to drive it

@MainActor @Test func searchScreenIsRegistered() {
    #expect(SearchMacFeature.surfaces == ["mac-search"])
}

@MainActor @Test func searchKeyIsBound() {
    // swift-tests lane: the lane's own keyBindings (⌘A, Tab, Escape: SearchMacFeature.swift:37-47) no longer hold "/"; the key map owns it as the second way into the `search` command
    // (Sources/Features/Shortcuts/KeyPreset.swift:129 and :210, `alsoKeys: [KeyBinding("/", "search")]`), so the chain the app walks for "/" is asked: some feature binds it.
    #expect(Registrations.key("/") != nil, "the search key (/ in the Superhuman preset) opens the search screen")
}

@MainActor @Test func searchRegionsServeRows() {
    let rows = SearchMacFeature.rowSources()
    for ax in ["mac-search.results", "mac-search.chips", "mac-search.bar", "mac-search.recent", "mac-search.sidebar.smart-folders", "mac-search.save-as-smart-folder.field"] {
        #expect(rows[ax] != nil, "no row source for \(ax)")
    }
}

@MainActor @Test func searchClicksAreRegistered() {
    let clicks = SearchMacFeature.clickTargets()
    for ax in ["mac-search.bar", "mac-search.save-as-smart-folder"] { #expect(clicks[ax] != nil, "no click target for \(ax)") }
}

// MARK: round 3, the one search (S1 Quiet rows): the real parser and the real recents, never a copy of their rules

private struct FilterCase: Decodable {
    struct Chip: Decodable { let kind: String; let value: String? }
    let text: String
    let chips: [Chip]
    let residual: String
    let bar_says: String?
}
private struct FilterCorpus: Decodable {
    let filter_cases: [FilterCase]
    let adversarial_queries: [Adversarial]
    struct Adversarial: Decodable { let name: String; let text: String?; let repeatWord: String?; let times: Int?
        enum CodingKeys: String, CodingKey { case name, text, repeatWord = "repeat", times }
    }
}
private func filterCorpus() throws -> FilterCorpus {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    return try JSONDecoder().decode(FilterCorpus.self, from: Data(contentsOf: root.appendingPathComponent("fixtures/tests-j2.json")))
}

@Test func everyFilterCaseOfTheCorpusParsesToItsChipsAndResidual() throws {
    for c in try filterCorpus().filter_cases {
        let q = SearchQuery(c.text)
        #expect(q.chips.map(\.kind.rawValue).sorted() == c.chips.map(\.kind).sorted(), "chips of \(c.text.debugDescription)")
        #expect(q.words.joined(separator: " ") == c.residual, "residual of \(c.text.debugDescription)")
        for want in c.chips { if let v = want.value { #expect(q.chips.first { $0.kind.rawValue == want.kind }?.value == v, "value of \(want.kind) in \(c.text.debugDescription)") } }
        #expect(q.notUnderstood.isEmpty == (c.bar_says == nil), "a not-understood filter must be named exactly when the case says the bar speaks: \(c.text.debugDescription)")
    }
}

@Test func theThreeContainsWordsOfTheDropdownBecomeTheirOwnChips() {
    // S1: the dropdown's "Sender contains", "Subject contains", "Attachment name contains" rows add a chip on Tab; typed in words they parse the same way.
    // swift-tests lane: the three dropdown rows add the chips `from`, `subject` and `attname` and read back from the colon form `from:"Linea"`, `subject:"quote"`, `attachment:"quote"`
    // (Sources/Features/SearchMac/SearchQuery.swift:6-30, `SearchChip.Kind` and `canonical`; SearchMacActs.swift:117-130, Tab). The old cases typed "sender contains Linea" and named kinds
    // `sender-contains`, `subject-contains`, `attachment-name-contains`, which the lane never had (tests-j2 round 3 wrote them against an assumed wire; c09db58a).
    let cases: [(String, String)] = [("from:Linea", "from"), ("subject:quote", "subject"),
                                     ("attachment:quote", "attname"), ("SUBJECT:quote", "subject")]
    for (text, kind) in cases {
        let q = SearchQuery(text)
        #expect(q.chips.map(\.kind.rawValue) == [kind], "\(text)")
        #expect(q.words.isEmpty, "\(text)")
    }
    // the bare words stay plain words (positive control: the same words WITH a value are chips above)
    #expect(SearchQuery("subject contains").chips.isEmpty)
    #expect(SearchQuery("sender").words == ["sender"])
}

@Test func adversarialTextNeverThrowsAndNeverInventsAChip() throws {
    for a in try filterCorpus().adversarial_queries {
        let text = a.text ?? String(repeating: (a.repeatWord ?? ""), count: a.times ?? 1)
        let q = SearchQuery(text)
        switch a.name {
        case "empty", "spaces only": #expect(q.isEmpty)
        case "empty quotes": #expect(q.phrase == nil && q.words.isEmpty)
        case "unterminated quote": #expect(q.phrase == nil && q.words == ["0.84", "EUR"])
        case "very long": #expect(q.words.count == 4000 && q.chips.isEmpty)
        case "filter bait only": #expect(q.chips.map(\.kind.rawValue) == ["attachment"] && q.words == ["from", "before"] && q.notUnderstood == ["before has"])
        case "bell character inside a word": #expect(q.words == ["quo", "te"])   // a non-space control character is read as a gap, never kept inside a word
        case "control characters": #expect(q.words == ["tab", "here", "newline"])
        default: #expect(q.chips.isEmpty, "\(a.name)")
        }
    }
}

@MainActor @Test func recentSearchesKeepTheNewestEightNewestFirstWithoutDuplicates() {
    let m = SearchMacModel()
    for i in 1...10 { m.remember("flood \(i)") }
    m.remember("flood 3")          // a repeat moves to the top, it does not add a row
    m.remember("   ")              // whitespace is not a search
    let want = ["flood 3"] + (4...10).reversed().map { "flood \($0)" }
    #expect(WhooshConstants.SEARCH_RECENT_MAX == 8)
    #expect(m.recents == Array(want.prefix(WhooshConstants.SEARCH_RECENT_MAX)))
}

@Test func theSearchMotionConstantsStayUnderTheMotionCeiling() {
    #expect(WhooshConstants.SEARCH_DROPDOWN_ENTER_MS <= WhooshConstants.MOTION_MAX_MS)
    #expect(WhooshConstants.SEARCH_SKELETON_FADE_MS <= WhooshConstants.MOTION_MAX_MS)
    #expect(WhooshConstants.SEARCH_HOVER_TINT_MS <= WhooshConstants.SEARCH_SKELETON_FADE_MS)
}

@MainActor @Test func theS1RegionsAreRegisteredWithTheSocket() {
    let rows = SearchMacFeature.rowSources()
    // The suggestions are served as `mac-search.dropdown` (SearchMacRows.swift:49; docs/lanes/seams.md:84) and the highlight is its rows' `highlighted` field;
    // the lane serves no `.suggestions.foot`, `.suggestion-highlight` or `.motion` region (docs/lanes/searchmac.md:81; the motion figures are the constants test above).
    for ax in ["mac-search.dropdown", "mac-search.skeleton", "mac-search.sections", "mac-search.header", "mac-search.footer"] {
        #expect(rows[ax] != nil, "no row source for \(ax)")
    }
    #expect(SearchMacFeature.clickTargets()["mac-search.bar.clear"] != nil)
}
