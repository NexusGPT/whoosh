import Foundation
import CryptoKit

/// How long to wait before the next attempt after a failed push: the listed seconds in turn, the last one repeating.
/// The numbers are supplied by the caller (they belong in docs/CONSTANTS.md, see the lane notes).
public struct RetryPolicy: Sendable {
    public let backoffSeconds: [Int]

    public init(backoffSeconds: [Int]) { self.backoffSeconds = backoffSeconds }

    func delayMs(afterFailures failures: Int) -> Int64 {
        guard let last = backoffSeconds.last else { return 0 }
        let index = min(max(failures - 1, 0), backoffSeconds.count - 1)
        _ = last
        return Int64(backoffSeconds[index]) * 1000
    }
}

public enum DeviceSyncState: Equatable, Sendable {
    case idle
    case inSync(atMs: Int64)
    case offline(pending: Int, retryAtMs: Int64)
}

/// One device's side of the sync: local rows, an outgoing queue, encrypted push, decrypted pull, latest write wins.
public final class DeviceSyncEngine: @unchecked Sendable {
    public let deviceId: String
    private let port: DeviceSyncPort
    private let keys: SyncKeyProvider
    private let clock: () -> Int64
    private let retry: RetryPolicy

    private let lock = NSLock()
    private var rows: [String: SyncedRow] = [:]
    private var queue: [String] = []
    private var failures = 0
    private var retryAtMs: Int64 = 0
    private var token: String?
    private var lastSyncMs: Int64?
    private var lastWasFailure = false

    public init(deviceId: String, port: DeviceSyncPort, keys: SyncKeyProvider, retry: RetryPolicy,
                clock: @escaping () -> Int64) {
        self.deviceId = deviceId
        self.port = port
        self.keys = keys
        self.retry = retry
        self.clock = clock
    }

    // MARK: reading

    public func row(kind: String, id: String) -> SyncedRow? {
        lock.lock(); defer { lock.unlock() }
        return rows["\(kind)|\(id)"]
    }

    public func rows(ofKind kind: String) -> [SyncedRow] {
        lock.lock(); defer { lock.unlock() }
        return rows.values.filter { $0.kind == kind }.sorted { $0.id < $1.id }
    }

    public var pendingCount: Int {
        lock.lock(); defer { lock.unlock() }
        return queue.count
    }

    public var state: DeviceSyncState {
        lock.lock(); defer { lock.unlock() }
        if lastWasFailure { return .offline(pending: queue.count, retryAtMs: retryAtMs) }
        if let at = lastSyncMs { return .inSync(atMs: at) }
        return .idle
    }

    /// the line the Sync settings row shows (J4.10, J4.11)
    public var statusLine: String {
        switch state {
        case .idle: return "Not synced yet"
        case .inSync: return "In sync"
        case .offline(let pending, _): return "Offline, \(pending) waiting, retrying on its own"
        }
    }

    // MARK: writing

    /// Write a row on this device. It is sealed and pushed; when the push fails it waits in the queue and retries on its own.
    @discardableResult
    public func write(kind: String, id: String, fields: [String: String]) throws -> SyncedRow {
        guard SyncedKinds.isSynced(kind) else { throw SyncRefusal.localOnly(kind) }
        if let bad = CredentialGuard.offence(in: fields) { throw SyncRefusal.credential(bad) }
        let row = SyncedRow(kind: kind, id: id, fields: fields, updatedMs: clock(), deviceId: deviceId)
        lock.lock()
        rows["\(kind)|\(id)"] = row
        queue.removeAll { $0 == "\(kind)|\(id)" }
        queue.append("\(kind)|\(id)")
        lock.unlock()
        flush()
        return row
    }

    /// Push everything waiting. A failure keeps the queue and sets the next attempt time.
    public func flush() {
        var pushed = false
        while true {
            lock.lock()
            guard let key = queue.first, let row = rows[key] else {
                if queue.isEmpty { if pushed { markSuccessLocked() } } else { queue.removeFirst() }
                let more = !queue.isEmpty
                lock.unlock()
                if more { continue }
                return
            }
            lock.unlock()
            do {
                try port.push(try sealed(row))
            } catch {
                lock.lock()
                failures += 1
                lastWasFailure = true
                retryAtMs = clock() + retry.delayMs(afterFailures: failures)
                lock.unlock()
                return
            }
            lock.lock()
            if queue.first == key { queue.removeFirst() }
            pushed = true
            lock.unlock()
        }
    }

    /// Called as time passes: tries again when the wait is over.
    public func tick() {
        lock.lock()
        let due = lastWasFailure && clock() >= retryAtMs
        lock.unlock()
        if due { flush() }
    }

    private func markSuccessLocked() {
        failures = 0
        lastWasFailure = false
        lastSyncMs = clock()
    }

    private func sealed(_ row: SyncedRow) throws -> PortChange {
        let body = try SealedBody.encode(fields: row.fields, device: row.deviceId)
        let box = try SyncCrypto.seal(body, kind: row.kind, id: row.id, key: try keys.key())
        return PortChange(recordKind: row.kind, id: row.id, payload: box, changedAtMs: row.updatedMs)
    }

    // MARK: reading from the cloud

    /// Pull what the other devices wrote. Latest write wins, a tie goes to the larger device id. Returns rows applied.
    @discardableResult
    public func pull() throws -> Int {
        let batch: PortBatch
        do {
            lock.lock(); let since = token; lock.unlock()
            batch = try port.pull(sinceToken: since)
        } catch {
            lock.lock()
            failures += 1
            lastWasFailure = true
            retryAtMs = clock() + retry.delayMs(afterFailures: failures)
            lock.unlock()
            throw error
        }
        var applied = 0
        for change in batch.changes {
            guard SyncedKinds.isSynced(change.recordKind) else { continue }
            let plain = try SyncCrypto.open(change.payload, kind: change.recordKind, id: change.id, key: try keys.key())
            let body = try SealedBody.decode(plain)
            let incoming = SyncedRow(kind: change.recordKind, id: change.id, fields: body.fields,
                                     updatedMs: change.changedAtMs, deviceId: body.device)
            lock.lock()
            let key = "\(incoming.kind)|\(incoming.id)"
            let wins: Bool
            if let have = rows[key] {
                wins = (incoming.updatedMs, incoming.deviceId) > (have.updatedMs, have.deviceId)
            } else {
                wins = true
            }
            if wins { rows[key] = incoming; applied += 1 }
            lock.unlock()
        }
        lock.lock()
        token = batch.nextToken ?? token
        markSuccessLocked()
        lock.unlock()
        return applied
    }
}
