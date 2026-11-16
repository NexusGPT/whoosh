#if SPINE_PROBE
// Temporary probe of the four spine properties, until Tests/Unit/SpineTests.swift (tests-j1) lands.
// Compiled only with -Xswiftc -DSPINE_PROBE; never part of the shipped build.
import Foundation
import Testing

private func fixtureCore() throws -> FixtureCore {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent()
    return try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock())
}

/// Forwards to the fixture; holds inbox reads at a gate and records every command it is given.
private final class SpyCore: WhooshCore, @unchecked Sendable {
    let inner: FixtureCore
    let gate = DispatchSemaphore(value: 0)
    private let lock = NSLock()
    private var held = false
    private(set) var commands: [Command] = []
    private(set) var subscribeCalls = 0
    init(_ inner: FixtureCore) { self.inner = inner }
    func hold(_ on: Bool) { lock.withLock { held = on } }
    func run(command: Command) throws -> CommandOutcome {
        lock.withLock { commands.append(command) }
        return try inner.run(command: command)
    }
    func inbox(query: InboxQuery) -> [InboxRow] {
        if lock.withLock({ held }) { gate.wait() }
        return inner.inbox(query: query)
    }
    func inboxRows(query: InboxQuery, rows: UInt32) -> [InboxRow] {
        if lock.withLock({ held }) { gate.wait() }
        return inner.inboxRows(query: query, rows: rows)
    }
    func rowsForMessages(messageIds: [String]) -> [InboxRow] { inner.rowsForMessages(messageIds: messageIds) }
    func notchAlerts() -> NotchAlerts { inner.notchAlerts() }
    func notchKind(messageId: String) -> NotchKind { inner.notchKind(messageId: messageId) }
    func counts() -> Counts { inner.counts() }
    func thread(threadId: String) -> MailThread? { inner.thread(threadId: threadId) }
    func state(path: String) -> String? { inner.state(path: path) }
    func subscribe(stream: StreamKind, sink: EventSink) -> UInt64 {
        lock.withLock { subscribeCalls += 1 }
        return inner.subscribe(stream: stream, sink: sink)
    }
    func unsubscribe(subscription: UInt64) { inner.unsubscribe(subscription: subscription) }
    // real core slice 1: detection and the sign-in check have no protocol default any more; the spy forwards them.
    func detectProvider(email: String) -> DetectionResult { inner.detectProvider(email: email) }
    func validateAccount(config: AccountConfig, password: SecretText) -> ValidationReport { inner.validateAccount(config: config, password: password) }
}

private let primary = InboxKey(tabId: "primary")

@MainActor @Test func aRefetchKeepsPriorRowsVisible() async throws {
    let spy = SpyCore(try fixtureCore())
    let spine = Spine(core: spy)
    let store = spine.inbox(primary)
    await store.refetch()
    let before = store.rows.map(\.messageId)
    #expect(before.count == 16)
    spy.hold(true)
    let pending = Task { await store.refetch() }
    while store.phase != .refetching { await Task.yield() }
    #expect(store.rows.map(\.messageId) == before)       // never blank while the read is in flight
    spy.hold(false)
    spy.gate.signal()
    await pending.value
    #expect(store.phase == .idle)
    #expect(store.rows.map(\.messageId) == before)
}

@MainActor @Test func oneStoreInstancePerKeyAndTwoViewsShareIt() throws {
    let spine = Spine(core: try fixtureCore())
    let viewA = spine.inbox(primary)
    let viewB = spine.inbox(InboxKey(tabId: "primary"))
    #expect(viewA === viewB)
    let filtered = spine.inbox(InboxKey(tabId: "primary", accountId: "acc-01"))
    #expect(filtered !== viewA)
    #expect(filtered === spine.inbox(InboxKey(tabId: "primary", accountId: "acc-01")))
    #expect(spine.liveInboxKeys.count == 2)
    #expect(spine.counts === spine.counts)
}

@MainActor @Test func aMutationGoesThroughTheCoreAndBothViewsSeeIt() async throws {
    let spy = SpyCore(try fixtureCore())
    let spine = Spine(core: spy)
    let a = spine.inbox(primary), b = spine.inbox(primary)
    await a.refetch(); await spine.counts.refetch()
    let first = try #require(a.rows.first?.messageId)
    let unread = spine.counts.unread(tab: "primary")
    guard case .done = await spine.run(.archive(messageIds: [first])) else { Issue.record("archive refused"); return }
    #expect(spy.commands.count == 1)                           // the only write path is the core command
    #expect(!a.rows.contains { $0.messageId == first })
    #expect(b.rows.count == a.rows.count && b.rows.count == 15)  // the same store, so the same rows
    #expect(spine.counts.unread(tab: "primary") == unread - 1)
    #expect(spy.inner.state(path: "message/\(first)/archived") == "true")
}

@MainActor @Test func aRefusedCommandKeepsRowsAndSaysWhy() async throws {
    let core = try fixtureCore()
    let spine = Spine(core: core)
    let store = spine.inbox(primary)
    await store.refetch()
    let bad = try #require(store.rows.first { $0.accountId == core.failingAccountId }?.messageId)
    let count = store.rows.count
    guard case .refused(let why) = await spine.run(.archive(messageIds: [bad])) else { Issue.record("expected a refusal"); return }
    #expect(!why.reasonWords.isEmpty && spine.lastFailure?.reasonWords == why.reasonWords)
    #expect(store.rows.count == count)
}

@MainActor @Test func eachStreamIsSubscribedOnceAndStopEndsIt() throws {
    let spy = SpyCore(try fixtureCore())
    let spine = Spine(core: spy)
    #expect(spy.subscribeCalls == 5)
    _ = spine.inbox(primary); _ = spine.inbox(primary)
    #expect(spy.subscribeCalls == 5)
    spine.stop()
}

@MainActor @Test func aPushFromTheCoreReachesTheStoresWithoutAnyRead() async throws {
    let core = try fixtureCore()
    let spine = Spine(core: core)
    let store = spine.inbox(primary)
    await store.refetch(); await spine.counts.refetch()
    let first = try #require(store.rows.first?.messageId)
    let unread = spine.counts.unread(tab: "primary")
    let arrivals = store.arrivals
    _ = try core.run(command: .archive(messageIds: [first]))   // the core changes on its own (a sync), not through the spine
    #expect(store.arrivals == arrivals + 1)
    #expect(!store.rows.contains { $0.messageId == first })
    #expect(spine.counts.unread(tab: "primary") == unread - 1)
}
#endif
