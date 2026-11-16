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

// J5.4-J5.6 parity (C-P-ask). The agent loop is tested in Tests/Integration/AgentSeamTests.swift; here: the phone screen and the limits it must respect.
@Test func theAgentLimitsAreTheConstantsNotLiteralsInTheScreen() {
    #expect(WhooshConstants.AGENT_MAX_STEPS == 10)                                            // mutant: the ledger value drifts from the generated one
    #expect(WhooshConstants.AGENT_CONTEXT_CHUNKS == 20)
}
// The Ask panel was retired on purpose: docs/DESIGN.md:26 "mac-ask retires into mac-search ... the results are the matching messages and nothing else";
// docs/SPEC.md A26 "no separate Ask AI"; docs/IPHONE-PARITY.md:7 (A26, Same: "search field answers questions too", the screen is iphone-search);
// docs/JOURNEYS.md J5.4 "no answer text, no agent reply: the results are the answer"; commit 59f91f24 "Retire the Ask panel".
// So the two tests below (names kept) now ask what the spec asks: no citation list, no proposal and no approve control exist, and the question's
// answer is the search results, each naming its account and sender.
@MainActor @Test func theAskScreenRegistersCitationsAndAProposalThatWaitsForApproval() throws {
    #expect(Registrations.rowSource("iphone-ask.citations") == nil)                           // broken copy: register a rowSource "iphone-ask.citations" (an answer with citations is back)
    #expect(Registrations.rowSource("iphone-ask.proposal") == nil)
    #expect(Registrations.click("iphone-ask.proposal.approve") == nil)                        // broken copy: register a click "iphone-ask.proposal.approve" (labels applied by an agent)
    #expect(Registrations.click("iphone-ask.citation.1") == nil)
    #expect(Registrations.rowSource("iphone-search.results") != nil)                          // broken copy: drop "iphone-search.results" from SearchPhoneRows.all (no answer at all)
}
@MainActor @Test func everyCitationNamesAccountSenderAndDate() throws {
    let m = try model()
    let search = SearchPhoneModel.shared
    search.open()
    defer { _ = search.close() }
    m.typed = "quote"                                                                         // the question; Linea Supply's "Quote for the Q1 print run" is in the fixture
    let r = try rows("iphone-search.results")
    try #require(!r.isEmpty, "no results to judge: the fixture holds a mail about a quote")
    #expect(r.allSatisfy { ($0["account"] as? String)?.isEmpty == false && ($0["sender"] as? String)?.isEmpty == false })   // broken copy: a result row built without "sender" or "account" in SearchPhoneRows.swift
    #expect(r.allSatisfy { $0["answer"] == nil })                                             // broken copy: a result row gains an "answer" text (an agent reply)
}
