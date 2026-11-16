import Foundation
import UserNotifications
import Intents

/// The real notification poster. It only works inside an app bundle: a bare test process has no notification
/// centre, so `current()` is never called there (the tests use a fake poster and never reach this type).
public final class SystemNotificationPoster: NotificationPoster, @unchecked Sendable {
    public init() {}

    public func requestAuthorization() async -> Bool {
        (try? await UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound, .badge])) ?? false
    }

    public func post(_ notification: LocalNotification) {
        let request = UNNotificationRequest(identifier: notification.id, content: Self.content(for: notification), trigger: nil)
        UNUserNotificationCenter.current().add(request) { _ in }
    }

    /// What the system shows for a notice. It carries the mail's id (`NotificationTap.messageKey`), so a tap on it opens that mail's thread.
    /// Pure: it builds a value and asks the system for nothing (a unit test reads it without a notification centre).
    static func content(for notification: LocalNotification) -> UNMutableNotificationContent {
        let content = UNMutableNotificationContent()
        content.title = notification.title
        content.body = notification.body
        content.threadIdentifier = notification.accountId
        content.userInfo = NotificationTap.userInfo(messageId: notification.id)
        content.interruptionLevel = notification.level == .passive ? .passive : .active
        if notification.level == .active { content.sound = .default }
        return content
    }
}

/// The tap on a mail notification (item 7b25ddc2, docs/lanes/gap-phone-wire.md finding 4): the notice carries the mail's id, and the app's notification
/// delegate hands it to the phone router, which opens that mail's thread over the inbox. Nothing here asks the system for permission: a delegate is set,
/// never an authorisation requested; the test engine's stand-in poster is the only thing that posts in a test.
public enum NotificationTap {
    /// The key of the mail's id in a notification's `userInfo`.
    public static let messageKey = "whoosh.message-id"

    public static func userInfo(messageId: String) -> [String: Any] { [messageKey: messageId] }

    /// The mail a notification's `userInfo` names; nil when it names none (a notice from another sender, an empty id).
    public static func messageId(from userInfo: [AnyHashable: Any]) -> String? {
        guard let id = userInfo[messageKey] as? String, !id.isEmpty else { return nil }
        return id
    }

    /// The default action (the tap on the notice itself) opens the thread; the long-press actions (archive, reply) run in the core and never open the app.
    /// False when the id is missing, the action is another one, or no mail with that id can be shown.
    @MainActor @discardableResult
    public static func handle(actionIdentifier: String, userInfo: [AnyHashable: Any]) -> Bool {
        guard actionIdentifier == UNNotificationDefaultActionIdentifier, let id = messageId(from: userInfo) else { return false }
        return PhoneRouter.shared.notificationTapped(messageId: id)
    }
}

/// The iPhone app sets this as `UNUserNotificationCenter.current().delegate` at launch (apps/ios/App/WhooshPhoneApp.swift).
public final class NotificationTapDelegate: NSObject, UNUserNotificationCenterDelegate, @unchecked Sendable {
    public static let shared = NotificationTapDelegate()

    public func userNotificationCenter(_ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse) async {
        let action = response.actionIdentifier
        // only the mail's id crosses to the main actor (a String): the notice's own dictionary is not sendable
        guard let id = NotificationTap.messageId(from: response.notification.request.content.userInfo) else { return }
        await MainActor.run { _ = NotificationTap.handle(actionIdentifier: action, userInfo: NotificationTap.userInfo(messageId: id)) }
    }
}

/// Focus through Intents. Without the user's Focus-status permission the answer is "not focused".
public struct SystemFocusStatus: FocusStatus {
    public init() {}
    public func isFocused() -> Bool { INFocusStatusCenter.default.focusStatus.isFocused ?? false }
}

/// Reads the settings from the core's state probe (`notification_setting/<acc>/enabled`, `notch_setting/follow_focus`).
/// An absent value reads as the default: notifications on, Focus followed.
public struct CoreStateSettings: NotificationSettings {
    private let core: WhooshCore
    public init(core: WhooshCore) { self.core = core }
    public func isEnabled(accountId: String) -> Bool { flag("notification_setting/\(accountId)/enabled") }
    public func followsFocus() -> Bool { flag("notch_setting/follow_focus") }
    private func flag(_ path: String) -> Bool { core.state(path: path).map { $0.trimmingCharacters(in: .whitespaces) != "false" } ?? true }
}

/// Reads the message fields the notice needs from the core's state probe. Assumed paths: message/<id>/{account_id,sender_name,subject,login_code}.
public struct CoreMessageLookup: MessageLookup {
    private let core: WhooshCore
    public init(core: WhooshCore) { self.core = core }
    public func arrival(messageId: String) -> ArrivalInfo? {
        // The message's row, read from the local database: the real engine serves no `message/<id>/account_id|subject|sender_name` state path (docs/STATE-PATHS.json
        // lists none), so reading those alone answered nil for every real arrival and neither the notification nor the notch ever came (main100 to main105: the arrival
        // was in the list, the notch held 0 items). The state paths stay as the fallback for a core that serves them (and for the login code, which the row does not carry).
        if let row = core.rowsForMessages(messageIds: [messageId]).first {
            return ArrivalInfo(messageId: messageId, accountId: row.accountId, senderName: row.senderName, subject: row.subject, loginCode: string("message/\(messageId)/login_code"))
        }
        guard let account = string("message/\(messageId)/account_id"), let subject = string("message/\(messageId)/subject") else { return nil }
        return ArrivalInfo(messageId: messageId, accountId: account, senderName: string("message/\(messageId)/sender_name") ?? "",
                           subject: subject, loginCode: string("message/\(messageId)/login_code"))
    }
    private func string(_ path: String) -> String? {
        guard let raw = core.state(path: path), let data = raw.data(using: .utf8),
              let value = try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed]) as? String else { return nil }
        return value
    }
}

/// The Mac wiring: one call from the app shell. Returns the router (its `notch` is the feed) and keeps the subscription.
public enum PushClientMac {
    public static func start(core: WhooshCore, poster: NotificationPoster = SystemNotificationPoster(),
                             focus: FocusStatus = SystemFocusStatus()) -> (router: ArrivalRouter, listener: ArrivalListener, subscription: UInt64) {
        let router = ArrivalRouter(lookup: CoreMessageLookup(core: core), settings: CoreStateSettings(core: core), focus: focus, poster: poster)
        let listener = ArrivalListener(router: router)
        return (router, listener, core.subscribe(stream: .inbox, sink: listener))
    }
}
