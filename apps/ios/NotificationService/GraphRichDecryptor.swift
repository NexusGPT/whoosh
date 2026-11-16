import Foundation
import CryptoKit
import CommonCrypto
import Security

/// Microsoft Graph "rich notification" encryptedContent (change-notifications-with-resource-data):
/// `dataKey` is the AES key wrapped with the subscription certificate's public key (RSA-OAEP, SHA-1),
/// `data` is AES-256-CBC (IV = first 16 bytes of the key, PKCS7), `dataSignature` is HMAC-SHA256 of `data`.
public struct GraphEncryptedContent: Equatable, Sendable {
    public let data: String
    public let dataKey: String
    public let dataSignature: String
    public let certificateID: String?
    public init(data: String, dataKey: String, dataSignature: String, certificateID: String?) {
        self.data = data; self.dataKey = dataKey; self.dataSignature = dataSignature; self.certificateID = certificateID
    }
}

/// The device-side private key. The key never leaves the device; the relay only ever sees the public certificate.
public protocol RichKeyUnwrapping: Sendable {
    func unwrap(_ wrappedKey: Data) throws -> Data
}

public enum RichDecryptError: Error, Equatable {
    case badBase64, keyUnwrapFailed, badKeyLength, signatureMismatch, cipherFailed(Int32), badResource
}

/// Sender and subject read out of the decrypted Graph message resource.
public struct DecryptedMessage: Equatable, Sendable {
    public let sender: String
    public let subject: String
}

public enum GraphRichDecryptor {
    /// Verifies the signature BEFORE decrypting, then decrypts.
    public static func decrypt(_ content: GraphEncryptedContent, keys: RichKeyUnwrapping) throws -> Data {
        guard let wrapped = Data(base64Encoded: content.dataKey),
              let cipher = Data(base64Encoded: content.data),
              let signature = Data(base64Encoded: content.dataSignature) else { throw RichDecryptError.badBase64 }
        let key: Data
        do { key = try keys.unwrap(wrapped) } catch { throw RichDecryptError.keyUnwrapFailed }
        guard key.count == kCCKeySizeAES256 else { throw RichDecryptError.badKeyLength }
        guard HMAC<SHA256>.isValidAuthenticationCode(signature, authenticating: cipher, using: SymmetricKey(data: key))
        else { throw RichDecryptError.signatureMismatch }
        return try aesCBCDecrypt(cipher, key: key, iv: key.prefix(kCCBlockSizeAES128))
    }

    public static func message(from content: GraphEncryptedContent, keys: RichKeyUnwrapping) throws -> DecryptedMessage {
        let plain = try decrypt(content, keys: keys)
        guard let json = try? JSONSerialization.jsonObject(with: plain) as? [String: Any] else { throw RichDecryptError.badResource }
        let address = ((json["from"] as? [String: Any])?["emailAddress"] as? [String: Any])
        let name = (address?["name"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? (address?["address"] as? String)
        guard let sender = name else { throw RichDecryptError.badResource }
        return DecryptedMessage(sender: sender, subject: (json["subject"] as? String) ?? "")
    }

    static func aesCBCDecrypt(_ cipher: Data, key: Data, iv: Data) throws -> Data {
        var out = Data(count: cipher.count + kCCBlockSizeAES128)
        var moved = 0
        let status = out.withUnsafeMutableBytes { outPtr in
            cipher.withUnsafeBytes { inPtr in
                key.withUnsafeBytes { keyPtr in
                    iv.withUnsafeBytes { ivPtr in
                        CCCrypt(CCOperation(kCCDecrypt), CCAlgorithm(kCCAlgorithmAES), CCOptions(kCCOptionPKCS7Padding),
                                keyPtr.baseAddress, key.count, ivPtr.baseAddress,
                                inPtr.baseAddress, cipher.count, outPtr.baseAddress, outPtr.count, &moved)
                    }
                }
            }
        }
        guard status == kCCSuccess else { throw RichDecryptError.cipherFailed(status) }
        return out.prefix(moved)
    }
}

/// RSA-OAEP(SHA-1) unwrap with a SecKey. Production loads the key from the shared Keychain access group
/// (the app creates it at account setup); tests hand in a generated key.
public struct SecKeyRichKeyUnwrapper: RichKeyUnwrapping, @unchecked Sendable {
    private let privateKey: SecKey
    public init(privateKey: SecKey) { self.privateKey = privateKey }

    public func unwrap(_ wrappedKey: Data) throws -> Data {
        var error: Unmanaged<CFError>?
        guard let plain = SecKeyCreateDecryptedData(privateKey, .rsaEncryptionOAEPSHA1, wrappedKey as CFData, &error) else {
            throw RichDecryptError.keyUnwrapFailed
        }
        return plain as Data
    }

    /// The key the app stored for Graph rich notifications, found by tag in the shared access group.
    public static func fromKeychain(tag: String, accessGroup: String?) -> SecKeyRichKeyUnwrapper? {
        var query: [String: Any] = [kSecClass as String: kSecClassKey,
                                    kSecAttrApplicationTag as String: Data(tag.utf8),
                                    kSecAttrKeyType as String: kSecAttrKeyTypeRSA,
                                    kSecReturnRef as String: true]
        if let accessGroup { query[kSecAttrAccessGroup as String] = accessGroup }
        var item: CFTypeRef?
        guard SecItemCopyMatching(query as CFDictionary, &item) == errSecSuccess, let item else { return nil }
        return SecKeyRichKeyUnwrapper(privateKey: item as! SecKey) // CF cast: SecItem returned a key ref
    }
}
