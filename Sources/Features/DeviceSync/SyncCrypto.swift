import Foundation
import CryptoKit

/// Where the sync key lives. In the product this is an iCloud Keychain item shared by the owner's devices; the
/// container never sees it. Tests use the in-memory stand-in.
public protocol SyncKeyProvider {
    func key() throws -> SymmetricKey
}

public final class InMemoryKeychain: SyncKeyProvider {
    private let stored: SymmetricKey

    public init() { stored = SymmetricKey(size: .bits256) }

    public func key() throws -> SymmetricKey { stored }
}

/// End-to-end encryption of one record: AES-GCM, the record's kind and id bound in as authenticated data so a
/// sealed blob cannot be moved onto another record.
public enum SyncCrypto {
    static func aad(kind: String, id: String) -> Data { Data("\(kind)|\(id)".utf8) }

    public static func seal(_ plain: Data, kind: String, id: String, key: SymmetricKey) throws -> Data {
        let box = try AES.GCM.seal(plain, using: key, authenticating: aad(kind: kind, id: id))
        guard let combined = box.combined else { throw SyncRefusal.tampered }
        return combined
    }

    public static func open(_ sealed: Data, kind: String, id: String, key: SymmetricKey) throws -> Data {
        do {
            let box = try AES.GCM.SealedBox(combined: sealed)
            return try AES.GCM.open(box, using: key, authenticating: aad(kind: kind, id: id))
        } catch {
            throw SyncRefusal.tampered
        }
    }
}

enum SealedBody {
    private struct Wire: Codable {
        var fields: [String: String]
        var device: String
    }

    static func encode(fields: [String: String], device: String) throws -> Data {
        try JSONEncoder().encode(Wire(fields: fields, device: device))
    }

    static func decode(_ data: Data) throws -> (fields: [String: String], device: String) {
        let wire = try JSONDecoder().decode(Wire.self, from: data)
        return (wire.fields, wire.device)
    }
}
