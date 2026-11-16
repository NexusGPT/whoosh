import UserNotifications

/// The extension's principal class (NSExtensionPrincipalClass = $(PRODUCT_MODULE_NAME).NotificationService).
/// All behaviour lives in `ServiceRunner`; this class only forwards the two system callbacks.
final class NotificationService: UNNotificationServiceExtension {
    private let runner = ServiceRunner(lookup: AppGroupMailLookup(), keys: ExtensionKeys.load())

    override func didReceive(_ request: UNNotificationRequest,
                             withContentHandler contentHandler: @escaping (UNNotificationContent) -> Void) {
        runner.didReceive(request, contentHandler: contentHandler)
    }

    override func serviceExtensionTimeWillExpire() {
        runner.serviceExtensionTimeWillExpire()
    }
}
