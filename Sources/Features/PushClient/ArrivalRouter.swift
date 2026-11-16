import Foundation

/// Decides, per arrival, whether a local notification posts and whether the notch drops.
/// Rules (fn-pushclient-mac): a muted account posts nothing and drops nothing; an unmuted one posts once per
/// message, never twice on a resync; under Focus (when the setting follows it) the notification is delivered
/// passively and the notch stays shut. A message the core cannot read yet is not remembered, so its next notice decides.
public final class ArrivalRouter: @unchecked Sendable {
    public enum Outcome: Sendable, Equatable { case posted, postedQuietly, mutedAccount, duplicate, unknownMessage }

    private let lookup: MessageLookup
    private let settings: NotificationSettings
    private let focus: FocusStatus
    private let poster: NotificationPoster
    private let seen: SeenStore
    public let notch: NotchFeed

    public init(lookup: MessageLookup, settings: NotificationSettings, focus: FocusStatus,
                poster: NotificationPoster, seen: SeenStore = MemorySeenStore(), notch: NotchFeed = NotchFeed()) {
        self.lookup = lookup; self.settings = settings; self.focus = focus
        self.poster = poster; self.seen = seen; self.notch = notch
    }

    @discardableResult
    public func handle(messageId: String) -> Outcome {
        guard let info = lookup.arrival(messageId: messageId) else { return .unknownMessage }
        // First sighting is remembered whatever the decision: unmuting later must not replay old mail.
        guard seen.markSeen(messageId) else { return .duplicate }
        guard settings.isEnabled(accountId: info.accountId) else { return .mutedAccount }
        let quiet = settings.followsFocus() && focus.isFocused()
        poster.post(LocalNotification(id: info.messageId, accountId: info.accountId, title: info.senderName,
                                      body: info.subject, level: quiet ? .passive : .active))
        if quiet { return .postedQuietly }
        notch.emit(NotchDrop(messageId: info.messageId, accountId: info.accountId, senderName: info.senderName,
                             subject: info.subject, loginCode: info.loginCode))
        return .posted
    }
}

/// Listens to the core and feeds the router. The contract has no stream kind of its own for `ArrivalNotice`;
/// it is read from the inbox stream (see docs/lanes/pushclient.md, Contract deltas).
public final class ArrivalListener: EventSink, @unchecked Sendable {
    private let router: ArrivalRouter
    public init(router: ArrivalRouter) { self.router = router }
    public func onEvent(event: CoreEvent) {
        if case .arrivalNotice(let messageId) = event { router.handle(messageId: messageId) }
    }
}
