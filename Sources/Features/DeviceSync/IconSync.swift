import Foundation
import CryptoKit

public enum DeviceSyncAccountIcon: Equatable, Sendable {
    /// a preloaded icon from the library: travels as its id only
    case library(id: String)
    /// an uploaded image: travels as a file (a CloudKit asset)
    case uploaded(fileId: String, bytes: Data)
}

extension DeviceSyncEngine {
    /// Set the account's mark. A library icon changes one field; an uploaded image also writes one file record.
    public func setAccountIcon(accountId: String, icon: DeviceSyncAccountIcon) throws {
        var fields = row(kind: "account", id: accountId)?.fields ?? [:]
        switch icon {
        case .library(let id):
            fields["icon_kind"] = "library"
            fields["icon_id"] = id
            fields["icon_file_id"] = nil
        case .uploaded(let fileId, let bytes):
            try write(kind: "account_icon_file", id: fileId, fields: [
                "bytes_b64": bytes.base64EncodedString(),
                "sha256": SHA256.hash(data: bytes).map { String(format: "%02x", $0) }.joined(),
            ])
            fields["icon_kind"] = "file"
            fields["icon_file_id"] = fileId
            fields["icon_id"] = nil
        }
        try write(kind: "account", id: accountId, fields: fields)
    }

    /// The mark this device shows for an account, or nil while the file has not arrived yet.
    public func accountIcon(accountId: String) -> DeviceSyncAccountIcon? {
        guard let account = row(kind: "account", id: accountId) else { return nil }
        if account.fields["icon_kind"] == "library", let id = account.fields["icon_id"] { return .library(id: id) }
        if account.fields["icon_kind"] == "file", let fileId = account.fields["icon_file_id"],
           let file = row(kind: "account_icon_file", id: fileId),
           let b64 = file.fields["bytes_b64"], let bytes = Data(base64Encoded: b64) {
            return .uploaded(fileId: fileId, bytes: bytes)
        }
        return nil
    }
}
