import Foundation
import Testing
@testable import WhooshUI

/// Probes for the 2026-11-07 root delta (findings from the contracttests and tests-j1 lanes).
private func core(_ clock: FixtureClock = FixtureClock()) throws -> FixtureCore {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    return try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: clock)
}

private final class Collect: EventSink, @unchecked Sendable {
    let lock = NSLock(); var events: [CoreEvent] = []
    func onEvent(event: CoreEvent) { lock.withLock { events.append(event) } }
}

@Test func clockRefusesAHugeOrNonFiniteAdvance() {
    let c = FixtureClock()
    let before = c.nowMs()
    #expect(!c.advance(seconds: 1e308))
    #expect(!c.advance(seconds: .infinity))
    #expect(!c.advance(seconds: .nan))
    #expect(c.nowMs() - before < 60_000)
    #expect(c.advance(seconds: 11))
}

@Test func aBooleanArgumentIsRefused() {
    if case .success = ControlRequest.parse(#"{"verb":"key","args":[true]}"#) { Issue.record("a boolean was accepted") }
    if case .failure = ControlRequest.parse(#"{"verb":"clock","args":["advance",11]}"#) { Issue.record("a number was refused") }
}

@Test func reauthMakesTheFailingAccountHealthy() throws {
    let c = try core()
    let id = try #require(c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)).first { $0.accountId == "acc-18" }?.messageId)
    #expect(throws: CoreError.self) { try c.run(command: .archive(messageIds: [id])) }
    #expect(!c.reauth(accountId: "acc-04"))
    #expect(c.reauth(accountId: "acc-18"))
    _ = try c.run(command: .archive(messageIds: [id]))
    #expect(c.syncStates().allSatisfy { $0.lastError == nil })
}

@Test @MainActor func syncStatusAndOutboxStreamsSendTheirStateOnSubscribe() throws {
    let c = try core()
    let sync = Collect(), out = Collect()
    _ = c.subscribe(stream: .syncStatus, sink: sync)
    _ = c.subscribe(stream: .outbox, sink: out)
    #expect(sync.events.count == 30)
    let failing = sync.events.compactMap { if case .syncStateChanged(let s) = $0, s.lastError != nil { return s.accountId } else { return nil } }
    #expect(failing == ["acc-18"])
    #expect(out.events.isEmpty && c.outbox().isEmpty)
    _ = try c.run(command: .quickSend(threadId: "msg-01", body: "Thanks."))
    #expect(c.outbox().count == 1)
    #expect(out.events.count == 1)
}

@Test func undoIsLastInFirstOut() throws {
    let c = try core()
    let rows = c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil))
    _ = try c.run(command: .archive(messageIds: [rows[0].messageId]))
    _ = try c.run(command: .archive(messageIds: [rows[3].messageId]))
    _ = try c.run(command: .undo)
    #expect(c.state(path: "message/\(rows[3].messageId)/archived") == "false")
    #expect(c.state(path: "message/\(rows[0].messageId)/archived") == "true")
}
