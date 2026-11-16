import Foundation
import Testing
@testable import WhooshUI

// Integration over the core seam (WhooshCore), through the fixture core now and the Rust core when it lands.
// Written from docs/CAPABILITY-CONTRACT.md and docs/JOURNEYS.md, never from a built screen.

private func core() throws -> (FixtureCore, FixtureClock) {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let clock = FixtureClock()
    return (try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: clock), clock)
}
private func primary(_ c: WhooshCore) -> [InboxRow] { c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)) }
private final class Recorder: EventSink, @unchecked Sendable {
    private let lock = NSLock(); private var seen: [CoreEvent] = []
    var events: [CoreEvent] { lock.withLock { seen } }
    func onEvent(event: CoreEvent) { lock.withLock { seen.append(event) } }
}

@Test func everyCommandOnTheFailingAccountFailsInWordsNamingTheAccount() throws {
    let (c, _) = try core()
    for cmd in [Command.archive(messageIds: ["msg-03"]), .snooze(messageIds: ["msg-03"], when: .tomorrowMorning),
                .label(messageIds: ["msg-03"], labelName: "Suppliers", createIfMissing: true), .moveToAccount(messageIds: ["msg-03"], accountId: "acc-01")] {
        do { _ = try c.run(command: cmd); Issue.record("expected a refusal for \(cmd)") }
        catch CoreError.Action(let f) { #expect(f.accountName == "Provanta RFP"); #expect(f.reasonWords == "needs you to sign in again"); #expect(f.retryable) }
    }
    #expect(c.state(path: "message/msg-03/archived") == "false")                    // the refusal left the row alone
}
@Test func aSnoozeInThePastIsRefusedWithAReason() throws {
    let (c, _) = try core()
    do { _ = try c.run(command: .snooze(messageIds: ["msg-02"], when: .at(atMs: 1))); Issue.record("accepted a past time") }
    catch CoreError.Invalid(let field, let words) { #expect(field == "when"); #expect(words == "that time has already passed") }
    #expect(c.state(path: "message/msg-02/snoozed") == "false")
}
@Test func undoTwiceWithOneEntryFailsTheSecondTimeWithNothingToUndo() throws {
    let (c, _) = try core()
    _ = try c.run(command: .archive(messageIds: ["msg-01"]))
    _ = try c.run(command: .undo)
    #expect(c.state(path: "message/msg-01/archived") == "false")
    do { _ = try c.run(command: .undo); Issue.record("second undo accepted") }       // idempotency: one entry, one undo
    catch CoreError.Invalid(_, let words) { #expect(words == "Nothing to undo") }
}
@Test func undoReversesTheLastActionFirst() throws {
    let (c, _) = try core()
    _ = try c.run(command: .archive(messageIds: ["msg-01"])); _ = try c.run(command: .pin(messageId: "msg-05", pinned: true))
    _ = try c.run(command: .undo)
    #expect(c.state(path: "pin/msg-05") == "false")                                 // mutant: undo takes the oldest entry
    #expect(c.state(path: "message/msg-01/archived") == "true")
}
@Test func theUndoStackKeepsOnlyUndoDepthEntries() throws {
    let (c, _) = try core()
    for i in 0..<(WhooshConstants.UNDO_DEPTH + 5) { _ = try c.run(command: .pin(messageId: "msg-0\(i % 9 + 1)", pinned: i % 2 == 0)) }
    #expect(c.state(path: "undo_entry/count") == "\(WhooshConstants.UNDO_DEPTH)")  // mutant: unbounded stack
}
@Test func labelWithAnEmptyNameIsRefused() throws {
    let (c, _) = try core()
    do { _ = try c.run(command: .label(messageIds: ["msg-04"], labelName: "", createIfMissing: true)); Issue.record("empty label accepted") }
    catch CoreError.Invalid { }                                                     // coercion bait: "" must not become a label
    #expect(c.state(path: "label_assignment/msg-04") == "[]")
}
