import Foundation
import UserNotifications

/// The core's side of a notification action (the app target adapts the generated core API to this).
public protocol ActionCore: Sendable {
    func archive(accountRoutingID: String, messageID: String) async throws
    func reply(accountRoutingID: String, messageID: String, text: String) async throws
}

public enum ActionOutcome: Equatable, Sendable {
    case archived, replied, openMessage(messageID: String), ignored, failed(String)
}

/// Runs inside `userNotificationCenter(_:didReceive:withCompletionHandler:)` in the app, woken in the background.
/// The completion handler is called exactly once on every path, including a core failure.
public struct NotificationActionRouter: Sendable {
    private let core: ActionCore
    public init(core: ActionCore) { self.core = core }

    public func handle(actionIdentifier: String, userInfo: [AnyHashable: Any], textInput: String?) async -> ActionOutcome {
        guard let payload = try? PushPayload.parse(userInfo: userInfo) else { return .ignored }
        do {
            switch actionIdentifier {
            case NotificationIDs.archive:
                try await core.archive(accountRoutingID: payload.accountRoutingID, messageID: payload.messageID)
                return .archived
            case NotificationIDs.reply:
                guard let text = textInput?.trimmingCharacters(in: .whitespacesAndNewlines), !text.isEmpty else { return .ignored }
                try await core.reply(accountRoutingID: payload.accountRoutingID, messageID: payload.messageID, text: text)
                return .replied
            case UNNotificationDefaultActionIdentifier:
                return .openMessage(messageID: payload.messageID)
            default:
                return .ignored
            }
        } catch {
            return .failed(String(describing: error))
        }
    }

    /// Delegate-shaped entry: always finishes, whatever the outcome.
    public func handle(actionIdentifier: String, userInfo: [AnyHashable: Any], textInput: String?,
                       completion: @escaping @Sendable (ActionOutcome) -> Void) {
        nonisolated(unsafe) let info = userInfo
        Task {
            let outcome = await handle(actionIdentifier: actionIdentifier, userInfo: info, textInput: textInput)
            completion(outcome)
        }
    }
}
