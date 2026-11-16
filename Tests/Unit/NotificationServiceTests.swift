import Foundation
import Testing

// fn-notificationservice: "an encrypted Graph payload fixture decrypts on device; Archive and Reply actions are registered".
//
// HAND-BACK (design finding): the Notification Service Extension is an app-extension target (apps/ios/NotificationService/**).
// WhooshTests links WhooshUI only, so no test here can call the extension's decrypt function, and a test must not re-implement
// the decryptor and then assert its own copy. Until the decrypt and the category registration live in a file the test target
// can import (for example under Sources/Features/PushClient/), the checks below read the extension's SOURCE for what the
// acceptance names, and FAIL while the extension does not exist. The owner of the lane decides the seam; this seat does not
// edit production.

private let repoRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private let nse = repoRoot.appendingPathComponent("apps/ios/NotificationService")

private func sources() throws -> [(name: String, text: String)] {
    let fm = FileManager.default
    try #require(fm.fileExists(atPath: nse.path), "apps/ios/NotificationService does not exist yet")
    let names = try fm.contentsOfDirectory(atPath: nse.path).filter { $0.hasSuffix(".swift") }
    try #require(!names.isEmpty, "the extension folder holds no Swift source")
    return try names.map { ($0, try String(contentsOf: nse.appendingPathComponent($0), encoding: .utf8)) }
}

@Test func theExtensionTargetExistsAndIsDeclaredAsAUserNotificationsService() throws {
    _ = try sources()                                                                          // mutant: no extension folder
    let plist = nse.appendingPathComponent("Info.plist")
    let text = try String(contentsOf: plist, encoding: .utf8)
    #expect(text.contains("com.apple.usernotifications.service"))                              // mutant: wrong extension point, iOS never runs it
}
@Test func archiveAndReplyAreRegisteredAsNotificationActions() throws {
    let all = try sources().map(\.text).joined(separator: "\n")
    #expect(all.contains("UNNotificationAction"))                                              // mutant: no actions registered at all
    #expect(all.range(of: #"identifier:\s*"archive""#, options: .regularExpression) != nil)    // mutant: Archive missing
    #expect(all.range(of: #"identifier:\s*"reply""#, options: .regularExpression) != nil)      // mutant: Reply missing
}
@Test func theExtensionDecryptsGraphRichNotificationsAndCallsBackInTime() throws {
    let all = try sources().map(\.text).joined(separator: "\n")
    #expect(all.contains("encryptedContent"))                                                  // mutant: the Graph envelope field is never read
    #expect(all.contains("dataSignature"))                                                     // mutant: the HMAC is never checked, any payload is trusted
    #expect(all.contains("contentHandler"))                                                    // mutant: the completion handler is never called
    #expect(all.contains("serviceExtensionTimeWillExpire"))                                    // mutant: nothing is delivered when the 30-second window ends
}
