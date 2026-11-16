import Foundation

/// No credential in any synced record. Secrets travel through iCloud Keychain, never the record store: a row may
/// carry a Keychain REFERENCE (service and account names) and nothing that opens the account.
public enum CredentialGuard {
    static let forbiddenKeyParts = [
        "password", "passwd", "secret", "token", "api_key", "apikey", "refresh", "authorization", "credential",
    ]
    static let allowedKeys: Set<String> = ["keychain_service", "keychain_account"]
    static let forbiddenValueMarkers = ["-----BEGIN", "Bearer ", "ya29.", "xoxb-", "sk-ant-"]

    /// the field name that gives the row away, or nil when the row is clean
    public static func offence(in fields: [String: String]) -> String? {
        for (key, value) in fields.sorted(by: { $0.key < $1.key }) {
            let lower = key.lowercased()
            if !allowedKeys.contains(lower), forbiddenKeyParts.contains(where: { lower.contains($0) }) {
                return key
            }
            if forbiddenValueMarkers.contains(where: { value.contains($0) }) { return key }
        }
        return nil
    }

    /// the same scan over bytes a container holds (the "field scan"): finds any marker or any given secret
    public static func scan(_ data: Data, secrets: [String]) -> String? {
        let text = String(decoding: data, as: UTF8.self)
        let lower = text.lowercased()
        for part in forbiddenKeyParts where lower.contains(part) { return part }
        for marker in forbiddenValueMarkers where text.contains(marker) { return marker }
        for secret in secrets where !secret.isEmpty && text.contains(secret) { return "a known secret" }
        return nil
    }
}
