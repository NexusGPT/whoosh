import Foundation
import Testing
@testable import WhooshUI

// fn-pushclient-ios (proof file checks/proofs/g2/acefe421.json): one relay registration per account, a renewal when the token changes, none after removal.
// The relay is a fake behind the RelayAPI port that keeps one row per account like the relay's registry does; nothing is sent over any socket.
// A real relay on the loopback address, and the phone itself, are outside this file (the proof file's lead proof).
// MUTANTS (red-first broken copies; exact edits in docs/lanes/gap-phone-b.md):
//   theThirtyAccountsHoldExactlyOneRegistrationEach     : PushRegistrar.swift `store.routingId(for: accountId) ?? RoutingId.make()` -> `RoutingId.make()` (a new id every call)
//   aSecondSetTokenWithTheSameTokenSendsNothing         : PushRegistrar.swift `let changed = hex != token` -> `let changed = true`
//   aNewTokenRenewsEachAccountOnceKeepingItsRoutingId   : PushRegistrar.swift `wanted.insert(accountId)` removed
//   removingAnAccountDeletesItsRowOnceAndNoneStaysAfter : PushRegistrar.swift `try await relay.delete(accountId: accountId)` -> `try await Task.sleep(nanoseconds: 1)`
//   aFailedRemovalIsKeptAndRetriedUntilTheRelayAnswers  : PushRegistrar.swift `catch { pendingRemoval.insert(accountId); throw error }` -> `catch { throw error }`

private final class FakeRelay: RelayAPI, @unchecked Sendable {
    struct Down: Error {}
    private let lock = NSLock()
    /// The relay's table: one row per (device token, routing id) pair, like relay/src/registry/devices.rs.
    private var rows: [String: RelayRegistration] = [:]
    private var puts = 0, deletes = 0
    private var down = false
    private var failNextUnregister = false
    func setDown(_ d: Bool) { lock.withLock { down = d } }
    func failUnregisterOnce() { lock.withLock { failNextUnregister = true } }
    private static func key(_ routingId: String, _ token: String) -> String { "\(routingId)/\(token)" }
    /// Rows by routing id; two rows for one routing id would show here as a smaller dictionary than the row count.
    var live: [String: RelayRegistration] { lock.withLock { Dictionary(rows.values.map { ($0.routingId, $0) }, uniquingKeysWith: { a, _ in a }) } }
    var rowCount: Int { lock.withLock { rows.count } }
    var putCount: Int { lock.withLock { puts } }
    var deleteCount: Int { lock.withLock { deletes } }
    func register(_ registration: RelayRegistration) async throws {
        try lock.withLock { if down { throw Down() }; puts += 1; rows[Self.key(registration.routingId, registration.deviceToken)] = registration }
    }
    func unregister(routingId: String, deviceToken: String) async throws {
        try lock.withLock {
            if down || failNextUnregister { failNextUnregister = false; throw Down() }
            deletes += 1; rows[Self.key(routingId, deviceToken)] = nil
        }
    }
}

private final class FakeSink: PushSink, @unchecked Sendable {
    private let lock = NSLock()
    private var told: [String: String] = [:]
    var routing: [String: String] { lock.withLock { told } }
    func registerRouting(accountId: String, routingId: String) throws { lock.withLock { told[accountId] = routingId } }
    func unregisterRouting(accountId: String) throws { lock.withLock { told[accountId] = nil } }
}

private let thirty = (1...30).map { "acct-\($0)" }
private func token(_ b: UInt8) -> Data { Data(repeating: b, count: 32) }
private func rig() -> (PushRegistrar, FakeRelay, FakeSink, MemoryRoutingStore) {
    let r = FakeRelay(), s = FakeSink(), m = MemoryRoutingStore()
    return (PushRegistrar(relay: r, sink: s, store: m), r, s, m)
}

@Test func theThirtyAccountsHoldExactlyOneRegistrationEach() async throws {
    let (reg, relay, sink, _) = rig()
    for a in thirty { #expect(try await reg.register(accountId: a) == .waitingForToken, "no token yet: nothing is sent") }
    #expect(relay.putCount == 0)
    try await reg.setToken(token(1))
    #expect(relay.putCount == 30, "one put per account")
    #expect(relay.rowCount == 30)
    let ids = Set(relay.live.values.map(\.routingId))
    #expect(ids.count == 30, "30 different opaque routing ids, one per account")
    #expect(Set(sink.routing.values) == Set(relay.live.keys) && sink.routing.count == 30, "the core was told the id the relay holds, for each account")
    #expect(await reg.registeredAccounts == Set(thirty))
}

@Test func aSecondSetTokenWithTheSameTokenSendsNothing() async throws {
    let (reg, relay, _, _) = rig()
    for a in thirty { try await reg.register(accountId: a) }
    try await reg.setToken(token(1))
    try await reg.setToken(token(1))
    try await reg.setToken(token(1))
    #expect(relay.putCount == 30, "the same token again registers nobody again: \(relay.putCount) puts")
    #expect(relay.rowCount == 30)
}

@Test func aNewTokenRenewsEachAccountOnceKeepingItsRoutingId() async throws {
    let (reg, relay, _, _) = rig()
    for a in thirty { try await reg.register(accountId: a) }
    try await reg.setToken(token(1))
    let before = Set(relay.live.keys)
    try await reg.setToken(token(2))
    #expect(relay.putCount == 60, "one renewal per account: \(relay.putCount) puts")
    #expect(relay.rowCount == 30, "never two rows for an account: the old token's row is deleted (\(relay.rowCount) rows)")
    #expect(relay.deleteCount == 30, "one delete of the old pair per account")
    #expect(Set(relay.live.keys) == before, "the routing id is kept")
    #expect(Set(relay.live.values.map(\.deviceToken)) == [String(repeating: "02", count: 32)], "every row carries the new token")
    #expect(await reg.staleRowCount == 0)
}

@Test func removingAnAccountDeletesItsRowOnceAndNoneStaysAfter() async throws {
    let (reg, relay, sink, store) = rig()
    for a in thirty { try await reg.register(accountId: a) }
    try await reg.setToken(token(1))
    let removedId = try #require(store.routingId(for: "acct-7"))
    try await reg.unregister(accountId: "acct-7")
    #expect(relay.deleteCount == 1)
    #expect(relay.rowCount == 29, "none for the removed account, the other 29 stay")
    #expect(Set(relay.live.keys).isDisjoint(with: [removedId]), "the relay holds no row for the removed routing id")
    #expect(sink.routing["acct-7"] == nil && store.routingId(for: "acct-7") == nil, "the core and the Keychain forget the id")
    try await reg.setToken(token(2))
    #expect(relay.rowCount == 29, "a later renewal does not bring it back")
    try await reg.reconcile(accountIds: Set(thirty).subtracting(["acct-7", "acct-8"]))
    #expect(relay.rowCount == 28, "reconcile unregisters an account that left the list")
}

@Test func aFailedRemovalIsKeptAndRetriedUntilTheRelayAnswers() async throws {
    let (reg, relay, _, store) = rig()
    try await reg.register(accountId: "a")
    try await reg.setToken(token(1))
    relay.setDown(true)
    await #expect(throws: FakeRelay.Down.self) { try await reg.unregister(accountId: "a") }
    #expect(await reg.pendingRemovals == ["a"], "the failed delete waits for a retry")
    #expect(relay.rowCount == 1 && store.routingId(for: "a") != nil, "nothing is forgotten before the relay confirmed")
    relay.setDown(false)
    try await reg.retryPendingRemovals()
    #expect(await reg.pendingRemovals.isEmpty && relay.rowCount == 0, "the retry removed the row")
    #expect(store.routingId(for: "a") == nil)
}

@Test func aFailedDeleteOfTheOldTokensRowIsKeptAndRetried() async throws {
    let (reg, relay, _, _) = rig()
    try await reg.register(accountId: "a")
    try await reg.setToken(token(1))
    relay.failUnregisterOnce()
    await #expect(throws: FakeRelay.Down.self) { try await reg.setToken(token(2)) }
    let staleAfterFailure = await reg.staleRowCount
    #expect(relay.rowCount == 2 && staleAfterFailure == 1, "the new row is in, the old token's row waits for a retry")
    try await reg.retryPendingRemovals()
    let staleAfterRetry = await reg.staleRowCount
    #expect(relay.rowCount == 1 && staleAfterRetry == 0, "the retry left one row for the account")
}

@Test func theRegistrationCarriesTheEnvironmentTheRegistrarWasBuiltWith() async throws {
    let r = FakeRelay()
    let reg = PushRegistrar(relay: r, sink: FakeSink(), store: MemoryRoutingStore(), environment: .production)
    try await reg.register(accountId: "a")
    try await reg.setToken(token(1))
    #expect(Set(r.live.values.map(\.environment)) == [.production])
}
