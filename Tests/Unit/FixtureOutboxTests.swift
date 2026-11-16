import Foundation
import Testing
@testable import WhooshUI

// The test engine's outbox settles a scheduled send at its time, as the real outbox does (Primary sends at the time).
// Old behaviour: settleOutboxLocked settled only `.undoable` entries, so a scheduled entry stayed `scheduled` for ever (J3.8).
// MUTANTS:
//   aScheduledEntryBeforeItsTimeStays        : the settle sends a scheduled entry at once
//   aScheduledEntryAtItsTimeIsSentOnce       : the settle forgets the scheduled branch (the old behaviour)
//   aSecondSettleSendsNothingMore            : the settle leaves the entry scheduled after it sent

private let repoRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()

private func scheduled(afterSeconds s: Double) throws -> (FixtureCore, FixtureClock) {
    let clock = FixtureClock()
    let c = try FixtureCore(url: repoRoot.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: clock)
    let draft = try #require(try c.run(command: .saveDraft(fromAccountId: nil, to: ["laura@example.com"], cc: [], subject: "Later", body: "Hello", replyToThreadId: nil)).createdId)
    _ = try c.run(command: .schedule(draftId: draft, atMs: clock.nowMs() + Int64(s * 1000)))
    return (c, clock)
}

@Test func aScheduledEntryBeforeItsTimeStays() throws {
    let (c, clock) = try scheduled(afterSeconds: 3600)
    clock.advance(seconds: 3000)
    #expect(c.state(path: "outbox_entry/0/state") == "\"scheduled\"")
}

@Test func aScheduledEntryAtItsTimeIsSentOnce() throws {
    let (c, clock) = try scheduled(afterSeconds: 3600)
    clock.advance(seconds: 3601)
    #expect(c.state(path: "outbox_entry/0/state") == "\"sent\"")
    #expect(c.state(path: "outbox_entry/count") == "1")
}

@Test func aSecondSettleSendsNothingMore() throws {
    let (c, clock) = try scheduled(afterSeconds: 3600)
    clock.advance(seconds: 3601)
    let first = c.state(path: "counts/sent")
    clock.advance(seconds: 100)
    #expect(c.state(path: "outbox_entry/0/state") == "\"sent\"")
    #expect(c.state(path: "counts/sent") == first, "a second settle sent again")
}

// `fixture offline` and `fixture server-refuse`: a send that is due waits for the network, or fails with the reason.
@Test func aDueSendWaitsWhileOfflineAndGoesWhenTheNetworkIsBack() throws {
    let (c, clock) = try scheduled(afterSeconds: 60)
    c.fixtureOutbox(offline: true)
    clock.advance(seconds: 61)
    #expect(c.state(path: "outbox_entry/0/state") == "\"waiting\"")
    c.fixtureOutbox(offline: false)
    #expect(c.state(path: "outbox_entry/0/state") == "\"sent\"")
}

@Test func aDueSendTheServerRefusesFailsWithTheReasonAndRetryGoesOnceItIsOff() throws {
    let (c, clock) = try scheduled(afterSeconds: 60)
    c.fixtureServerRefuse("mailbox full")
    clock.advance(seconds: 61)
    #expect(c.state(path: "outbox_entry/0/state") == "\"failed\"")
    c.fixtureServerRefuse(nil)
    #expect(c.state(path: "outbox_entry/0/state") == "\"failed\"", "off does not resend by itself")
    let id = c.entries[0].id
    _ = try c.run(command: .sendNow(outboxId: id))
    #expect(c.state(path: "outbox_entry/0/state") == "\"sent\"")
}
