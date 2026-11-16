import Foundation
import UserNotifications

/// Long-press actions on a mail notification. Neither opens the app: the action runs in the core in the background.
public enum NotificationIDs {
    public static let category = "io.nexus.whoosh.message"
    public static let archive = "archive"   // must equal the literal in make(); a test compares them
    public static let reply = "reply"
}

public enum NotificationCategories {
    /// The app registers this set at launch: `UNUserNotificationCenter.current().setNotificationCategories(...)`.
    public static func make() -> Set<UNNotificationCategory> {
        let archive = UNNotificationAction(identifier: "archive", title: "Archive", options: [])
        let reply = UNTextInputNotificationAction(identifier: "reply", title: "Reply", options: [],
                                                  textInputButtonTitle: "Send", textInputPlaceholder: "Reply")
        return [UNNotificationCategory(identifier: NotificationIDs.category, actions: [archive, reply],
                                       intentIdentifiers: [], options: [])]
    }
}
