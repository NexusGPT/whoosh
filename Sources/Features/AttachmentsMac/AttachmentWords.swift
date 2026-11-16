#if os(macOS)
import Foundation

/// Every word the attachment proxy shows, through the catalogue (Words.text).
enum AttachmentWords {
    static var quickLook: String { Words.text("Quick Look") }
    static var open: String { Words.text("Open") }
    static var openWith: String { Words.text("Open With") }
    static var saveToDownloads: String { Words.text("Save to Downloads") }
    static var saveAs: String { Words.text("Save As…") }
    static var copy: String { Words.text("Copy") }
    static var share: String { Words.text("Share") }
    static var showInFinder: String { Words.text("Show in Finder") }
    static var remove: String { Words.text("Remove") }
    static var other: String { Words.text("Other…") }
    static var defaultApp: String { Words.text("default") }
    static var spaceKey: String { Words.text("Space") }
    static func downloading(_ percent: Int) -> String { Words.text("Downloading %@", (Double(percent) / 100).formatted(.percent.locale(AttachmentSize.locale))) }
    /// The absent chip's one grey line (design Attachment T1): fetching with a figure, fetching without, failed.
    static func fetching(_ percent: Int) -> String { Words.text("Fetching · %@", (Double(percent) / 100).formatted(.percent.locale(AttachmentSize.locale))) }
    static var fetchingBare: String { Words.text("Fetching") }
    static func notFetched(_ reason: String) -> String {
        var why = reason.trimmingCharacters(in: .whitespacesAndNewlines)
        while why.hasSuffix(".") { why.removeLast() }
        return Words.text("Not fetched: %@. Space retries.", why)
    }
    static var waitingQuickLook: String { Words.text("Quick Look opens when the download ends") }
    static var waitingOpen: String { Words.text("Opens when the download ends") }
    static var notOnThisMac: String { Words.text("Not on this Mac yet") }
    static func saved(_ folder: String) -> String { Words.text("Saved to %@", folder) }
    static func opened(_ app: String) -> String { Words.text("Opened in %@", app) }
    static var copied: String { Words.text("Copied") }
    static var saveAll: String { Words.text("Save all") }
    static var saveAllTip: String { Words.text("Save every attachment to Downloads") }
    static var quickLookAllTip: String { Words.text("Quick Look over every attachment · Space") }
    static var more: String { Words.text("more") }
    static func showMore(_ n: Int) -> String { Words.text("Show %lld more", n) }
    /// One file is "1 file", in every language through its own key ("1 files" was shown in the English rig).
    static func galleryCount(_ n: Int, _ size: String) -> String {
        n == 1 ? Words.text("1 file · %@", size) : Words.text("%lld files · %@", n, size)
    }
    /// No file of the message is here: no size is printed (there is none to print).
    static func galleryNoneHere(_ n: Int) -> String {
        n == 1 ? Words.text("1 file · not on this Mac yet") : Words.text("%lld files · not on this Mac yet", n)
    }
    /// Some are here: how many, never a size that counts only part of the files.
    static func galleryPartial(_ n: Int, _ here: Int) -> String { Words.text("%lld files · %lld on this Mac", n, here) }
    /// The tooltip: what the chip is, then its keys (docs/CONSTRAINTS.md: every control shows its key).
    static func tooltip(_ name: String) -> String {
        Words.text("%@ · Quick Look: Space · Open: ⌘O · Double-click opens · Drag to the Finder", name)
    }
    static func accessibility(_ name: String, _ kind: String) -> String { Words.text("%@, %@ attachment", name, kind) }
}
#endif
