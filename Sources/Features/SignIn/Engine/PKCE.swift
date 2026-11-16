import Foundation
import CryptoKit

/// RFC 7636, method S256.
public struct PKCE: Sendable, Equatable {
    public let verifier: String
    public var challenge: String { PKCE.challenge(for: verifier) }
    public init() {
        var bytes = [UInt8](repeating: 0, count: 32)
        let status = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        if status != errSecSuccess { bytes = (0..<32).map { _ in UInt8.random(in: 0...255) } }
        verifier = PKCE.base64url(Data(bytes))
    }
    public init(verifier: String) { self.verifier = verifier }
    public static func challenge(for verifier: String) -> String {
        base64url(Data(SHA256.hash(data: Data(verifier.utf8))))
    }
    static func base64url(_ d: Data) -> String {
        d.base64EncodedString().replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }
    public static func randomState() -> String { PKCE().verifier }
}
