import Foundation
#if os(macOS)
import AppKit
#endif

/// The events the notch listens for besides the push feed.
@MainActor
public enum NotchEvents {
    /// N10: the event the reader (or the core) posts when a tracked message is opened by its recipient.
    /// userInfo: messageId, subject, who (all String); seenKey (String, rig only). Nothing posts it yet: docs/lanes/notch.md names the need.
    static let trackedOpened = Notification.Name("whoosh.message.tracked-opened")
    /// Posted by the Settings lane after it writes a notch setting; the notch also re-reads when any window gains or loses key.
    static let settingsChanged = Notification.Name("whoosh.notch.settings-changed")
    private static var started = false

    /// Once: `fixture deliver` (the rig's arrival) and the tracked-open event, both on the posting thread, so a
    /// socket reply is sent after the notch already holds the mail.
    public static func start() {
        guard !started else { return }
        started = true
        NotificationCenter.default.addObserver(forName: FixtureVerbs.arrival, object: nil, queue: nil) { n in
            guard let f = n.userInfo as? [String: Any], let id = f["id"] as? String else { return }
            let drop = NotchDrop(messageId: id, accountId: f["account"] as? String ?? "", senderName: f["sender"] as? String ?? "",
                                 subject: f["subject"] as? String ?? "", loginCode: f["code"] as? String)
            let preview = f["first_lines"] as? String, thread = f["thread"] as? String, from = f["from"] as? String
            MainActor.assumeIsolated { NotchFeature.deliver(drop, preview: preview, threadId: thread, address: from) }
        }
        let c = NotificationCenter.default
        c.addObserver(forName: FixtureVerbs.frontApp, object: nil, queue: nil) { n in
            let name = n.userInfo?["name"] as? String ?? ""
            MainActor.assumeIsolated { NotchModel.shared.frontApp = name }
        }
        c.addObserver(forName: FixtureVerbs.pointerRest, object: nil, queue: nil) { n in
            guard (n.userInfo?["target"] as? String)?.hasPrefix(NotchIds.surface) == true else { return }
            MainActor.assumeIsolated {
                #if os(macOS)
                NotchPanel.shared.install()
                #endif
                NotchModel.shared.hover(true)
            }
        }
        c.addObserver(forName: FixtureVerbs.dropFile, object: nil, queue: nil) { n in
            guard (n.userInfo?["target"] as? String)?.hasPrefix(NotchIds.surface) == true, let f = n.userInfo?["file"] as? String else { return }
            MainActor.assumeIsolated { NotchFeature.wire(); _ = NotchModel.shared.dropFile(rigFile(f)) }
        }
        c.addObserver(forName: FixtureVerbs.muteAccount, object: nil, queue: nil) { n in
            guard let id = n.userInfo?["account"] as? String else { return }
            MainActor.assumeIsolated { NotchModel.shared.muted.insert(id) }
        }
        c.addObserver(forName: FixtureVerbs.notchGeometry, object: nil, queue: nil) { n in
            let none = n.userInfo?["shape"] as? String == "none"
            MainActor.assumeIsolated {
                #if os(macOS)
                NotchPanel.shared.install()
                #endif
                NotchModel.shared.shapeOverride = none ? NotchShape.none : nil
                NotchModel.shared.onChange()
                #if os(macOS)
                NotchPanel.shared.place()
                #endif
            }
        }
        c.addObserver(forName: settingsChanged, object: nil, queue: nil) { _ in MainActor.assumeIsolated { NotchFeature.rereadSettings() } }
        #if os(macOS)
        for name in [NSWindow.didBecomeKeyNotification, NSWindow.didResignKeyNotification] {
            c.addObserver(forName: name, object: nil, queue: nil) { _ in MainActor.assumeIsolated { NotchFeature.rereadSettings() } }
        }
        #endif
        NotificationCenter.default.addObserver(forName: trackedOpened, object: nil, queue: nil) { n in
            guard let f = n.userInfo as? [String: Any], let id = f["messageId"] as? String else { return }
            let subject = f["subject"] as? String ?? "", who = f["who"] as? String ?? "", key = f["seenKey"] as? String
            MainActor.assumeIsolated {
                let seen = key.map(NotchSeenStore.init(key:)) ?? .standard   // a drive keeps its own store
                NotchFeature.trackedOpened(messageId: id, subject: subject, who: who, seen: seen)
            }
        }
    }

    /// The rig's `fixture drop-file` names a file by a bare name (fixtures/tests-j6.json "quote.pdf"): a real drop carries a
    /// path that exists, so the rig writes a small file of that name in the temp folder first. A path with a folder is used as given.
    static func rigFile(_ name: String) -> URL {
        guard !name.contains("/") else { return URL(fileURLWithPath: name) }
        let url = FileManager.default.temporaryDirectory.appendingPathComponent(name)
        if !FileManager.default.fileExists(atPath: url.path) { try? Data(name.utf8).write(to: url) }
        return url
    }
}
