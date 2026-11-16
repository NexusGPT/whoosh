import Foundation
import Security

/// The pairing token held as a Keychain item, so it never sits in a record, a file or a log. The service name carries
/// a prefix; rig and test runs use "rig." so they never touch an owner item.
public struct DeviceLinkKeychain: Sendable {
    public let service: String
    public let account: String

    public init(service: String, account: String) {
        self.service = service
        self.account = account
    }

    private var query: [String: Any] {
        [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service, kSecAttrAccount as String: account]
    }

    @discardableResult
    public func save(_ value: String) -> Bool {
        delete()
        var q = query
        q[kSecValueData as String] = Data(value.utf8)
        q[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        return SecItemAdd(q as CFDictionary, nil) == errSecSuccess
    }

    public func read() -> String? {
        var q = query
        q[kSecReturnData as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitOne
        var out: CFTypeRef?
        guard SecItemCopyMatching(q as CFDictionary, &out) == errSecSuccess, let data = out as? Data else { return nil }
        return String(decoding: data, as: UTF8.self)
    }

    @discardableResult
    public func delete() -> Bool {
        let status = SecItemDelete(query as CFDictionary)
        return status == errSecSuccess || status == errSecItemNotFound
    }
}

/// The Move-accounts link as the core serves it (docs/CAPABILITY-CONTRACT.md: device-link, settings-sync): one live
/// single-use token in the Keychain, replaced after QR_EXPIRY_S, cleared by the first scan.
public final class DeviceLinkModel: @unchecked Sendable {
    private let pairing: PairingService
    private let keychain: DeviceLinkKeychain
    private let expirySeconds: Int
    private let lock = NSLock()
    private var linkNumber = 0
    private var linkId: String?
    private var consumed = false

    public init(expirySeconds: Int, clock: @escaping () -> Int64, keychain: DeviceLinkKeychain) {
        self.expirySeconds = expirySeconds
        self.pairing = PairingService(expirySeconds: expirySeconds, clock: clock)
        self.keychain = keychain
    }

    private func ensureLocked() {
        guard !pairing.hasLiveToken else { return }
        let token = pairing.issue()
        keychain.save(token.value)
        linkNumber += 1
        linkId = "link-\(linkNumber)"
    }

    /// opening "Move accounts to iPhone": a live link, a new one when the last expired or was used
    public var id: String {
        lock.lock(); defer { lock.unlock() }
        consumed = false
        ensureLocked()
        return linkId ?? ""
    }

    public var expiresInS: Int { expirySeconds }

    /// the live token, nil once it was used
    public var token: String? {
        lock.lock(); defer { lock.unlock() }
        if consumed { return nil }
        ensureLocked()
        return keychain.read()
    }

    /// the paired device's first request: the token works once
    @discardableResult
    public func scan() -> Result<PairingGrant, PairingError> {
        lock.lock(); defer { lock.unlock() }
        guard let value = keychain.read() else { return .failure(.unknown) }
        let result = pairing.redeem(value)
        keychain.delete()
        consumed = true
        return result
    }

    /// the record kinds the settings sync carries: the accounts and every other synced STATE row, never a secret
    public var recordsCarried: String { SyncedKinds.synced.sorted().joined(separator: ",") }
}
