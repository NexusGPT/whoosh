import Foundation

/// The plain row a device holds and syncs: one STATE (or STATE + FILE) record of docs/DATA-MAP.md.
public struct SyncedRow: Equatable, Sendable {
    public var kind: String
    public var id: String
    public var fields: [String: String]
    public var updatedMs: Int64
    public var deviceId: String

    public init(kind: String, id: String, fields: [String: String], updatedMs: Int64, deviceId: String) {
        self.kind = kind
        self.id = id
        self.fields = fields
        self.updatedMs = updatedMs
        self.deviceId = deviceId
    }
}

/// What the cloud container stores. It mirrors the contract's SyncChange; the bridge file converts.
public struct PortChange: Equatable, Sendable {
    public var recordKind: String
    public var id: String
    public var payload: Data
    public var changedAtMs: Int64

    public init(recordKind: String, id: String, payload: Data, changedAtMs: Int64) {
        self.recordKind = recordKind
        self.id = id
        self.payload = payload
        self.changedAtMs = changedAtMs
    }
}

public struct PortBatch: Equatable, Sendable {
    public var changes: [PortChange]
    public var nextToken: String?

    public init(changes: [PortChange], nextToken: String?) {
        self.changes = changes
        self.nextToken = nextToken
    }
}

/// The port the engine talks to. The contract's CloudSync (push, pull) is adapted onto it in ContractBridge.swift.
public protocol DeviceSyncPort: AnyObject {
    func push(_ change: PortChange) throws
    func pull(sinceToken: String?) throws -> PortBatch
}

public enum SyncPortError: Error, Equatable {
    case offline
}

public enum SyncRefusal: Error, Equatable {
    /// the kind never leaves its device (docs/DATA-MAP.md rows whose sync column says "none")
    case localOnly(String)
    /// the row carries something that looks like a credential
    case credential(String)
    case tampered
}

/// Which records travel. STATE rows whose DATA-MAP sync column names the encrypted sync; layout, folder cursors,
/// tracking events and tracker rules stay on their device.
public enum SyncedKinds {
    public static let assetKinds: Set<String> = ["account_icon_file"]
    public static let synced: Set<String> = [
        "account", "account_icon_file", "setting", "split_tab", "vip", "template", "rule", "note",
        "voice_profile", "decision_model", "reminder",
    ]

    public static func isSynced(_ kind: String) -> Bool { synced.contains(kind) }
    public static func isAsset(_ kind: String) -> Bool { assetKinds.contains(kind) }
}
