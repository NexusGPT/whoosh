import Foundation
import Testing
@testable import WhooshUI

// Send later on the fixture core (Sources/CoreBridge/FixtureCore.swift, scheduleLocked), read the way the app reads it:
// the Outbox mailbox listing (InboxQuery.mailbox = .outbox) and the outbox read (WhooshCore.outbox) that carries the time.
// The engine's counterpart is core/crates/engine/tests/engine_act.rs (the Schedule command, near line 195).
// Not tested here, on purpose: the fixture core never sends a scheduled entry when its time comes (settleOutboxLocked moves undoable entries only),
// and a cancelled schedule does not put the draft back into Drafts (the engine's own test only asserts the outbox read is empty).

private func core() throws -> (FixtureCore, FixtureClock) {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let clock = FixtureClock()
    return (try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: clock), clock)
}
private func listed(_ c: WhooshCore, _ mailbox: Mailbox) -> [InboxRow] {
    c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil, mailbox: mailbox))
}
private func newDraft(_ c: WhooshCore) throws -> String {
    try #require(try c.run(command: .saveDraft(fromAccountId: nil, to: ["w@example.test"], cc: [], subject: "Plan", body: "later", replyToThreadId: nil)).createdId)
}
private func scheduledAt(_ e: OutboxEntry) -> Int64? { if case .scheduled(let at) = e.state { return at } else { return nil } }

@Test func aDraftScheduledForTheFutureIsListedWaitingWithItsTime() throws {
    let (c, clock) = try core()
    let draftId = try newDraft(c)
    let at = clock.nowMs() + 3_600_000
    let out = try c.run(command: .schedule(draftId: draftId, atMs: at))
    let entryId = try #require(out.createdId)
    #expect(listed(c, .outbox).map(\.messageId) == [draftId], "the Outbox listing shows the scheduled message by its draft")
    let entry = try #require(c.outbox().first { $0.id == entryId })
    #expect(scheduledAt(entry) == at, "the entry keeps the time it was scheduled for")
    #expect(listed(c, .drafts).isEmpty, "a scheduled draft leaves the drafts")
}

@Test func aScheduleForAPastTimeIsRefusedWithTheEnginesWords() throws {
    let (c, clock) = try core()
    let draftId = try newDraft(c)
    do { _ = try c.run(command: .schedule(draftId: draftId, atMs: clock.nowMs() - 1000)); Issue.record("accepted a past time") }
    catch CoreError.Invalid(let field, let words) { #expect(field == "at_ms"); #expect(words == "pick a time in the future") }
    #expect(c.outbox().isEmpty, "a refused schedule queues nothing")
    #expect(listed(c, .drafts).map(\.messageId) == [draftId], "and the draft stays a draft")
}

@Test func cancellingAScheduledSendTakesItOutOfTheListing() throws {
    let (c, clock) = try core()
    let draftId = try newDraft(c)
    let entryId = try #require(try c.run(command: .schedule(draftId: draftId, atMs: clock.nowMs() + 3_600_000)).createdId)
    _ = try c.run(command: .cancelSend(outboxId: entryId))
    #expect(listed(c, .outbox).isEmpty)
    #expect(c.outbox().isEmpty, "a cancelled entry is not open")
    #expect(c.state(path: "outbox_entry/0/state") == "\"cancelled\"")
}

@Test func nothingIsSentBeforeTheScheduledTime() throws {
    let (c, clock) = try core()
    let draftId = try newDraft(c)
    _ = try c.run(command: .schedule(draftId: draftId, atMs: clock.nowMs() + 3_600_000))
    clock.advance(seconds: Double(WhooshConstants.UNDO_SEND_DEFAULT_S) + 60)
    #expect(c.state(path: "outbox_entry/0/state") == "\"scheduled\"", "the undo window passing does not send a scheduled message")
    #expect(listed(c, .sent).isEmpty)
    #expect(listed(c, .outbox).map(\.messageId) == [draftId])
}
