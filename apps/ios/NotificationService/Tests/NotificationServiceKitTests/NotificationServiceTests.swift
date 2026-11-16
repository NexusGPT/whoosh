import Testing
import Foundation
import UserNotifications
@testable import NotificationServiceKit

@Suite("Notification Service Extension")
struct NotificationServiceTests {
    let keys = Fixtures.makeKeyPair()

    func runner(_ mode: FakeLookup.Mode, key: SecKey? = nil) -> ServiceRunner {
        ServiceRunner(lookup: FakeLookup(mode: mode), keys: SecKeyRichKeyUnwrapper(privateKey: key ?? keys.priv))
    }
    let fetched = FetchedMessage(sender: "Grace Hopper", subject: "Compiler notes", accountName: "Work")

    // MARK: the alert carries only a signal; the phone fetches the message itself

    @Test func signalPayloadCarriesNoMailContent() throws {
        let info = Fixtures.signalUserInfo()
        let payload = try PushPayload.parse(userInfo: info)
        #expect(payload.kind == .signal)
        let whoosh = info["whoosh"] as! [String: Any]
        #expect(Set(whoosh.keys) == ["v", "account", "message", "kind"])      // no sender, subject or body field
        let aps = info["aps"] as! [String: Any]
        #expect(((aps["alert"] as! [String: Any])["title"] as? String) == "New mail")
    }

    @Test func signalPushIsFetchedOnTheDevice() async {
        let d = Delivery()
        runner(.ok(fetched)).didReceive(Fixtures.request(Fixtures.signalUserInfo()), contentHandler: d.handler())
        #expect(await d.waitForFirst())
        #expect(d.count == 1)
        #expect(d.first?.title == "Grace Hopper")
        #expect(d.first?.body == "Compiler notes")
        #expect(d.first?.subtitle == "Work")
        #expect(d.first?.categoryIdentifier == NotificationIDs.category)
    }

    // MARK: Graph rich payload decrypts on the device

    @Test func encryptedGraphPayloadDecryptsToSenderAndSubject() throws {
        let enc = Fixtures.graphEncrypted(resource: Fixtures.messageResource, publicKey: keys.pub)
        let m = try GraphRichDecryptor.message(from: enc, keys: SecKeyRichKeyUnwrapper(privateKey: keys.priv))
        #expect(m.sender == Fixtures.sender)
        #expect(m.subject == Fixtures.subject)
    }

    @Test func richPushShowsDecryptedSenderWithoutFetching() async {
        let enc = Fixtures.graphEncrypted(resource: Fixtures.messageResource, publicKey: keys.pub)
        let d = Delivery()
        // a lookup that would fail proves the fetch is not needed when the payload decrypts
        runner(.fails).didReceive(Fixtures.request(Fixtures.richUserInfo(enc)), contentHandler: d.handler())
        #expect(await d.waitForFirst())
        #expect(d.first?.title == Fixtures.sender)
        #expect(d.first?.body == Fixtures.subject)
        #expect(d.first?.subtitle == "Work")
    }

    @Test func tamperedSignatureIsRefusedBeforeDecrypting() {
        let enc = Fixtures.graphEncrypted(resource: Fixtures.messageResource, publicKey: keys.pub, tamperSignature: true)
        #expect(throws: RichDecryptError.signatureMismatch) {
            try GraphRichDecryptor.decrypt(enc, keys: SecKeyRichKeyUnwrapper(privateKey: keys.priv))
        }
    }

    @Test func wrongDeviceKeyCannotRead() {
        let other = Fixtures.makeKeyPair()
        let enc = Fixtures.graphEncrypted(resource: Fixtures.messageResource, publicKey: keys.pub)
        #expect(throws: RichDecryptError.keyUnwrapFailed) {
            try GraphRichDecryptor.decrypt(enc, keys: SecKeyRichKeyUnwrapper(privateKey: other.priv))
        }
    }

    // MARK: the completion handler is called in every path

    @Test func decryptFailureFallsBackToFetchAndCompletes() async {
        let enc = Fixtures.graphEncrypted(resource: Fixtures.messageResource, publicKey: keys.pub, tamperSignature: true)
        let d = Delivery()
        runner(.ok(fetched)).didReceive(Fixtures.request(Fixtures.richUserInfo(enc)), contentHandler: d.handler())
        #expect(await d.waitForFirst())
        #expect(d.count == 1)
        #expect(d.first?.title == "Grace Hopper")
    }

    @Test func decryptAndFetchBothFailingStillCompletesWithGenericAlert() async {
        let enc = Fixtures.graphEncrypted(resource: Fixtures.messageResource, publicKey: keys.pub, tamperSignature: true)
        let d = Delivery()
        runner(.fails).didReceive(Fixtures.request(Fixtures.richUserInfo(enc)), contentHandler: d.handler())
        #expect(await d.waitForFirst())
        #expect(d.count == 1)
        #expect(d.first?.title == ServiceRunner.genericTitle)
    }

    @Test func fetchFailureCompletes() async {
        let d = Delivery()
        runner(.fails).didReceive(Fixtures.request(Fixtures.signalUserInfo()), contentHandler: d.handler())
        #expect(await d.waitForFirst())
        #expect(d.count == 1)
        #expect(d.first?.title == ServiceRunner.genericTitle)
    }

    static let malformed: [Int] = [0, 1, 2, 3]
    static func malformedInfo(_ i: Int) -> [AnyHashable: Any] {
        switch i {
        case 0: return [:]
        case 1: return ["whoosh": "not a dictionary"]
        case 2: return ["whoosh": ["v": 9, "account": "a", "message": "m"]]
        default: return ["whoosh": ["v": 1, "account": "a", "message": "m", "kind": "graph-rich"]]
        }
    }

    @Test(arguments: malformed)
    func malformedPayloadCompletes(case index: Int) async {
        let d = Delivery()
        runner(.ok(fetched)).didReceive(Fixtures.request(Self.malformedInfo(index)), contentHandler: d.handler())
        #expect(await d.waitForFirst())
        #expect(d.count == 1)
        #expect(d.first?.title == ServiceRunner.genericTitle)
    }

    @Test func timeWillExpireDeliversTheBestAttemptOnce() async {
        let d = Delivery()
        let r = runner(.hangs)
        r.didReceive(Fixtures.request(Fixtures.signalUserInfo()), contentHandler: d.handler())
        try? await Task.sleep(for: .milliseconds(50))
        #expect(d.count == 0)
        r.serviceExtensionTimeWillExpire()
        r.serviceExtensionTimeWillExpire()
        try? await Task.sleep(for: .milliseconds(100))
        #expect(d.count == 1)
        #expect(d.first?.title == ServiceRunner.genericTitle)
    }

    @Test func handlerRunsOnceWhenFinishAndExpiryRace() async {
        for _ in 0..<50 {
            let d = Delivery()
            let r = runner(.ok(fetched))
            r.didReceive(Fixtures.request(Fixtures.signalUserInfo()), contentHandler: d.handler())
            r.serviceExtensionTimeWillExpire()
            try? await Task.sleep(for: .milliseconds(20))
            #expect(d.count == 1)
        }
    }

    // MARK: Archive and Reply are registered and act in the core

    @Test func archiveAndReplyRegisteredAsCategoryActions() {
        let categories = NotificationCategories.make()
        let c = categories.first { $0.identifier == NotificationIDs.category }
        #expect(c != nil)
        #expect(c?.actions.map(\.identifier) == [NotificationIDs.archive, NotificationIDs.reply])
        #expect(c?.actions.first?.options.contains(.foreground) == false)       // runs without opening the app
        #expect(c?.actions.last is UNTextInputNotificationAction)
    }

    @Test func archiveFromTheCategoryArchivesInTheCore() async {
        let core = ActionLog()
        let out = await NotificationActionRouter(core: core)
            .handle(actionIdentifier: NotificationIDs.archive, userInfo: Fixtures.signalUserInfo(), textInput: nil)
        #expect(out == .archived)
        #expect(await core.archived == ["\(Fixtures.account)/\(Fixtures.messageID)"])
    }

    @Test func replySendsTypedTextAndRefusesEmpty() async {
        let core = ActionLog()
        let router = NotificationActionRouter(core: core)
        #expect(await router.handle(actionIdentifier: NotificationIDs.reply, userInfo: Fixtures.signalUserInfo(), textInput: "  ") == .ignored)
        #expect(await router.handle(actionIdentifier: NotificationIDs.reply, userInfo: Fixtures.signalUserInfo(), textInput: "On it") == .replied)
        #expect(await core.replies.count == 1)
    }

    @Test func coreRefusalStillFinishesTheAction() async {
        let core = ActionLog(); await core.setFail(true)
        let router = NotificationActionRouter(core: core)
        await confirmation { done in
            router.handle(actionIdentifier: NotificationIDs.archive, userInfo: Fixtures.signalUserInfo(), textInput: nil) { outcome in
                if case .failed = outcome { done() }
            }
            try? await Task.sleep(for: .milliseconds(200))
        }
    }
}
