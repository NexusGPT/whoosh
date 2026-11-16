import Foundation
import CryptoKit

public struct PairingToken: Equatable, Sendable {
    public let value: String
    public let expiresAtMs: Int64
    /// what the QR code encodes: a pairing token only, never an account or a secret
    public var qrPayload: String { "whoosh-pair:v1:\(value)" }
}

/// what a good scan returns: the pairing happened; the sync key itself travels through iCloud Keychain, not here
public struct PairingGrant: Equatable, Sendable {
    public init() {}
}

public enum PairingError: Error, Equatable, Sendable {
    case unknown
    case expired
    case alreadyUsed
}

/// The QR pairing token: random, single use, expiring. Only a hash is kept, so a memory dump holds no usable token.
/// Issuing a new token replaces the live one.
public final class PairingService: @unchecked Sendable {
    private let expirySeconds: Int
    private let clock: () -> Int64
    private let random: () -> Data
    private let lock = NSLock()
    private var live: (hash: String, expiresAtMs: Int64)?
    private var used: Set<String> = []

    public init(expirySeconds: Int, clock: @escaping () -> Int64,
                random: @escaping () -> Data = { Data((0..<32).map { _ in UInt8.random(in: 0...255) }) }) {
        self.expirySeconds = expirySeconds
        self.clock = clock
        self.random = random
    }

    private static func hash(_ value: String) -> String {
        SHA256.hash(data: Data(value.utf8)).map { String(format: "%02x", $0) }.joined()
    }

    public func issue() -> PairingToken {
        let value = random().map { String(format: "%02x", $0) }.joined()
        let expires = clock() + Int64(expirySeconds) * 1000
        lock.lock()
        live = (Self.hash(value), expires)
        lock.unlock()
        return PairingToken(value: value, expiresAtMs: expires)
    }

    /// Accept the token once. The first scan consumes it, so a second scan is refused.
    public func redeem(_ value: String) -> Result<PairingGrant, PairingError> {
        let hash = Self.hash(value)
        lock.lock(); defer { lock.unlock() }
        if used.contains(hash) { return .failure(.alreadyUsed) }
        guard let current = live, current.hash == hash else { return .failure(.unknown) }
        live = nil
        used.insert(hash)
        if clock() >= current.expiresAtMs { return .failure(.expired) }
        return .success(PairingGrant())
    }

    public var hasLiveToken: Bool {
        lock.lock(); defer { lock.unlock() }
        guard let current = live else { return false }
        return clock() < current.expiresAtMs
    }
}
