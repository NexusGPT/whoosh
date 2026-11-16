import Foundation
import UserNotifications

/// The whole extension behaviour, free of the `UNNotificationServiceExtension` subclass so a test can drive it.
///  - signal push: the phone fetches the message itself, then shows sender, subject and account;
///  - Graph rich push: decrypt on the device; a decrypt failure falls back to a fetch, then to a generic line;
///  - every path ends in exactly one call to the completion handler.
public final class ServiceRunner: @unchecked Sendable {
    public static let genericTitle = "New mail"

    private let lookup: MailLookup
    private let keys: RichKeyUnwrapping
    private let lock = NSLock()
    private var gate: CompletionGate?
    private var bestAttempt: UNMutableNotificationContent?
    private var task: Task<Void, Never>?

    public init(lookup: MailLookup, keys: RichKeyUnwrapping) { self.lookup = lookup; self.keys = keys }

    public func didReceive(_ request: UNNotificationRequest, contentHandler: @escaping (UNNotificationContent) -> Void) {
        let gate = CompletionGate(contentHandler)
        let content = (request.content.mutableCopy() as? UNMutableNotificationContent) ?? UNMutableNotificationContent()
        if content.title.isEmpty { content.title = Self.genericTitle }
        content.categoryIdentifier = NotificationIDs.category
        lock.lock(); self.gate = gate; self.bestAttempt = content; lock.unlock()

        nonisolated(unsafe) let userInfo = request.content.userInfo
        nonisolated(unsafe) let box = content
        let started = Task { [lookup, keys] in
            let resolved = await Self.resolve(userInfo: userInfo, lookup: lookup, keys: keys)
            if let resolved { Self.apply(resolved, to: box) }
            gate.finish(box)
        }
        lock.lock(); task = started; lock.unlock()
    }

    /// The system's 30-second warning: deliver what we have now.
    public func serviceExtensionTimeWillExpire() {
        lock.lock(); let gate = self.gate, best = self.bestAttempt, running = task; lock.unlock()
        running?.cancel()
        if let gate, let best { gate.finish(best) }
    }

    // MARK: resolution (never throws: every failure becomes `nil`, which keeps the generic content)

    struct Resolved: Sendable { let sender: String; let subject: String; let accountName: String? }

    static func resolve(userInfo: [AnyHashable: Any], lookup: MailLookup, keys: RichKeyUnwrapping) async -> Resolved? {
        guard let payload = try? PushPayload.parse(userInfo: userInfo) else { return nil }
        if case .graphRich(let encrypted) = payload.kind,
           let message = try? GraphRichDecryptor.message(from: encrypted, keys: keys) {
            let account = await lookup.accountName(accountRoutingID: payload.accountRoutingID)
            return Resolved(sender: message.sender, subject: message.subject, accountName: account)
        }
        // A signal, or a rich payload that would not decrypt: the phone fetches the message itself.
        guard let fetched = try? await lookup.fetchMessage(accountRoutingID: payload.accountRoutingID, messageID: payload.messageID)
        else { return nil }
        return Resolved(sender: fetched.sender, subject: fetched.subject, accountName: fetched.accountName)
    }

    static func apply(_ resolved: Resolved, to content: UNMutableNotificationContent) {
        content.title = resolved.sender
        content.body = resolved.subject
        content.subtitle = resolved.accountName ?? ""
    }
}
