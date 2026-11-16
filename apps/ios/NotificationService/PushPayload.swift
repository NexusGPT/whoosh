import Foundation

/// What the relay puts in the APNs `whoosh` dictionary. The alert itself is generic: a push carries a SIGNAL
/// (which account, which message), never the sender, subject or body. The phone fetches the message itself.
/// Graph rich pushes add the provider's encrypted resource data, which only this device can decrypt.
public struct PushPayload: Equatable, Sendable {
    public enum Kind: Equatable, Sendable { case signal, graphRich(GraphEncryptedContent) }

    public let accountRoutingID: String
    public let messageID: String
    public let kind: Kind

    public static let userInfoKey = "whoosh"

    public enum ParseError: Error, Equatable { case missingDictionary, missingField(String), unsupportedVersion(Int), malformedRichContent }

    /// Parses the remote-notification userInfo. Never trusts types: every field is checked.
    public static func parse(userInfo: [AnyHashable: Any]) throws -> PushPayload {
        guard let body = userInfo[userInfoKey] as? [String: Any] else { throw ParseError.missingDictionary }
        let version = (body["v"] as? Int) ?? 1
        guard version == 1 else { throw ParseError.unsupportedVersion(version) }
        guard let account = body["account"] as? String, !account.isEmpty else { throw ParseError.missingField("account") }
        guard let message = body["message"] as? String, !message.isEmpty else { throw ParseError.missingField("message") }
        if (body["kind"] as? String) == "graph-rich" {
            guard let rich = body["rich"] as? [String: Any],
                  let data = rich["data"] as? String, let key = rich["dataKey"] as? String,
                  let signature = rich["dataSignature"] as? String
            else { throw ParseError.malformedRichContent }
            let content = GraphEncryptedContent(data: data, dataKey: key, dataSignature: signature,
                                                certificateID: rich["encryptionCertificateId"] as? String)
            return PushPayload(accountRoutingID: account, messageID: message, kind: .graphRich(content))
        }
        return PushPayload(accountRoutingID: account, messageID: message, kind: .signal)
    }
}
