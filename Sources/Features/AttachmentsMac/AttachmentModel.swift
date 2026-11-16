#if os(macOS)
import Foundation
import UniformTypeIdentifiers

/// One attachment as the Mac treats a file: the message that holds it, its place in that message, its file name.
/// A pending file in the composer carries the message id `AttachmentRef.composer`.
public struct AttachmentRef: Hashable, Sendable {
    public let messageId: String
    public let index: Int
    public let name: String
    public static let composer = "composer"
    public var isPending: Bool { messageId == Self.composer }
    public var kind: AttachmentKind { AttachmentKind.of(name) }
}

/// What the file is, read from its name: decides a real thumbnail (image, PDF, video) or the document icon.
public enum AttachmentKind: String, Sendable {
    case image, pdf, video, audio, document, archive, other

    public static func of(_ name: String) -> AttachmentKind {
        guard let t = UTType(filenameExtension: (name as NSString).pathExtension.lowercased()) else { return .other }
        if t.conforms(to: .pdf) { return .pdf }
        if t.conforms(to: .image) { return .image }
        if t.conforms(to: .movie) || t.conforms(to: .video) { return .video }
        if t.conforms(to: .audio) { return .audio }
        if t.conforms(to: .archive) { return .archive }
        if t.conforms(to: .content) || t.conforms(to: .text) { return .document }
        return .other
    }

    /// Images, PDFs and videos get a real thumbnail in the chip; other kinds show their document icon.
    public var hasThumbnail: Bool { self == .image || self == .pdf || self == .video }
}

/// Where the file's bytes are. `ready` holds the local copy Quick Look, Open, drag and Share use.
public enum AttachmentPhase: Equatable, Sendable {
    case remote
    case downloading(Double)
    case ready(URL)
    case unavailable(String)

    public var url: URL? { if case .ready(let u) = self { return u }; return nil }
    /// A download's fraction, or nil when the source gives none (the real core carries no percent: `.downloading(noFigure)`).
    public var fraction: Double? { if case .downloading(let p) = self, p >= 0 { return p }; return nil }
    public static let noFigure = -1.0
    public var isFetching: Bool { if case .downloading = self { return true }; return false }
    /// 0 when fetching with no figure, so a ring or a count never reads a negative.
    public var progress: Double? { if case .downloading(let p) = self { return max(0, p) }; return nil }
    public var word: String {
        switch self {
        case .remote: return "remote"
        case .downloading: return "downloading"
        case .ready: return "ready"
        case .unavailable: return "unavailable"
        }
    }
}

/// An act asked for while the file was still downloading: it runs the moment the bytes land.
public enum AttachmentWaitingAct: String, Sendable { case quickLook = "quick-look", open }

/// The reader's tile words carry the size after the name ("quote.pdf (212 KB)"): the file name and the size apart.
public enum AttachmentName {
    public static func split(_ words: String) -> (file: String, size: String?) {
        guard words.hasSuffix(")"), let open = words.range(of: " (", options: .backwards) else { return (words, nil) }
        let size = String(words[open.upperBound..<words.index(before: words.endIndex)])
        return (String(words[..<open.lowerBound]), size)
    }

    /// True when the words name a file (an extension after the last dot), not a count like "3 attachments".
    public static func isFile(_ words: String) -> Bool {
        let ext = (split(words).file as NSString).pathExtension
        return !ext.isEmpty && ext.count <= 5 && !ext.contains(" ")
    }
}
#endif
