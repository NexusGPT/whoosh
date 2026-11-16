import Foundation

/// What the phone's own core knows. The extension reads it from the shared app-group store; it never asks a server.
public struct FetchedMessage: Equatable, Sendable {
    public let sender: String
    public let subject: String
    public let accountName: String
    public init(sender: String, subject: String, accountName: String) {
        self.sender = sender; self.subject = subject; self.accountName = accountName
    }
}

public protocol MailLookup: Sendable {
    /// The phone fetches the message itself from the mail server, for a signal push.
    func fetchMessage(accountRoutingID: String, messageID: String) async throws -> FetchedMessage
    /// Display name of the account (a Graph rich push already carries sender and subject).
    func accountName(accountRoutingID: String) async -> String?
}
