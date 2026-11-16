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

@Test func aQuickSendWaitsInTheUndoWindowThenGoes() throws {
    let (c, clock) = try core()
    let out = try c.run(command: .quickSend(threadId: "msg-04", body: "Thanks."))
    #expect(out.createdId != nil)
    #expect(c.state(path: "outbox_entry/count") == "1")
    #expect(c.state(path: "outbox_entry/0/state") == "\"undoable\"")              // mutant: sent at once
    clock.advance(seconds: Double(WhooshConstants.UNDO_SEND_DEFAULT_S) + 1)
    #expect(c.state(path: "outbox_entry/0/state") == "\"sent\"")                  // mutant: never leaves
}
@Test func cancellingInsideTheWindowStopsTheSendForGood() throws {
    let (c, clock) = try core()
    let id = try #require(try c.run(command: .quickSend(threadId: "msg-04", body: "Thanks.")).createdId)
    _ = try c.run(command: .cancelSend(outboxId: id))
    clock.advance(seconds: 60)
    #expect(c.state(path: "outbox_entry/0/state") == "\"cancelled\"")             // mutant: a cancelled entry is sent later
}
@Test func cancellingAnEntryThatDoesNotExistIsNotFound() throws {
    let (c, _) = try core()
    do { _ = try c.run(command: .cancelSend(outboxId: "outbox-999")); Issue.record("accepted") }
    catch CoreError.NotFound(let kind, _) { #expect(kind == "outbox entry") }
}
// swift-tests lane: FixtureCore.deliver (FixtureCore.swift, `static func deliver`) hands an event to the sink synchronously only when the caller is on the
// main thread, else through the main queue; a plain `@Test` runs on a pool thread and read the sink before the event arrived (main76: heard 0 / 1 of 2).
// `@MainActor` puts the test on the main thread, which is where the app subscribes.
@MainActor @Test func theOutboxStreamTellsSubscribersAboutTheNewEntry() throws {
    let (c, _) = try core(); let rec = Recorder()
    _ = c.subscribe(stream: .outbox, sink: rec)
    _ = try c.run(command: .quickSend(threadId: "msg-04", body: "Thanks."))
    #expect(rec.events.contains { if case .outboxTick = $0 { return true } else { return false } })   // the fixture core emits nothing on .outbox today
}
@Test func aQuickSendOnTheFailingAccountQueuesNothing() throws {
    let (c, _) = try core()
    do { _ = try c.run(command: .quickSend(threadId: "msg-03", body: "x")); Issue.record("accepted") } catch CoreError.Action { }
    #expect(c.state(path: "outbox_entry/count") == "0")
}
