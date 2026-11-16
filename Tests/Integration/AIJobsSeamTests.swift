import Foundation
import Testing
@testable import WhooshUI

// Integration over the WhooshCore seam for J3.5-J3.8 (send, undo, schedule) and the AI jobs behind J3.2-J3.4, on FixtureCore.
// Written from docs/JOURNEYS.md, docs/CONSTANTS.md and docs/CAPABILITY-CONTRACT.md, never from a built screen. No server and no
// provider is reached: nothing leaves this Mac. The contract has no command for an AI draft, a rewrite, a dictation clean-up or a
// voice-profile refresh; the two ai_job tests below name the state paths those jobs must serve and fail until they do (hand-back).
// MUTANTS (the mutation of FixtureCore that must turn each red):
//   theWindowHoldsUntilTheLedgerSecond : the entry turns "sent" one second early
//   cancelInsideTheWindowNeverSends    : cancelSend leaves the entry undoable
//   cancelTwiceIsOneCancel             : the second cancel adds an entry or throws
//   cancelAfterTheWindowIsRefused      : cancelSend marks a message that already left as cancelled
//   scheduleKeepsItsTimeAndRefusesThePast / aiJobsServeTheirCount : the stub features (FixtureCore does not serve schedule or ai_job)

private func repo() -> URL { URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent() }
private func core(_ clock: FixtureClock = FixtureClock()) throws -> FixtureCore {
    try FixtureCore(url: repo().appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: clock)
}
private func firstThread(_ c: FixtureCore) throws -> String {
    try #require(c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)).first { $0.accountId != c.failingAccountId }?.threadId)
}
private func state(_ c: FixtureCore, _ path: String) -> String? { c.state(path: path) }
private let window = Double(WhooshConstants.UNDO_SEND_DEFAULT_S)

@Test func theWindowHoldsUntilTheLedgerSecond() throws {
    let clock = FixtureClock(), c = try core(clock)
    _ = try c.run(command: .quickSend(threadId: try firstThread(c), body: "Thanks."))
    #expect(state(c, "outbox_entry/0/state") == "\"undoable\"")
    clock.advance(seconds: window - 1)
    #expect(state(c, "outbox_entry/0/state") == "\"undoable\"", "one second before the end the undo is still offered")
    clock.advance(seconds: 1)
    #expect(state(c, "outbox_entry/0/state") == "\"sent\"", "at the end of the window the message has left")
}

@Test func cancelInsideTheWindowNeverSends() throws {
    let clock = FixtureClock(), c = try core(clock)
    let id = try #require(try c.run(command: .quickSend(threadId: try firstThread(c), body: "Thanks.")).createdId)
    _ = try c.run(command: .cancelSend(outboxId: id))
    #expect(state(c, "outbox_entry/0/state") == "\"cancelled\"")
    clock.advance(seconds: window + 1)
    #expect(state(c, "outbox_entry/0/state") == "\"cancelled\"", "the window passing does not send a cancelled message")
    // positive control: an uncancelled message does leave when the window passes
    _ = try c.run(command: .quickSend(threadId: try firstThread(c), body: "Again."))
    clock.advance(seconds: window + 1)
    #expect(state(c, "outbox_entry/1/state") == "\"sent\"")
}

@Test func cancelTwiceIsOneCancel() throws {
    let c = try core()
    let id = try #require(try c.run(command: .quickSend(threadId: try firstThread(c), body: "Thanks.")).createdId)
    _ = try c.run(command: .cancelSend(outboxId: id))
    _ = try? c.run(command: .cancelSend(outboxId: id))
    #expect(state(c, "outbox_entry/count") == "1")
    #expect(state(c, "outbox_entry/0/state") == "\"cancelled\"")
}

@Test func cancelAfterTheWindowIsRefused() throws {
    let clock = FixtureClock(), c = try core(clock)
    let id = try #require(try c.run(command: .quickSend(threadId: try firstThread(c), body: "Thanks.")).createdId)
    clock.advance(seconds: window + 1)
    #expect(state(c, "outbox_entry/0/state") == "\"sent\"")
    _ = try? c.run(command: .cancelSend(outboxId: id))
    #expect(state(c, "outbox_entry/0/state") == "\"sent\"", "a message that has left cannot be cancelled")
}

@Test func scheduleKeepsItsTimeAndRefusesThePast() throws {
    let clock = FixtureClock(), c = try core(clock)
    let draftId = try #require(try c.run(command: .quickSend(threadId: try firstThread(c), body: "Thanks.")).createdId)
    let later = clock.nowMs() + 20 * 3600 * 1000
    _ = try c.run(command: .schedule(draftId: draftId, atMs: later))
    #expect(state(c, "outbox_entry/1/state") == "\"scheduled\"")
    #expect(throws: CoreError.self) { _ = try c.run(command: .schedule(draftId: draftId, atMs: clock.nowMs() - 1000)) }
}

@Test func aiJobsServeTheirCount() throws {
    let c = try core()
    #expect(state(c, "ai_job/count") == "0", "ASSUMED PATH: the core must say how many AI jobs ran (docs/lanes/tests-j3.md, hand-back)")
}

@Test func theVoiceProfileHasABuildTime() throws {
    let c = try core()
    #expect(state(c, "voice_profile/built_at_ms") != nil, "ASSUMED PATH: the voice profile the J3.3 draft reads has a build time (refreshed every VOICE_REFRESH_DAYS)")
}
