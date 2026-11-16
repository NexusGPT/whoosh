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

@Test func theCountsFollowTheFixtureAtTheStart() throws {
    let (c, _) = try core()
    let counts = c.counts()
    // swift-tests lane (main76: tab ids primary, all, needs-reply, updates, promotions, lbl-6; Primary unread 24): the tab list is derived from the labels by the contract's
    // `inboxTabs(labels:)` (Sources/CoreBridge/FixtureLabels.swift:77-82), so the first five are the built-ins in this order and a pinned label follows; the file's `tabs`
    // (primary, needs-reply, updates, promotions, social, vips) is the pre-labels design. Primary is the file's figure plus the flow example's rejected rows filed there (FixtureFileFigures.primaryUnread).
    #expect(Array(counts.tabs.map(\.tabId).prefix(5)) == ["primary", "all", "needs-reply", "updates", "promotions"])
    let primaryUnread = UInt32(try FixtureFileFigures.primaryUnread())   // the core's counts are UInt32
    #expect(counts.tabs.first { $0.tabId == "primary" }?.unread == primaryUnread)
    #expect(counts.tabs.first { $0.tabId == "primary" }?.waiting == 4)
    #expect(counts.mailboxes["snoozed"] == 7)
}
@Test func snoozedCountGrowsByOneAndUndoTakesItBack() throws {
    let (c, _) = try core()
    _ = try c.run(command: .snooze(messageIds: ["msg-02"], when: .tomorrowMorning))
    #expect(c.counts().mailboxes["snoozed"] == 8)                                   // mutant: not recomputed
    _ = try c.run(command: .undo)
    #expect(c.counts().mailboxes["snoozed"] == 7)
}
// swift-tests lane: FixtureCore.deliver (FixtureCore.swift, `static func deliver`) hands an event to the sink synchronously only when the caller is on the
// main thread, else through the main queue; a plain `@Test` runs on a pool thread and read the sink before the event arrived (main76: heard 0 / 1 of 2).
// `@MainActor` puts the test on the main thread, which is where the app subscribes.
@MainActor @Test func theSyncStatusStreamReportsTheFailingAccountByName() throws {
    let (c, _) = try core(); let rec = Recorder()
    _ = c.subscribe(stream: .syncStatus, sink: rec)
    _ = try? c.run(command: .archive(messageIds: ["msg-03"]))
    let named = rec.events.contains { if case .syncStateChanged(let s) = $0 { return s.accountId == "acc-18" && s.lastError != nil } else { return false } }
    #expect(named, "the failing account must surface on the syncStatus stream")      // the fixture core emits nothing on .syncStatus today
}
@Test func aReadOfAnUnknownCorePathIsNilNotAGuess() throws {
    let (c, _) = try core()
    #expect(c.state(path: "message/msg-01/nonsense") == nil)                        // mutant: returns "false" for any field
    #expect(c.state(path: "message/msg-01/archived") != nil)                        // positive control
}
