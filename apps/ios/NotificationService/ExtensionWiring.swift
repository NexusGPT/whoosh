import Foundation

/// The Graph rich-notification private key from the shared Keychain group (the app creates it at account setup).
enum ExtensionKeys {
    static let keyTag = "io.nexus.whoosh.graph-rich-key"

    static func load() -> RichKeyUnwrapping {
        SecKeyRichKeyUnwrapper.fromKeychain(tag: keyTag, accessGroup: nil) ?? MissingKey()
    }
    private struct MissingKey: RichKeyUnwrapping {
        func unwrap(_ wrappedKey: Data) throws -> Data { throw RichDecryptError.keyUnwrapFailed }
    }
}

/// Reads the phone's own core through the shared app group. Until the core exposes a read-only message
/// fetch to the extension (docs/lanes/notificationservice.md, Contract deltas), it reports "not available",
/// which keeps the generic alert: the extension still completes.
struct AppGroupMailLookup: MailLookup {
    struct Unavailable: Error {}
    func fetchMessage(accountRoutingID: String, messageID: String) async throws -> FetchedMessage { throw Unavailable() }
    func accountName(accountRoutingID: String) async -> String? { nil }
}
