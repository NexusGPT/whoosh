import Foundation
import CryptoKit
import CommonCrypto
import Security
import UserNotifications
@testable import NotificationServiceKit

/// Invented data only: a generated key pair, an invented sender and subject. Nothing real, nothing sent.
enum Fixtures {
    static let sender = "Ada Lovelace"
    static let subject = "Engine schedule for Tuesday"
    static let account = "acct-routing-7f3a"
    static let messageID = "msg-0001"

    static func makeKeyPair() -> (priv: SecKey, pub: SecKey) {
        let attrs: [String: Any] = [kSecAttrKeyType as String: kSecAttrKeyTypeRSA, kSecAttrKeySizeInBits as String: 2048]
        var err: Unmanaged<CFError>?
        let priv = SecKeyCreateRandomKey(attrs as CFDictionary, &err)!
        return (priv, SecKeyCopyPublicKey(priv)!)
    }

    /// Builds what Graph sends: AES-256-CBC data, RSA-OAEP(SHA-1) wrapped key, HMAC-SHA256 signature.
    static func graphEncrypted(resource: [String: Any], publicKey: SecKey, tamperSignature: Bool = false) -> GraphEncryptedContent {
        let plain = try! JSONSerialization.data(withJSONObject: resource)
        let key = Data((0..<32).map { _ in UInt8.random(in: 0...255) })
        var out = Data(count: plain.count + 16); var moved = 0
        let status = out.withUnsafeMutableBytes { o in plain.withUnsafeBytes { p in key.withUnsafeBytes { k in key.prefix(16).withUnsafeBytes { iv in
            CCCrypt(CCOperation(kCCEncrypt), CCAlgorithm(kCCAlgorithmAES), CCOptions(kCCOptionPKCS7Padding),
                    k.baseAddress, 32, iv.baseAddress, p.baseAddress, plain.count, o.baseAddress, o.count, &moved) } } } }
        precondition(status == kCCSuccess)
        let cipher = out.prefix(moved)
        var sig = Data(HMAC<SHA256>.authenticationCode(for: cipher, using: SymmetricKey(data: key)))
        if tamperSignature { sig[0] ^= 0xFF }
        var err: Unmanaged<CFError>?
        let wrapped = SecKeyCreateEncryptedData(publicKey, .rsaEncryptionOAEPSHA1, key as CFData, &err)! as Data
        return GraphEncryptedContent(data: cipher.base64EncodedString(), dataKey: wrapped.base64EncodedString(),
                                     dataSignature: sig.base64EncodedString(), certificateID: "cert-1")
    }

    static var messageResource: [String: Any] { [
        "id": messageID, "subject": subject, "bodyPreview": "invented preview",
        "from": ["emailAddress": ["name": sender, "address": "ada@example.test"]],
    ] }

    static func signalUserInfo() -> [AnyHashable: Any] {
        ["aps": ["alert": ["title": "New mail"], "mutable-content": 1],
         "whoosh": ["v": 1, "account": account, "message": messageID, "kind": "signal"]]
    }

    static func richUserInfo(_ c: GraphEncryptedContent) -> [AnyHashable: Any] {
        ["aps": ["alert": ["title": "New mail"], "mutable-content": 1],
         "whoosh": ["v": 1, "account": account, "message": messageID, "kind": "graph-rich",
                    "rich": ["data": c.data, "dataKey": c.dataKey, "dataSignature": c.dataSignature,
                             "encryptionCertificateId": c.certificateID ?? ""]]]
    }

    static func request(_ userInfo: [AnyHashable: Any]) -> UNNotificationRequest {
        let content = UNMutableNotificationContent()
        content.title = "New mail"; content.userInfo = userInfo
        return UNNotificationRequest(identifier: "req", content: content, trigger: nil)
    }
}

struct FakeLookup: MailLookup {
    enum Mode: Sendable { case ok(FetchedMessage), fails, hangs }
    var mode: Mode
    var name: String? = "Work"
    func fetchMessage(accountRoutingID: String, messageID: String) async throws -> FetchedMessage {
        switch mode {
        case .ok(let m): return m
        case .fails: struct Boom: Error {}; throw Boom()
        case .hangs: try await Task.sleep(for: .seconds(3600)); throw CancellationError()
        }
    }
    func accountName(accountRoutingID: String) async -> String? { name }
}

/// Collects what the system's completion handler receives.
final class Delivery: @unchecked Sendable {
    private let lock = NSLock()
    private var contents: [UNNotificationContent] = []
    func handler() -> (UNNotificationContent) -> Void { { [self] c in lock.lock(); contents.append(c); lock.unlock() } }
    var count: Int { lock.lock(); defer { lock.unlock() }; return contents.count }
    var first: UNNotificationContent? { lock.lock(); defer { lock.unlock() }; return contents.first }
    func waitForFirst(seconds: Double = 5) async -> Bool {
        let end = Date().addingTimeInterval(seconds)
        while Date() < end { if count > 0 { return true }; try? await Task.sleep(for: .milliseconds(10)) }
        return false
    }
}

actor ActionLog: ActionCore {
    var archived: [String] = []; var replies: [(String, String)] = []; var fail = false
    func setFail(_ v: Bool) { fail = v }
    func archive(accountRoutingID: String, messageID: String) async throws {
        if fail { struct Refused: Error {}; throw Refused() }
        archived.append("\(accountRoutingID)/\(messageID)")
    }
    func reply(accountRoutingID: String, messageID: String, text: String) async throws {
        if fail { struct Refused: Error {}; throw Refused() }
        replies.append((messageID, text))
    }
}
