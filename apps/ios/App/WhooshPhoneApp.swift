import SwiftUI
import UserNotifications
import WhooshUI

/// The app delegate has one job: it is the notification centre's delegate from the first moment of the launch, so a tap on a mail notification that
/// starts the app reaches the phone router. Setting a delegate asks for nothing: the permission is asked on the Welcome notification step, never here.
final class WhooshPhoneAppDelegate: NSObject, UIApplicationDelegate {
    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil) -> Bool {
        UNUserNotificationCenter.current().delegate = NotificationTapDelegate.shared
        return true
    }
}

/// The iPhone shell: nothing but the scene. Everything else lives in the WhooshUI package.
@main
struct WhooshPhoneApp: App {
    @UIApplicationDelegateAdaptor(WhooshPhoneAppDelegate.self) private var delegate
    var body: some Scene { WhooshScene() }
}
