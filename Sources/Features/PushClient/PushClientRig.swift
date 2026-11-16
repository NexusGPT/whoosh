import Foundation
import QuartzCore

/// The test copy's notification side (the phone rig; the owner's copy posts through `SystemNotificationPoster` as before and never reaches this file).
/// A simulator's lock screen cannot be read, so the posts the router decides on are RECORDED and served as the rows `iphone-lock.notifications`
/// (J7.1): the sender, the subject and the account of each notice, in the order they were posted. Nothing is posted to the system from here.
public final class RecordingNotificationPoster: NotificationPoster, @unchecked Sendable {
    public static let shared = RecordingNotificationPoster()
    public struct Post: Sendable, Equatable {
        public var notification: LocalNotification
        public var atMs: Double
    }
    private let lock = NSLock()
    private var posts: [Post] = []

    public func post(_ notification: LocalNotification) {
        lock.lock(); defer { lock.unlock() }
        posts.append(Post(notification: notification, atMs: CACurrentMediaTime() * 1000))
    }

    public var all: [Post] { lock.lock(); defer { lock.unlock() }; return posts }
    public func clear() { lock.lock(); posts = []; lock.unlock() }
}

/// What the router asks of the notification settings in a test copy: every account is on, Focus is not followed.
struct RigNotificationSettings: NotificationSettings {
    func isEnabled(accountId: String) -> Bool { true }
    func followsFocus() -> Bool { false }
}

struct RigFocus: FocusStatus {
    func isFocused() -> Bool { false }
}

/// The arrival's facts from the core's message (the fixture core answers `message(messageId:)`).
struct RigMessageLookup: MessageLookup {
    let core: WhooshCore
    func arrival(messageId: String) -> ArrivalInfo? {
        guard let m = core.message(messageId: messageId) else { return nil }
        return ArrivalInfo(messageId: messageId, accountId: m.accountId, senderName: m.senderName, subject: m.subject, loginCode: m.loginCode)
    }
}

/// A seen-store `fixture reset` can empty, so the same arrival delivered again (the dark pass of a journey) is decided again.
final class RigSeenStore: SeenStore, @unchecked Sendable {
    private let lock = NSLock()
    private var ids = Set<String>()
    func markSeen(_ messageId: String) -> Bool { lock.lock(); defer { lock.unlock() }; return ids.insert(messageId).inserted }
    func clear() { lock.lock(); ids = []; lock.unlock() }
}

@MainActor
public enum PushClientRig {
    private static var started: (router: ArrivalRouter, listener: ArrivalListener, subscription: UInt64)?
    private static var resetToken: NSObjectProtocol?

    /// The phone's test copy only (`CoreHost.shared.kind == .fixture`): the arrival router on the inbox stream with the recording poster.
    /// Called once from the phone's router when it is first used; a second call, or any other copy, does nothing.
    public static func startForPhone() {
        guard started == nil, CoreHost.shared.kind == .fixture else { return }
        let core = CoreHost.shared.core
        let seen = RigSeenStore()
        let router = ArrivalRouter(lookup: RigMessageLookup(core: core), settings: RigNotificationSettings(), focus: RigFocus(),
                                   poster: RecordingNotificationPoster.shared, seen: seen)
        let listener = ArrivalListener(router: router)
        started = (router, listener, core.subscribe(stream: .inbox, sink: listener))
        resetToken = NotificationCenter.default.addObserver(forName: FixtureEvents.name("reset"), object: nil, queue: .main) { _ in
            MainActor.assumeIsolated { RecordingNotificationPoster.shared.clear(); seen.clear() }
        }
    }

    /// `iphone-lock.notifications`: one row per recorded notice. The system's long-press actions (Archive, Reply) are not registered anywhere yet, so
    /// `iphone-lock.notifications.actions` has no rows and its case stays not judged.
    static func notificationRows(_ m: AppModel) -> [ControlRow] {
        RecordingNotificationPoster.shared.all.enumerated().map { i, p in
            let row: ControlRow = ["ax": "iphone-lock.notifications.\(i)", "id": p.notification.id, "sender": p.notification.title, "subject": p.notification.body,
                                   "account": PhoneAccounts.of(p.notification.accountId, in: m).name, "passive": p.notification.level == .passive]
            return row
        }
    }
}
