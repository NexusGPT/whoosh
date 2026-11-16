import Foundation

/// The reference the database keeps: the Keychain service and account, the same two strings as the core's KeychainRef.
public struct VaultReference: Sendable, Equatable, Hashable {
    public let service: String
    public let account: String
    public init(service: String, account: String) { self.service = service; self.account = account }
}

public enum VaultKind: String, Sendable {
    /// an app password or sign-in token for one mail account (core-secrets SecretKind::AccountCredential)
    case accountCredential = "account-credential"
    /// an AI provider key or sign-in token (SecretKind::ProviderCredential)
    case providerCredential = "provider-credential"
}

/// The seam to core-secrets' StoreSecret / ReadSecret / DeleteSecret ports. Tokens go in here and nowhere else:
/// not the database, not a file, not the log.
public protocol TokenVault: Sendable {
    func store(kind: VaultKind, name: String, secret: Data) throws -> VaultReference
    func read(_ reference: VaultReference) throws -> Data
    func delete(_ reference: VaultReference) throws
}

/// What is stored under one reference: the token set as JSON.
public struct TokenSet: Codable, Sendable, Equatable {
    public var accessToken: String
    public var refreshToken: String?
    public var expiresAtMs: Int64?
    public var scope: String?
    public var tokenType: String?
    public init(accessToken: String, refreshToken: String?, expiresAtMs: Int64?, scope: String?, tokenType: String?) {
        self.accessToken = accessToken; self.refreshToken = refreshToken; self.expiresAtMs = expiresAtMs
        self.scope = scope; self.tokenType = tokenType
    }
}

/// A vault for tests and previews: process memory only.
public final class MemoryVault: TokenVault, @unchecked Sendable {
    private let lock = NSLock()
    private var items: [VaultReference: Data] = [:]
    public let prefix: String
    public init(prefix: String = "io.nexus.whoosh.rig.signin-test.") { self.prefix = prefix }
    public func store(kind: VaultKind, name: String, secret: Data) throws -> VaultReference {
        let ref = VaultReference(service: prefix + kind.rawValue, account: name)
        lock.withLock { items[ref] = secret }
        return ref
    }
    public func read(_ reference: VaultReference) throws -> Data {
        guard let d = lock.withLock({ items[reference] }) else { throw SignInError.words("That sign-in is no longer saved.") }
        return d
    }
    public func delete(_ reference: VaultReference) throws { lock.withLock { items[reference] = nil } }
    public var count: Int { lock.withLock { items.count } }
}
