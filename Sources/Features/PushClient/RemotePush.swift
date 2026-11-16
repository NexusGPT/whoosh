#if os(iOS)
import UIKit
import UserNotifications

/// iPhone side: ask for permission, then let the system mint the APNs token. The app delegate forwards the token to
/// `PushRegistrar.setToken`. Compiled for the simulator through SwiftPM; the run needs the iOS platform (see docs/lanes/pushclient.md).
@MainActor
public enum RemotePush {
    public static func requestPermissionAndRegister() async -> Bool {
        let granted = (try? await UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound, .badge])) ?? false
        if granted { UIApplication.shared.registerForRemoteNotifications() }
        return granted
    }
}
#endif
