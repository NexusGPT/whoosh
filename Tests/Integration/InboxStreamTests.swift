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

// swift-tests lane: FixtureCore.deliver (FixtureCore.swift, `static func deliver`) hands an event to the sink synchronously only when the caller is on the
// main thread, else through the main queue; a plain `@Test` runs on a pool thread and read the sink before the event arrived (main76: heard 0 / 1 of 2).
// `@MainActor` puts the test on the main thread, which is where the app subscribes.
@MainActor @Test func anArchiveReachesAnInboxSubscriberWithoutTheArchivedRow() throws {
    let (c, _) = try core(); let rec = Recorder()
    let sub = c.subscribe(stream: .inbox, sink: rec)
    let first = primary(c)[0].messageId
    let before = primary(c).count   // the file's messages plus the flow example's rejected rows, one fewer after the archive
    _ = try c.run(command: .archive(messageIds: [first]))
    guard case .inboxChanged(let tab, let rows)? = rec.events.last else { Issue.record("no inboxChanged event"); return }
    #expect(tab == "primary")
    #expect(!rows.contains { $0.messageId == first })                               // mutant: stale rows in the event
    #expect(rows.count == before - 1)
    c.unsubscribe(subscription: sub)
}
@MainActor @Test func anUnsubscribedSinkHearsNothing() throws {
    let (c, _) = try core(); let rec = Recorder()
    let sub = c.subscribe(stream: .inbox, sink: rec)
    _ = try c.run(command: .pin(messageId: "msg-05", pinned: true))
    let heard = rec.events.count
    #expect(heard >= 1)                            // positive control: the sink was live
    c.unsubscribe(subscription: sub)
    _ = try c.run(command: .pin(messageId: "msg-06", pinned: true))
    #expect(rec.events.count == heard)             // mutant: unsubscribe is a no-op
}
@MainActor @Test func theCountsStreamCarriesTheNewUnreadFigure() throws {
    let (c, _) = try core(); let rec = Recorder()
    _ = c.subscribe(stream: .counts, sink: rec)
    // Primary starts at the file's figure plus the flow example's unread rejected rows (FixtureFileFigures.primaryUnread); archiving one unread row takes it to one less.
    let unreadBefore = c.counts().tabs.first { $0.tabId == "primary" }?.unread
    _ = try c.run(command: .archive(messageIds: [primary(c)[0].messageId]))
    guard case .countsChanged(let counts)? = rec.events.last else { Issue.record("no countsChanged event"); return }
    let startFigure = UInt32(try FixtureFileFigures.primaryUnread())   // the core's counts are UInt32
    #expect(unreadBefore == startFigure)
    #expect(counts.tabs.first { $0.tabId == "primary" }?.unread == startFigure - 1)             // mutant: counts not recomputed
}
