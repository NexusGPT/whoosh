import Foundation
import Security

/// INTERIM adapter for the vault seam: writes the item the core-secrets crate would write (generic password, service
/// `io.nexus.whoosh.<kind>`, account = name, iCloud-synchronizable) straight through the Security framework, because
/// the FFI exposes no StoreSecret command yet. Replace with a CoreBridge call when the contract adds it
/// (docs/lanes/signin.md "## Contract deltas", item 1). Tokens never go anywhere else.
public struct KeychainVault: TokenVault {
    public let prefix: String
    public init(prefix: String = "io.nexus.whoosh.") { self.prefix = prefix }

    private func query(_ r: VaultReference) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: r.service,
         kSecAttrAccount as String: r.account, kSecAttrSynchronizable as String: kCFBooleanTrue as Any]
    }

    public func store(kind: VaultKind, name: String, secret: Data) throws -> VaultReference {
        let ref = VaultReference(service: prefix + kind.rawValue, account: name)
        SecItemDelete(query(ref) as CFDictionary)
        var add = query(ref)
        add[kSecValueData as String] = secret
        add[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlock
        let st = SecItemAdd(add as CFDictionary, nil)
        guard st == errSecSuccess else { throw SignInError.words("The sign-in could not be saved to the Keychain (code \(st)).") }
        return ref
    }

    public func read(_ reference: VaultReference) throws -> Data {
        var q = query(reference)
        q[kSecReturnData as String] = true
        var out: CFTypeRef?
        guard SecItemCopyMatching(q as CFDictionary, &out) == errSecSuccess, let d = out as? Data else {
            throw SignInError.words("That sign-in is no longer saved.")
        }
        return d
    }

    public func delete(_ reference: VaultReference) throws {
        let st = SecItemDelete(query(reference) as CFDictionary)
        guard st == errSecSuccess || st == errSecItemNotFound else { throw SignInError.words("The saved sign-in could not be removed (code \(st)).") }
    }
}
