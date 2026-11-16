import Foundation

/// Adapters between the engine's port and the frozen contract trait CloudSync (push, pull). Compiled only inside the
/// package; the standalone check compiles every other file in this folder without it.

/// the engine talks to any contract CloudSync (the real CloudKit one, once it exists)
public final class CloudSyncPortAdapter: DeviceSyncPort, @unchecked Sendable {
    private let cloud: CloudSync

    public init(_ cloud: CloudSync) { self.cloud = cloud }

    public func push(_ change: PortChange) throws {
        try cloud.push(change: SyncChange(recordKind: change.recordKind, id: change.id, payload: change.payload,
                                          changedAtMs: change.changedAtMs))
    }

    public func pull(sinceToken: String?) throws -> PortBatch {
        let batch = try cloud.pull(sinceToken: sinceToken)
        return PortBatch(changes: batch.changes.map {
            PortChange(recordKind: $0.recordKind, id: $0.id, payload: $0.payload, changedAtMs: $0.changedAtMs)
        }, nextToken: batch.nextToken)
    }
}

/// the fake container seen as a contract CloudSync, for tests that hand a CloudSync to the core
public final class FakeCloudSync: CloudSync, @unchecked Sendable {
    public let container: FakeCloudContainer

    public init(_ container: FakeCloudContainer) { self.container = container }

    public func push(change: SyncChange) throws {
        try container.push(PortChange(recordKind: change.recordKind, id: change.id, payload: change.payload,
                                      changedAtMs: change.changedAtMs))
    }

    public func pull(sinceToken: String?) throws -> SyncBatch {
        let batch = try container.pull(sinceToken: sinceToken)
        return SyncBatch(changes: batch.changes.map {
            SyncChange(recordKind: $0.recordKind, id: $0.id, payload: $0.payload, changedAtMs: $0.changedAtMs)
        }, nextToken: batch.nextToken)
    }
}
