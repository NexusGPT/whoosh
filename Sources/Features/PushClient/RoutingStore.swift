import Foundation
import Security

/// The opaque routing id per account (docs/DATA-MAP.md push-registration: a Keychain item on the iPhone).
/// The id is random and says nothing about the account; the relay learns the mapping only from the registration.
public protocol RoutingStore: Sendable {
    func routingId(for accountId: String) -> String?
    func save(_ routingId: String, for accountId: String)
    func remove(accountId: String)
}

public final class MemoryRoutingStore: RoutingStore, @unchecked Sendable {
    private let lock = NSLock()
    private var map: [String: String] = [:]
    public init() {}
    public func routingId(for accountId: String) -> String? { lock.lock(); defer { lock.unlock() }; return map[accountId] }
    public func save(_ routingId: String, for accountId: String) { lock.lock(); map[accountId] = routingId; lock.unlock() }
    public func remove(accountId: String) { lock.lock(); map[accountId] = nil; lock.unlock() }
}

public struct KeychainRoutingStore: RoutingStore {
    private let service: String
    public init(service: String = "whoosh.push.routing") { self.service = service }

    private func query(_ account: String) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service, kSecAttrAccount as String: account]
    }
    public func routingId(for accountId: String) -> String? {
        var q = query(accountId); q[kSecReturnData as String] = true; q[kSecMatchLimit as String] = kSecMatchLimitOne
        var out: CFTypeRef?
        guard SecItemCopyMatching(q as CFDictionary, &out) == errSecSuccess, let data = out as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }
    public func save(_ routingId: String, for accountId: String) {
        SecItemDelete(query(accountId) as CFDictionary)
        var q = query(accountId); q[kSecValueData as String] = Data(routingId.utf8)
        q[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlock
        SecItemAdd(q as CFDictionary, nil)
    }
    public func remove(accountId: String) { SecItemDelete(query(accountId) as CFDictionary) }
}

enum RoutingId {
    /// 16 random bytes as hex.
    static func make() -> String {
        var bytes = [UInt8](repeating: 0, count: 16)
        _ = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        return bytes.map { String(format: "%02x", $0) }.joined()
    }
}
