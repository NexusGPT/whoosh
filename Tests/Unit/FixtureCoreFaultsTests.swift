import Foundation
import Testing
@testable import WhooshUI

// Faults of the fixture core (the test copy's stand-in engine), lane app-fixture-faults. Written against the code read; never run by the lane.
// Each test names the old behaviour it fails on.

private let repoRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func fixtureCore() throws -> FixtureCore {
    try FixtureCore(url: repoRoot.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock())
}
/// The J2 corpus loaded into a fresh core; `accounts` moves named messages to another account first (a copy in the temp folder).
private func corpusCore(accounts moved: [String: String] = [:]) throws -> FixtureCore {
    let c = try fixtureCore()
    let source = repoRoot.appendingPathComponent("fixtures/tests-j2.json")
    guard !moved.isEmpty else { try c.loadCorpus(from: source); return c }
    var doc = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: source)) as? [String: Any])
    var rows = try #require(doc["messages"] as? [[String: Any]])
    for i in rows.indices { if let id = rows[i]["id"] as? String, let to = moved[id] { rows[i]["account"] = to } }
    doc["messages"] = rows
    let copy = FileManager.default.temporaryDirectory.appendingPathComponent("faults-corpus-\(UUID().uuidString).json")
    try JSONSerialization.data(withJSONObject: doc).write(to: copy)
    defer { try? FileManager.default.removeItem(at: copy) }
    try c.loadCorpus(from: copy)
    return c
}
private func jsonList(_ c: FixtureCore, _ path: String) throws -> [String] {
    let text = try #require(c.state(path: path), "no state at \(path)")
    return try JSONDecoder().decode([String].self, from: Data(text.utf8))
}

// MARK: fault 1, a refused send from a failing account

// Old behaviour: refuseFailing read `account[threadId]` (the map is keyed by message id), found nothing, and let the send through.
@Test func aQuickSendOnAThreadOfTheFailingAccountIsRefused() throws {
    let c = try corpusCore(accounts: ["msg-s08": "acc-18"])
    #expect(c.failingAccountId == "acc-18")
    var words: String?
    do { _ = try c.run(command: .quickSend(threadId: "thr-s08", body: "Thanks")) } catch CoreError.Action(let failure) { words = failure.reasonWords }
    #expect(words == "needs you to sign in again", "a thread id resolves to its messages' accounts")
}

@Test func aThreadIdResolvesToTheAccountsOfItsMessagesAndAMessageIdToItsOwn() throws {
    let c = try corpusCore(accounts: ["msg-01a": "acc-18"])
    #expect(c.accountsOf("thr-01").sorted() == ["acc-04", "acc-04", "acc-18"])
    #expect(c.accountsOf("msg-01a") == ["acc-18"])
    #expect(c.accountsOf("thr-s03") == ["acc-04"])
    #expect(c.accountsOf("thr-nowhere").isEmpty)
    #expect(throws: CoreError.self) { try c.refuseFailing(["thr-01"]) }
    #expect(throws: Never.self) { try c.refuseFailing(["thr-s03"]) }
}

// MARK: fault 2, a smart folder lists what the search screen lists

// Old behaviour: the folder ran the fixture search on the three terms of "0.84 EUR" one by one, so thr-s02 (10,080.00 EUR),
// thr-s05 (0.84 USD) and thr-s07 (110.84 EUR) were listed beside msg-01, the only message that holds the phrase.
@Test func aSmartFolderOnAQuotedPhraseListsOnlyTheMessageThatHoldsIt() throws {
    let c = try corpusCore()
    let id = try #require(try c.run(command: .saveSmartFolder(name: "Quoted figure", ruleWords: "\"0.84 EUR\"")).createdId)
    #expect(try jsonList(c, "smart_folder/\(id)/threads") == ["msg-01"])
    let rows = c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil, smartFolderId: id)).map(\.messageId)
    #expect(rows == ["msg-01"])
}

@Test func aQuotedPhraseNeedsTheWholeFigureAndAFigureThatIsNowhereListsNothing() throws {
    let c = try corpusCore()
    let near = try #require(try c.run(command: .saveSmartFolder(name: "Part of a figure", ruleWords: "\"84 EUR\"")).createdId)
    #expect(try jsonList(c, "smart_folder/\(near)/threads").isEmpty, "84 EUR is not inside 0.84 EUR or 110.84 EUR")
    let none = try #require(try c.run(command: .saveSmartFolder(name: "No such figure", ruleWords: "\"9.99 EUR\"")).createdId)
    #expect(try jsonList(c, "smart_folder/\(none)/threads").isEmpty)
}

// MARK: fault 3, one count for a rule

// Old behaviour: the editor's ruleMatchCount counted messages holding every word of three letters or more (for "0.84 EUR": the word "eur"
// alone, archived or not) while `smart_folder/<id>/count` counted the search's hits, so the two numbers differed for one rule.
@Test func theEditorCountAndTheSavedFolderCountAreOneNumberForTheSameRule() throws {
    let c = try corpusCore()
    for (i, rule) in ["\"0.84 EUR\"", "invoice", "forklift pallets"].enumerated() {
        let id = try #require(try c.run(command: .saveSmartFolder(name: "Rule \(i)", ruleWords: rule)).createdId)
        #expect(c.state(path: "smart_folder/\(id)/count") == "\(c.ruleMatchCount(ruleWords: rule))", "rule \(rule)")
    }
    #expect(c.ruleMatchCount(ruleWords: "\"0.84 EUR\"") == 1)
}

@Test func anArchiveMovesTheEditorCountAndTheFolderCountTogether() throws {
    let c = try corpusCore()
    let rule = "\"0.84 EUR\""
    let id = try #require(try c.run(command: .saveSmartFolder(name: "Quoted figure", ruleWords: rule)).createdId)
    #expect(c.ruleMatchCount(ruleWords: rule) == 1 && c.state(path: "smart_folder/\(id)/count") == "1")
    _ = try c.run(command: .archive(messageIds: ["msg-01"]))
    #expect(c.ruleMatchCount(ruleWords: rule) == 0 && c.state(path: "smart_folder/\(id)/count") == "0")
    #expect(c.state(path: "smart_folder/\(id)/unread") == "0")
}

@Test func aRuleWithNoWordLeftCountsNothingInBothPlaces() throws {
    let c = try corpusCore()
    #expect(c.ruleMatchCount(ruleWords: "  of a ") == 0)
    #expect(c.ruleMatchCount(ruleWords: "") == 0)
}

// MARK: fault 4, the labelling example is real

private func flowCounts(_ c: FixtureCore) throws -> [Int] {
    let text = try #require(c.state(path: "label/needs-reply/flow/counts"))
    let o = try #require(try JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Int])
    return ["caught_by_entry", "sorted_by_ai", "kept_by_exit", "sent_to_fallback"].map { o[$0] ?? -1 }
}

// Old behaviour: 1 / 31 / 23 / 8 was a fixed override; the filings behind it gave 1 / 4 / 3 / 1. SPEC A140: step 1 picks 37 (the 32 rows and 5 corpus mails that
// open "Hi Assem"), the AI accepts 32, step 3 keeps 24, 8 go to the fallback (the old order read 1 / 31 / 23 / 8: step 1 filed one mail itself).
@Test func theLabellingExampleDerivesItsFiguresFromRealFilings() throws {
    let c = try fixtureCore()
    #expect(try flowCounts(c) == [37, 32, 24, 8])
    let rows = try #require(try JSONSerialization.jsonObject(with: Data(contentsOf: repoRoot.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"))) as? [String: Any])["flow_example"] as? [[String: Any]]
    #expect(rows?.filter { $0["step"] as? String == "kept" }.count == 24 && rows?.filter { $0["step"] as? String == "rejected" }.count == 8 && rows?.filter { $0["step"] as? String == "entry" }.count == 0)
    #expect(Set(rows?.compactMap { $0["id"] as? String } ?? []).count == 32, "every example row has its own id")
}

// Old behaviour: after one reply the served override was dropped and the derived figures read kept 2, sent 2.
@Test func aReplyOnOneKeptMessageMovesItToPrimaryAndTheFiguresFollow() throws {
    let c = try fixtureCore()
    let r = try c.fixtureReply(messageId: "flow-kept-1")
    #expect(r.repliedByMe && !r.unanswered)
    #expect(try flowCounts(c) == [37, 32, 23, 9])
    #expect(c.state(path: "message/flow-kept-1/flow_rejection").map { $0.contains("primary") } == true)
}

// Old behaviour (main81): reading a smart folder ran its rule through the app's search book and wrote the book's last search, and the rule-match
// cache counted as state. The state digest after a reset then differed from the launch digest and whole groups of checks were not judged.
@Test func readingASmartFolderLeavesTheStateDigestAsItWas() throws {
    let c = try fixtureCore()
    let id = try #require(try c.run(command: .saveSmartFolder(name: "Words rule", ruleWords: "Linea invoice")).createdId)
    let before = c.stateDigest()
    _ = c.state(path: "smart_folder/\(id)/count")
    _ = c.state(path: "smart_folder/\(id)/threads")
    let after = c.stateDigest()
    let differing = before.parts.keys.filter { before.parts[$0] != after.parts[$0] }.sorted()
    #expect(differing.isEmpty, "a read changed these parts of the state: \(differing)")
    #expect(before.total == after.total)
}

