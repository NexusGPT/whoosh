import Foundation
import Testing
import UserNotifications
@testable import WhooshUI

// Lane fix-rest-app, point 4 (item 7b25ddc2, docs/lanes/gap-phone-wire.md finding 4): the notification carries the mail's id and the tap reaches the phone router.
// NOTHING here asks the system for notification permission or touches `UNUserNotificationCenter.current()`: the content is built as a plain value, the tap is
// made by calling the pure entry the delegate calls (`NotificationTap.handle`), and the only poster a test uses is the test engine's. Each test names its broken copy.
// Run with WHOOSH_FIXTURE_CORE=1.

private let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
@MainActor private func appModel() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    setenv("WHOOSH_FIXTURE_PATH", root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    PhoneRouter.shared.send(.reset)
    return m
}

@Test func theNoticeCarriesTheIdOfTheMailItIsAbout() {
    let notice = LocalNotification(id: "msg-42", accountId: "acc-1", title: "Linea Supply", body: "Quote", level: .active)
    let content = SystemNotificationPoster.content(for: notice)
    #expect(NotificationTap.messageId(from: content.userInfo) == "msg-42")          // mutant: `content.userInfo = ...` removed from SystemNotificationPoster.content
    #expect(content.title == "Linea Supply" && content.body == "Quote" && content.threadIdentifier == "acc-1", "the rest of the notice is as it was")
    let quiet = SystemNotificationPoster.content(for: LocalNotification(id: "msg-43", accountId: "acc-1", title: "t", body: "b", level: .passive))
    #expect(NotificationTap.messageId(from: quiet.userInfo) == "msg-43" && quiet.sound == nil, "a passive notice carries it too, and stays silent")
}

@Test func aNoticeThatNamesNoMailOrAnEmptyIdOpensNothing() {
    #expect(NotificationTap.messageId(from: [:]) == nil)
    #expect(NotificationTap.messageId(from: [NotificationTap.messageKey: ""]) == nil)       // mutant: the `!id.isEmpty` guard removed
    #expect(NotificationTap.messageId(from: [NotificationTap.messageKey: 7]) == nil)
    #expect(NotificationTap.messageId(from: ["messageId": "msg-1"]) == nil, "another key is not ours")
}

@MainActor @Test func aTapOnTheNoticeOpensThatMailsThreadOverTheInbox() throws {
    let m = try appModel()
    defer { ThreadPhoneState.shared.reset(); m.overlay = nil; PhoneRouter.shared.send(.reset) }
    let id = try #require(m.rows.dropFirst().first?.messageId, "the fixture inbox has rows")
    let info = NotificationTap.userInfo(messageId: id)
    #expect(NotificationTap.handle(actionIdentifier: UNNotificationDefaultActionIdentifier, userInfo: info))   // mutant: handle() returns true without calling the router
    #expect(PhoneRouter.shared.nav.path == [.inbox, .thread(id)])
    #expect(m.overlay == "iphone-thread")
    #expect(ThreadPhoneState.shared.target(m) == id, "the thread screen is on the notified mail")
}

@MainActor @Test func theLongPressActionsAndADismissOpenNothing() throws {
    let m = try appModel()
    let id = try #require(m.rows.first?.messageId)
    let info = NotificationTap.userInfo(messageId: id)
    for action in ["archive", "reply", UNNotificationDismissActionIdentifier] {
        #expect(!NotificationTap.handle(actionIdentifier: action, userInfo: info), "\(action) never opens the app's thread")   // mutant: the actionIdentifier guard removed
    }
    #expect(PhoneRouter.shared.nav.path == PhoneNav().path && m.overlay == nil, "nothing opened: the stack is still the launch inbox")   // PhoneNav.path starts as [.inbox] (Sources/App/PhoneRoutes.swift:40), never empty
}

@MainActor @Test func aNoticeForAMailThatIsGoneOrUnnamedOpensNothingAndSaysNo() throws {
    let m = try appModel()
    #expect(!NotificationTap.handle(actionIdentifier: UNNotificationDefaultActionIdentifier, userInfo: NotificationTap.userInfo(messageId: "msg-that-is-gone")))
    #expect(!NotificationTap.handle(actionIdentifier: UNNotificationDefaultActionIdentifier, userInfo: [:]))
    #expect(PhoneRouter.shared.nav.path == PhoneNav().path && m.overlay == nil, "nothing opened: the stack is still the launch inbox")   // PhoneNav.path starts as [.inbox] (Sources/App/PhoneRoutes.swift:40), never empty
}

@Test func theNotificationDelegateIsTheOneTheAppInstallsAndItAsksForNothing() throws {
    // The delegate is a plain object: building it touches no notification centre (this test never reads `UNUserNotificationCenter.current()`).
    #expect(NotificationTapDelegate.shared === NotificationTapDelegate.shared)
    let source = try String(contentsOf: root.appendingPathComponent("apps/ios/App/WhooshPhoneApp.swift"), encoding: .utf8)
    #expect(source.contains("UNUserNotificationCenter.current().delegate = NotificationTapDelegate.shared"), "the app sets the delegate at launch")    // mutant: the line removed from the app delegate
    #expect(!source.contains("requestAuthorization"), "the app file never asks for permission")
    for file in ["Sources/Features/PushClient/SystemPorts.swift"] {
        let text = try String(contentsOf: root.appendingPathComponent(file), encoding: .utf8)
        let tap = try #require(text.range(of: "public enum NotificationTap"), "the tap type is in \(file)")
        #expect(!text[tap.lowerBound...].contains("requestAuthorization"), "the tap and its delegate never ask for permission")
    }
}
