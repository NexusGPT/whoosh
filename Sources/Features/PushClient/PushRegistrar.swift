import Foundation

/// Keeps exactly one relay registration per account: register, renew when the APNs token changes, unregister on removal.
/// The core is told the routing id through the frozen `PushSink` (register_routing / unregister_routing).
public actor PushRegistrar {
    public enum Result: Sendable, Equatable { case registered(routingId: String), waitingForToken }

    private let relay: RelayAPI
    private let sink: PushSink
    private let store: RoutingStore
    private var token: String?
    private var wanted = Set<String>()          // accounts that must be registered
    private var registered = Set<String>()      // accounts the relay accepted with the current token
    private var pendingRemoval = Set<String>()  // unregister that failed and must be retried
    private var staleRows: [StaleRow] = []      // (routing id, old token) pairs the relay still holds after a token change
    private let environment: RelayEnvironment

    /// A relay row left behind by a token change: the relay keys a row by (token, routing id), so the old pair must be deleted.
    private struct StaleRow: Equatable { let routingId: String; let deviceToken: String }

    public init(relay: RelayAPI, sink: PushSink, store: RoutingStore, environment: RelayEnvironment = .forThisBuild) {
        self.relay = relay; self.sink = sink; self.store = store; self.environment = environment
    }

    public var registeredAccounts: Set<String> { registered }
    public var pendingRemovals: Set<String> { pendingRemoval }
    /// Old-token rows still to delete on the relay (count only; the pairs stay private).
    public var staleRowCount: Int { staleRows.count }

    /// The system handed over a (new) APNs token: renew every account that needs one.
    public func setToken(_ deviceToken: Data) async throws {
        let hex = deviceToken.map { String(format: "%02x", $0) }.joined()
        let changed = hex != token
        let old = token
        token = hex
        if changed {
            // Every account the relay holds under the old token keeps its routing id, so the old pair is a second row: delete it after the new one is in.
            if let old { for account in registered { if let routing = store.routingId(for: account) { staleRows.append(StaleRow(routingId: routing, deviceToken: old)) } } }
            registered.removeAll()
            try await renew()
            try await sweepStale()
        }
        else { for account in wanted.subtracting(registered) { _ = try await register(accountId: account) } }
    }

    @discardableResult
    public func register(accountId: String) async throws -> Result {
        wanted.insert(accountId)
        guard let token else { return .waitingForToken }
        let routing = store.routingId(for: accountId) ?? RoutingId.make()
        try await relay.register(RelayRegistration(deviceToken: token, routingId: routing, environment: environment))
        store.save(routing, for: accountId)
        try sink.registerRouting(accountId: accountId, routingId: routing)
        registered.insert(accountId)
        return .registered(routingId: routing)
    }

    /// Re-sends every wanted account (token changed, or the system asked). Same routing id, so the relay still holds one row each.
    public func renew() async throws {
        for account in wanted.sorted() { _ = try await register(accountId: account) }
    }

    /// Account removed: the relay deletes its row, the core forgets the id, the Keychain item goes. A failed relay call is retried.
    public func unregister(accountId: String) async throws {
        wanted.remove(accountId); registered.remove(accountId)
        do { if let token, let routing = store.routingId(for: accountId) { try await relay.unregister(routingId: routing, deviceToken: token) } }
        catch { pendingRemoval.insert(accountId); throw error }
        pendingRemoval.remove(accountId)
        try sink.unregisterRouting(accountId: accountId)
        store.remove(accountId: accountId)
    }

    public func retryPendingRemovals() async throws {
        for account in pendingRemoval.sorted() { try await unregister(accountId: account) }
        try await sweepStale()
    }

    /// Deletes the old-token rows one by one; one that fails stays in the list for the next retry.
    private func sweepStale() async throws {
        var firstError: Error?
        for row in staleRows {
            do { try await relay.unregister(routingId: row.routingId, deviceToken: row.deviceToken); staleRows.removeAll { $0 == row } }
            catch { firstError = firstError ?? error }
        }
        if let firstError { throw firstError }
    }

    /// The account list changed (J4.10 added accounts, a removal): register what is missing, unregister what is gone.
    public func reconcile(accountIds: Set<String>) async throws {
        for gone in wanted.subtracting(accountIds).sorted() { try await unregister(accountId: gone) }
        for added in accountIds.subtracting(wanted).sorted() { _ = try await register(accountId: added) }
    }
}
