import Foundation
import Testing
@testable import WhooshUI

// Search (Mac) faults found by the sorting of build main79's failed checks (lane app-search-faults).

private func result(_ id: String, thread: String, subject: String = "Quote") -> SearchResult {
    SearchResult(row: InboxRow(messageId: id, threadId: thread, accountId: "acc-01", senderName: "Linea Supply", subject: subject, preview: "", receivedAtMs: 0,
                               unread: false, pinned: false, labelNames: [], attachmentCount: 0, threadCount: 1),
                 score: 1, passage: "", highlights: [], marks: [], subjectMarks: [], attachmentName: nil, attachmentMarks: [], matchedIn: "subject")
}

@MainActor @Test func aMessageThatIsNotThereAnyMoreCountsAsGone() {
    #expect(SearchMacModel.isGone(nil))
}

@MainActor @Test func aResultWhoseMessageHasGoneLeavesTheListAndTheNoticeSaysSo() {
    let m = SearchMacModel()
    m.launched = true
    m.outcome.results = [result("m1", thread: "t1"), result("m2", thread: "t2"), result("m3", thread: "t3")]
    m.selected = 1
    #expect(m.notice.isEmpty)
    m.forget(messageId: "m2", at: 1)
    #expect(m.results.map(\.row.messageId) == ["m1", "m3"])
    #expect(m.notice == Words.text("no longer in your mail"))
    #expect(!m.notice.isEmpty)
    #expect(m.selected == 1)                          // the next row takes the place
}

@MainActor @Test func theLastResultGoingLeavesTheSelectionOnTheNewLastRow() {
    let m = SearchMacModel()
    m.launched = true
    m.outcome.results = [result("m1", thread: "t1"), result("m2", thread: "t2")]
    m.forget(messageId: "m2", at: 1)
    #expect(m.selected == 0)
}

@MainActor @Test func theNoticeIsServedAsOneRowAndAbsentWhenThereIsNone() throws {
    let rows = try #require(SearchMacRows.all["mac-search.notice"])
    let m = SearchMacModel.shared
    m.notice = ""
    #expect(try rows(AppModel.shared).isEmpty)
    m.notice = Words.text("no longer in your mail")
    let served = try rows(AppModel.shared)
    #expect(served.count == 1)
    #expect(served.first?["text"] as? String == Words.text("no longer in your mail"))
    m.notice = ""
}

// MARK: suggestions

@MainActor @Test func aTypedWordDoesNotSuggestANameThatOnlySharesItsStart() {
    #expect(SearchSuggest.match("Linea Supply", typed: ["Supplier"]) == nil)
    #expect(SearchSuggest.match("Linea Supply", typed: ["suppl"])?.range == 6..<11)
}

@MainActor @Test func aTypedWordSuggestsTheNameWhoseWordStartsWithItWhateverTheAccentsAndCase() {
    #expect(SearchSuggest.match("Linea Supply", typed: ["supp"])?.range == 6..<10)
    #expect(SearchSuggest.match("Élodie Brun", typed: ["elo"])?.range == 0..<3)
    #expect(SearchSuggest.match("Linea Supply", typed: ["su"]) == nil)       // under three letters nothing matches
}

@MainActor @Test func everySuggestedNameContainsTheWordThatWasTyped() {
    // the engine answers the people by their first letters (SearchSuggest.swift PeopleRead): "Supplier" finds nobody in "Linea Supply", so it sends nobody
    let source = SearchSuggest.Source(people: [], labels: ["Suppliers"], folders: [("Supplier quotes", "quote")], recents: [])
    let rows = SearchSuggest.build(draft: "Supplier", chips: [], source: source, recentMax: 3)
    #expect(!rows.contains { $0.kind == .from || $0.kind == .to })
    #expect(rows.contains { $0.kind == .folder && $0.title == "Supplier quotes" })
    for r in rows where [.label, .folder].contains(r.kind) {
        #expect(r.title.folding(options: [.caseInsensitive, .diacriticInsensitive], locale: nil).contains("supplier"))
    }
}

// MARK: truncation facts

@MainActor @Test func aTextThatNeedsMoreWidthThanItsBoxIsCutAndOneThatFitsIsNot() {
    #expect(SearchMacRows.isCut(need: 240, box: 180))
    #expect(!SearchMacRows.isCut(need: 180, box: 180))
    #expect(!SearchMacRows.isCut(need: 180.4, box: 180))        // the pixel grid's rounding is not a cut
}

@MainActor @Test func aResultsRowServesWhatWasJudgedAndNullForWhatWasNot() {
    let cut = ["mac-search.results.0.subject": true, "mac-search.results.0.attachment": false]
    #expect(SearchMacRows.cutValue(cut, 0, "subject") as? Bool == true)
    #expect(SearchMacRows.cutValue(cut, 0, "attachment") as? Bool == false)
    #expect(SearchMacRows.cutValue(cut, 1, "subject") is NSNull)                         // not judged: null, never a made-up false
    #expect(SearchMacRows.cutValue(cut, 1, "attachment", shown: false) as? Bool == false)   // no attachment on the row: nothing to cut
}

@MainActor @Test func everyResultsRowCarriesTheCutFactsAndTheFade() throws {
    let m = SearchMacModel.shared
    m.launched = true
    m.outcome.results = [result("m1", thread: "t1")]
    defer { m.launched = false; m.outcome = SearchEngine.Outcome() }
    let rows = try #require(SearchMacRows.all["mac-search.results"])
    let row = try #require(try rows(AppModel.shared).first)
    #expect(row["subject_truncated"] != nil)
    #expect(row["attachment_truncated"] as? Bool == false)
    #expect(row["fade_ms"] as? Int == Int(WhooshConstants.SEARCH_SKELETON_FADE_MS))
}
