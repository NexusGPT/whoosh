import Foundation

/// A file size in the app's language, digits always ("2 kB" in English, "2 ko" in French, "0 KB" never "Zero KB"):
/// ByteCountFormatter followed the system's language, so an English app on a French Mac said "Zéro ko".
enum AttachmentSize {
    /// The app's language (the choice in Settings, else the Mac's): never the system's alone ("0 octet" in an English app on a French Mac).
    static var locale: Locale { Locale(identifier: Words.current) }

    static func words(_ bytes: Int64) -> String {
        bytes.formatted(.byteCount(style: .file, allowedUnits: .all, spellsOutZero: false, includesActualByteCount: false).locale(locale))
    }
}
