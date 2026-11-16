import Foundation

// The Mac side of notifications (fn-pushclient-mac). It starts from Apple's UserNotifications
// (UNUserNotificationCenter for the post, Intents' INFocusStatusCenter for Focus); nothing here is written from
// scratch that the system already does. Every outside thing sits behind a small port so a test drives the whole
// decision with fakes, and no test ever posts to the owner's notification centre.

/// What an arrival shows. Built from the core's `ArrivalNotice{message_id}` plus the message record.
public struct ArrivalInfo: Sendable, Equatable {
    public var messageId: String
    public var accountId: String
    public var senderName: String
    public var subject: String
    public var loginCode: String?
    public init(messageId: String, accountId: String, senderName: String, subject: String, loginCode: String? = nil) {
        self.messageId = messageId; self.accountId = accountId; self.senderName = senderName
        self.subject = subject; self.loginCode = loginCode
    }
}

/// `passive` is delivered to the notification centre without a banner or sound (what Focus asks for).
public enum NotificationLevel: Sendable, Equatable { case active, passive }

public struct LocalNotification: Sendable, Equatable {
    public var id: String          // the message id: the system replaces a notification with the same id, a second guard
    public var accountId: String
    public var title: String
    public var body: String
    public var level: NotificationLevel
}

/// What the notch lane consumes: one value per new message that may drop. Muted accounts never produce one.
public struct NotchDrop: Sendable, Equatable {
    public var messageId: String
    public var accountId: String
    public var senderName: String
    public var subject: String
    public var loginCode: String?
}

public protocol MessageLookup: Sendable { func arrival(messageId: String) -> ArrivalInfo? }
public protocol NotificationPoster: Sendable { func post(_ notification: LocalNotification) }
public protocol FocusStatus: Sendable { func isFocused() -> Bool }

/// The two settings the decision reads (docs/DATA-MAP.md: notification-setting per account; notch-setting follow_focus).
public protocol NotificationSettings: Sendable {
    func isEnabled(accountId: String) -> Bool
    func followsFocus() -> Bool
}

/// Which message ids were already decided. In memory by default; a persistent store can replace it.
public protocol SeenStore: Sendable {
    /// true when the id is new (and is now remembered), false when it was seen before
    func markSeen(_ messageId: String) -> Bool
}

public final class MemorySeenStore: SeenStore, @unchecked Sendable {
    private let lock = NSLock()
    private var ids = Set<String>()
    public init() {}
    public func markSeen(_ messageId: String) -> Bool {
        lock.lock(); defer { lock.unlock() }
        return ids.insert(messageId).inserted
    }
}

/// The notch feed. The notch lane subscribes once; every unmuted, unduplicated arrival is emitted to every subscriber.
public final class NotchFeed: @unchecked Sendable {
    private let lock = NSLock()
    private var handlers: [UInt64: @Sendable (NotchDrop) -> Void] = [:]
    private var next: UInt64 = 1
    public init() {}

    @discardableResult
    public func subscribe(_ handler: @escaping @Sendable (NotchDrop) -> Void) -> UInt64 {
        lock.lock(); defer { lock.unlock() }
        let token = next; next += 1; handlers[token] = handler
        return token
    }
    public func unsubscribe(_ token: UInt64) {
        lock.lock(); handlers[token] = nil; lock.unlock()
    }
    /// The same feed as a stream, for a consumer written with async/await.
    public func drops() -> AsyncStream<NotchDrop> {
        AsyncStream { continuation in
            let token = subscribe { continuation.yield($0) }
            continuation.onTermination = { [weak self] _ in self?.unsubscribe(token) }
        }
    }
    func emit(_ drop: NotchDrop) {
        lock.lock(); let all = Array(handlers.values); lock.unlock()
        for handler in all { handler(drop) }
    }
}
