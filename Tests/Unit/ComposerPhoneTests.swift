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
// FixtureCore.state(path:) answers strings JSON-encoded (\"undoable\"), booleans and counts bare.
private func primary(_ c: FixtureCore) -> Int { Int(c.counts().tabs.first { $0.tabId == "primary" }?.unread ?? 0) }
private func primaryRows(_ c: FixtureCore) -> [InboxRow] { c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)) }

@Test func undoSendDefaultSitsInsideItsOwnBounds() {
    #expect(WhooshConstants.UNDO_SEND_MIN <= WhooshConstants.UNDO_SEND_DEFAULT_S)             // mutant: default below the minimum
    #expect(WhooshConstants.UNDO_SEND_DEFAULT_S <= WhooshConstants.UNDO_SEND_MAX)             // mutant: default above the maximum
    #expect(WhooshConstants.UNDO_SEND_DEFAULT_S == 10)
}
@Test func aReplyIsUndoableForExactlyTheDefaultWindowThenSent() throws {
    let (c, clock) = try fixtureCore()
    let thread = try #require(primaryRows(c).first { $0.accountId != c.failingAccountId }).messageId
    let out = try c.run(command: .quickSend(threadId: thread, body: "thanks i will send the file tomorrow"))
    #expect(out.createdId != nil)
    #expect(c.state(path: "outbox_entry/count") == "1")
    #expect(c.state(path: "outbox_entry/0/state") == "\"undoable\"")                               // mutant: sent at once, no undo window
    clock.advance(seconds: Double(WhooshConstants.UNDO_SEND_DEFAULT_S) - 1)
    #expect(c.state(path: "outbox_entry/0/state") == "\"undoable\"")                               // mutant: window shorter than the default
    clock.advance(seconds: 2)
    #expect(c.state(path: "outbox_entry/0/state") == "\"sent\"")                                   // mutant: window never closes
}
@Test func cancelInsideTheWindowStopsTheSendAndItStaysStopped() throws {
    let (c, clock) = try fixtureCore()
    let thread = try #require(primaryRows(c).first { $0.accountId != c.failingAccountId }).messageId
    let id = try #require(try c.run(command: .quickSend(threadId: thread, body: "x")).createdId)
    _ = try c.run(command: .cancelSend(outboxId: id))
    clock.advance(seconds: Double(WhooshConstants.UNDO_SEND_DEFAULT_S) + 5)
    #expect(c.state(path: "outbox_entry/0/state") == "\"cancelled\"")                              // mutant: cancel does not hold past the window
}
@Test func cancellingAnUnknownEntryIsNotFoundNotASilentSuccess() throws {
    let (c, _) = try fixtureCore()
    do { _ = try c.run(command: .cancelSend(outboxId: "outbox-nope")); Issue.record("an unknown entry was cancelled") }
    catch let e as CoreError { guard case .NotFound = e else { Issue.record("expected NotFound, got \(e)"); return } }   // mutant: cancel of nothing succeeds
}
@Test func aReplyFromTheFailingAccountIsRefusedInWords() throws {
    let (c, _) = try fixtureCore()
    let thread = try #require(primaryRows(c).first { $0.accountId == c.failingAccountId }).messageId
    do { _ = try c.run(command: .quickSend(threadId: thread, body: "x")); Issue.record("a send from the failing account was queued") }
    catch let e as CoreError { guard case .Action(let f) = e else { Issue.record("expected ActionFailed, got \(e)"); return }
        #expect(f.accountName == "Provanta RFP") }                                             // mutant: the send is queued and fails later, silently
    #expect(c.state(path: "outbox_entry/count") == "0")
}
@MainActor @Test func theComposerRegistersTheMicrophoneUndoCancelAndTheFromAddress() throws {
    #expect(Registrations.click("iphone-composer.dictate") != nil)                            // mutant: no microphone control
    #expect(Registrations.click("iphone-composer.undo.cancel") != nil)                        // mutant: no cancel on the undo bar
    // The From row answers only while a composer is open (ComposerPhoneProbe.from): open a new message here and close it again, so the
    // test does not depend on a draft another test of the same process left open. A new message is sent from the account the list is
    // filtered to, else the first account (ComposerModel.reset): the test sets its own filter instead of reading what an earlier test left.
    let m = try model()
    let northlight = try #require(m.core.accounts().first { $0.address == "assem@northlight-example.de" }, "the fixture has the northlight account")
    m.accountFilter = northlight.id
    defer { ComposerPhoneState.shared.close(m); m.accountFilter = nil }
    try #require(ComposerPhoneState.shared.open(reply: nil, m), "a new message opens on the fixture")
    let from = try rows("iphone-composer.from")
    #expect(from.first?["text"] as? String == "assem@northlight-example.de")                  // mutant: replies go out from another account
}
