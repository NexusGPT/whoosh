import Foundation

/// A local stand-in for the CloudKit private database, behind the same push and pull port. It holds exactly what a
/// server would hold: kind, id, time and sealed bytes. Optionally persisted to a file so two processes can share it.
/// It can go offline. The real container needs the iCloud container and a signed app (docs/lanes/devicesync.md).
public final class FakeCloudContainer: DeviceSyncPort, @unchecked Sendable {
    private struct Stored: Codable {
        var seq: Int
        var kind: String
        var id: String
        var payload: Data
        var changedAtMs: Int64
    }

    private let lock = NSLock()
    private var records: [String: Stored] = [:]
    private var nextSeq = 1
    private var isOnline = true
    private let fileURL: URL?
    private var pushes = 0

    public init(fileURL: URL? = nil) {
        self.fileURL = fileURL
        if let url = fileURL, let data = try? Data(contentsOf: url),
           let loaded = try? JSONDecoder().decode([Stored].self, from: data) {
            for item in loaded { records["\(item.kind)|\(item.id)"] = item }
            nextSeq = (loaded.map(\.seq).max() ?? 0) + 1
        }
    }

    public var online: Bool {
        get { lock.lock(); defer { lock.unlock() }; return isOnline }
        set { lock.lock(); isOnline = newValue; lock.unlock() }
    }

    public func push(_ change: PortChange) throws {
        lock.lock(); defer { lock.unlock() }
        guard isOnline else { throw SyncPortError.offline }
        let key = "\(change.recordKind)|\(change.id)"
        // the record store keeps the latest version of a record, as a CloudKit record does
        records[key] = Stored(seq: nextSeq, kind: change.recordKind, id: change.id, payload: change.payload,
                              changedAtMs: change.changedAtMs)
        nextSeq += 1
        pushes += 1
        persist()
    }

    public func pull(sinceToken: String?) throws -> PortBatch {
        lock.lock(); defer { lock.unlock() }
        guard isOnline else { throw SyncPortError.offline }
        let since = Int(sinceToken ?? "") ?? 0
        let fresh = records.values.filter { $0.seq > since }.sorted { $0.seq < $1.seq }
        let changes = fresh.map { PortChange(recordKind: $0.kind, id: $0.id, payload: $0.payload, changedAtMs: $0.changedAtMs) }
        return PortBatch(changes: changes, nextToken: String(nextSeq - 1))
    }

    /// how many pushes the container accepted, to see that a retry sends a record once
    public var pushCount: Int {
        lock.lock(); defer { lock.unlock() }
        return pushes
    }

    /// everything the server holds, for the field scan
    public var rawRecords: [PortChange] {
        lock.lock(); defer { lock.unlock() }
        return records.values.sorted { $0.seq < $1.seq }.map {
            PortChange(recordKind: $0.kind, id: $0.id, payload: $0.payload, changedAtMs: $0.changedAtMs)
        }
    }

    /// how many records are CloudKit assets (files) rather than plain encrypted fields
    public var assetCount: Int {
        rawRecords.filter { SyncedKinds.isAsset($0.recordKind) }.count
    }

    /// replace a stored payload, as a hostile server could (tamper test)
    public func overwritePayload(kind: String, id: String, with payload: Data) {
        lock.lock(); defer { lock.unlock() }
        if var item = records["\(kind)|\(id)"] {
            item.payload = payload
            item.seq = nextSeq
            nextSeq += 1
            records["\(kind)|\(id)"] = item
        }
    }

    private func persist() {
        guard let url = fileURL, let data = try? JSONEncoder().encode(Array(records.values)) else { return }
        try? data.write(to: url, options: .atomic)
    }
}
