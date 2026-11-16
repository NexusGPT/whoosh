import Foundation
import Testing
@testable import WhooshUI

/// Probes for contracts delta 15: the sent reply joins its thread, each account's signature, and the search arm's lookup.
private let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func core(_ clock: FixtureClock = FixtureClock()) throws -> FixtureCore {
    try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: clock)
}
private final class Collect: EventSink, @unchecked Sendable {
    let lock = NSLock(); var events: [CoreEvent] = []
    func onEvent(event: CoreEvent) { lock.withLock { events.append(event) } }
}

@MainActor @Test func d15ASentReplyJoinsItsThreadOnceAndNeverTheInbox() throws {
    let clock = FixtureClock()
    let c = try core(clock)
    let sink = Collect()
    _ = c.subscribe(stream: .thread, sink: sink)
    let outbox = try #require(try c.run(command: .quickSend(threadId: "msg-03", body: "Thanks, see you Thursday.")).createdId)
    let sentId = "sent-\(outbox)"
    #expect(!(c.thread(threadId: "msg-03")?.messageIds ?? []).contains(sentId), "not listed during the undo window")
    #expect(clock.advance(seconds: Double(WhooshConstants.UNDO_SEND_DEFAULT_S) + 1))
    let ids = try #require(c.thread(threadId: "msg-03")).messageIds
    #expect(ids.filter { $0 == sentId }.count == 1 && ids.first == "msg-03", "the thread's own message, then the reply: \(ids)")
    let m = try #require(c.message(messageId: sentId))
    let from = try #require(c.accounts().first { $0.id == m.accountId }).address
    #expect(m.isOutgoing && m.fromAddress == from && m.folder == "Sent" && m.preview.hasPrefix("Thanks, see you Thursday."))
    #expect(!c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)).contains { $0.messageId == sentId }, "a sent copy is no inbox row")
    let changed = sink.lock.withLock { sink.events.compactMap { if case .threadChanged(let t) = $0 { return t } else { return nil } } }
    #expect(changed.contains { $0.id == "msg-03" && $0.messageIds.contains(sentId) }, "ThreadChanged fires")
    #expect(clock.advance(seconds: 60))
    #expect(c.thread(threadId: "msg-03")?.messageIds.filter { $0 == sentId }.count == 1, "once, however long after")
}

@MainActor @Test func d15ARefusedSendIsNeverListedInItsThread() throws {
    let clock = FixtureClock()
    let c = try core(clock)
    #expect(c.failNext("send", reason: "the mail server did not accept the message", code: "550 5.7.1"))
    let outbox = try #require(try c.run(command: .quickSend(threadId: "msg-03", body: "Refused")).createdId)
    #expect(clock.advance(seconds: Double(WhooshConstants.UNDO_SEND_DEFAULT_S) + 1))
    #expect(!(c.thread(threadId: "msg-03")?.messageIds ?? []).contains("sent-\(outbox)"))
    #expect(c.message(messageId: "sent-\(outbox)") == nil)
}

@Test func d15EveryAccountReadsItsOwnSignatureAndTheFrenchOneIsFrench() throws {
    let c = try core()
    let accounts = c.accounts()
    #expect(!accounts.isEmpty && accounts.allSatisfy { $0.signature.contains("\n") && $0.signature.contains($0.name) }, "two lines with the account's name")
    let fr = try #require(accounts.first { $0.id == "acc-05" })
    #expect(fr.signature.hasPrefix("Bien à vous,"))
    #expect(c.signature(address: fr.address)?.text == fr.signature, "the address read agrees with the account read")
    let work = try #require(accounts.first { $0.id == "acc-01" })
    #expect(work.signature == "Best,\nAssem Chammah · \(work.name)")
}

@Test func d15TheSearchAsksTheLookupOnlyForTheIdsTheRankerTouches() throws {
    let c = try core()
    let calls = Counter()
    let saved = FixtureCore.meaningRanker
    defer { FixtureCore.meaningRanker = saved }
    FixtureCore.meaningRanker = { _, ids, text in
        let touched = Array(ids.prefix(3))
        for id in touched { calls.add(); _ = text(id) }
        return touched.map { (id: $0, score: 0.9) }
    }
    _ = try c.run(command: .search(query: "quote"))
    #expect(calls.value == 3, "the arm builds no body string the ranker did not ask for")
    _ = try c.run(command: .search(query: "\"exact words\""))
    #expect(calls.value == 3, "a quoted query never asks the meaning ranker")
}

private final class Counter: @unchecked Sendable {
    private let lock = NSLock(); private var n = 0
    func add() { lock.withLock { n += 1 } }
    var value: Int { lock.withLock { n } }
}
