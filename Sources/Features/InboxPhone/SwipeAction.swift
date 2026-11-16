import Foundation

/// What a row swipe does. The owner picks the leading and the trailing action (customisable swipe buttons,
/// par-inboxphone); the defaults are the board's: swipe right archives, swipe left snoozes.
public enum SwipeAction: String, CaseIterable, Sendable {
    case archive, snooze, pin, read

    public var title: String {
        switch self { case .archive: "Archive"; case .snooze: "Snooze"; case .pin: "Pin"; case .read: "Read" }
    }
    /// The capsule's verb after the swipe.
    public var done: String {
        switch self { case .archive: "Archived"; case .snooze: "Snoozed"; case .pin: "Pinned"; case .read: "Marked read" }
    }
    public var symbol: String {
        switch self { case .archive: "archivebox"; case .snooze: "clock"; case .pin: "pin"; case .read: "envelope.open" }
    }
    /// Archive and snooze take the row out of the list; pin and read change it in place.
    public var removesRow: Bool { self == .archive || self == .snooze }

    func command(_ row: InboxRow) -> Command {
        switch self {
        case .archive: .archive(messageIds: [row.messageId])
        case .snooze: .snooze(messageIds: [row.messageId], when: .tomorrowMorning)
        case .pin: .pin(messageId: row.messageId, pinned: !row.pinned)
        case .read: .markRead(messageIds: [row.messageId], read: row.unread)
        }
    }
}

/// The owner's two swipe picks, kept on this device (a layout state, like the Mailboxes selection).
public struct SwipeSettings: Equatable, Sendable {
    public var leading: SwipeAction
    public var trailing: SwipeAction
    public static let standard = SwipeSettings(leading: .archive, trailing: .snooze)

    static let leadingKey = "inboxphone.swipe.leading"
    static let trailingKey = "inboxphone.swipe.trailing"

    static func load(_ d: UserDefaults = .standard) -> SwipeSettings {
        SwipeSettings(leading: d.string(forKey: leadingKey).flatMap(SwipeAction.init(rawValue:)) ?? standard.leading,
                      trailing: d.string(forKey: trailingKey).flatMap(SwipeAction.init(rawValue:)) ?? standard.trailing)
    }
    func save(_ d: UserDefaults = .standard) {
        d.set(leading.rawValue, forKey: Self.leadingKey)
        d.set(trailing.rawValue, forKey: Self.trailingKey)
    }
}
