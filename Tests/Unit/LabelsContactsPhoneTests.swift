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

// J2.6-J2.7 parity (C-P-labels).
private func flat(_ nodes: [[String: Any]], _ depth: Int = 0) -> [String] {
    nodes.flatMap { n in ["\(depth):\(n["name"] as! String)"] + flat((n["children"] as? [[String: Any]]) ?? [], depth + 1) }
}
@Test func theFixtureLabelTreeNestsThreeDeepSoTheScreenHasDepthToShow() throws {
    let tree = flat(try list("labels"))
    #expect(tree.contains("2:Nordlicht"))                                                      // mutant: the fixture is flattened
    #expect(tree.first == "0:Work")
}
@MainActor @Test func thePhoneShowsTheSameLabelTreeWithTheSameDepths() throws {
    // The core files its four built-in labels first, as top-level rows (RootDelta9Tests.swift:28); the file's tree follows them.
    // The phone counts depth the way LabelTree does (1 = top level, LabelTree.swift:23); the file's `flat` starts at 0, so each wanted depth is one more.
    let builtIn = ["1:Needs Reply", "1:Updates", "1:Promotions", "1:Social"]
    let want = builtIn + flat(try list("labels")).map { line -> String in
        let parts = line.split(separator: ":", maxSplits: 1).map(String.init)
        return "\(Int(parts[0])! + 1):\(parts[1])"
    }
    let r = try rows("iphone-labels-contacts.labels")
    #expect(r.map { "\($0["depth"] as! Int):\($0["name"] as! String)" } == want)            // mutant: labels flattened on the phone
}
// The phone lists the smart folders read only: docs/lanes/labelscontactsphone.md "Not built": "Editing a label's rules, creating, renaming, nesting, moving: stays on the Mac"
// and "J2.5 and J2.6's folder editor stay on the Mac" (the page says "Rules are edited on the Mac."), so there is no phone rule editor to draw a count "before saving".
// What the phone keeps of C-J2.6 (docs/CAPABILITY-CONTRACT.md:38: "a smart folder is a saved query; its count equals its result count"; "0 matches"): each folder's rule in words
// and its count, a folder that matches nothing reading 0 (never a missing row).
@MainActor @Test func theSmartFoldersShowTheirRuleInWordsAndAZeroMatchCountBeforeSaving() throws {
    let folders = try list("smart_folders")
    let r = try rows("iphone-labels-contacts.smart-folders")
    #expect(r.map { $0["name"] as? String } == folders.map { $0["name"] as? String })         // broken copy: Probe.smartFolders drops its last folder (.dropLast())
    #expect(r.allSatisfy { ($0["rule"] as? String)?.isEmpty == false })                       // broken copy: the row's "rule" is "" (no rule in words)
    let counts = try rows("iphone-labels-contacts.match-count")
    #expect(counts.count == folders.count)                                                    // broken copy: Probe.matchCount returns [] (no count served)
    #expect(counts.allSatisfy { ($0["count"] as? Int).map { $0 >= 0 } == true })              // broken copy: "count" served as NSNull for a folder that matches nothing
}
@MainActor @Test func theContactCardListsEveryMessageFromTheSenderWithItsAccount() throws {
    let msgs = try list("messages")
    let sender = msgs[0]["sender"] as! String
    let address = msgs[0]["from"] as! String
    let mine = msgs.filter { ($0["sender"] as? String) == sender }
    // The card's rows exist only while a card is open: open the sender's card through the door the thread lane calls (docs/lanes/labelscontactsphone.md, hand-over 3).
    let app = try model()
    let state = LabelsContactsPhoneState.shared
    try #require(state.openContact(address: address, in: app), "the core has no contact card for \(address)")
    defer { _ = state.back(app) }
    let r = try rows("iphone-labels-contacts.contact.messages")
    #expect(r.map { $0["subject"] as? String } == mine.map { $0["subject"] as? String })      // mutant: another sender's mail
    #expect(r.map { $0["account"] as? String } == mine.map { $0["account"] as? String })      // mutant: no account on a card row
}

// mob-labelscontactsphone (proof file checks/proofs/g2/fa8497a8.json): the page's card wording is the Mac's (checks/labelscontactsphone-check.py L3).
// One test per wording, each reading the real card rows of a label page the fixture core serves. The language is pinned to English: the phone writes the titles through Words.
// mutants (exact edits in docs/lanes/gap-phone-b.md): LabelPhoneRead.swift Words.text("Entry rules") -> Words.text("Entry rule"); Words.text("AI sorting") -> Words.text("AI sort"); Words.text("Exit filter") -> Words.text("Exit")
@MainActor @Test(arguments: [("entry", "Entry rules"), ("ai", "AI sorting"), ("exit", "Exit filter")])
func theLabelPageNamesItsCardsAsTheMacDoes(card: String, title: String) throws {
    try LanguagePin.with("en") {
        let app = try model()
        let labels = try rows("iphone-labels-contacts.labels")
        let id = try #require(labels.first { ($0["name"] as? String) == "Suppliers" }?["id"] as? String, "the fixture's Suppliers label is not listed")
        let state = LabelsContactsPhoneState.shared
        try #require(state.openLabel(id, in: app), "the core has no label \(id)")
        defer { _ = state.back(app) }
        let cards = try rows("iphone-labels-contacts.label.cards")
        let row = try #require(cards.first { ($0["card"] as? String) == card }, "no \(card) card on the label page")
        #expect(row["title"] as? String == title)
    }
}
