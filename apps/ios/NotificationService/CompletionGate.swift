import Foundation
import UserNotifications

/// Holds the system's completion handler and lets exactly one caller use it. The system shows the ORIGINAL
/// notification when the handler is never called, and may crash the extension if it is called twice.
public final class CompletionGate: @unchecked Sendable {
    private let lock = NSLock()
    private var handler: ((UNNotificationContent) -> Void)?
    public init(_ handler: @escaping (UNNotificationContent) -> Void) { self.handler = handler }

    /// True when this call delivered.
    @discardableResult
    public func finish(_ content: UNNotificationContent) -> Bool {
        lock.lock()
        let h = handler
        handler = nil
        lock.unlock()
        guard let h else { return false }
        h(content)
        return true
    }

    public var isFinished: Bool { lock.lock(); defer { lock.unlock() }; return handler == nil }
}
