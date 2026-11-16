import Foundation

/// The fixture core's embedder stub (delta 8, group 2): a deterministic bag-of-words hash into EMBED_DIM floats, unit length.
/// It implements both foreign traits the search round adds, so the contract is exercised end to end with no model: documents
/// and queries take DIFFERENT prefixes (the same words embed to different, near vectors), and an image or a voice note is
/// embedded from its file name, so a text query that names it lands near it. Not a model; it proves the plumbing.
public final class FixtureEmbedder: Embedder, MediaEmbedder, @unchecked Sendable {
    public init() {}
    public static let documentPrefix = "title: none | text: "
    public static let queryPrefix = "task: search result | query: "

    public func dimension() -> UInt32 { UInt32(WhooshConstants.EMBED_DIM) }

    public func embed(texts: [String]) throws -> [[Float]] { texts.map { Self.vector(Self.documentPrefix + $0) } }
    /// Delta 12: the mail's subject in the title slot, "title: <subject> | text: "; no title or a blank one is "none".
    public func embedTitled(title: String?, texts: [String]) throws -> [[Float]] {
        let t = title?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let prefix = t.isEmpty ? Self.documentPrefix : "title: \(t) | text: "
        return texts.map { Self.vector(prefix + $0) }
    }
    public func embedQuery(text: String) throws -> [Float] { Self.vector(Self.queryPrefix + text) }
    public func embedImage(path: String) throws -> [Float] { try Self.media(path) }
    public func embedAudio(path: String) throws -> [Float] { try Self.media(path) }
    /// Delta 9: the one media entry; image, audio and a video frame all embed from the file name, like the two above.
    public func embedMedia(kind: MediaKind, path: String) throws -> [Float] { try Self.media(path) }

    private static func media(_ path: String) throws -> [Float] {
        let name = (path as NSString).lastPathComponent
        guard !name.isEmpty else { throw CoreError.Invalid(field: "path", reasonWords: "there is no file to read at that path") }
        return vector(documentPrefix + (name as NSString).deletingPathExtension.replacingOccurrences(of: "-", with: " "))
    }

    static func vector(_ text: String) -> [Float] {
        var v = [Float](repeating: 0, count: WhooshConstants.EMBED_DIM)
        for w in text.lowercased().split(whereSeparator: { !$0.isLetter && !$0.isNumber }) {
            var h: UInt64 = 0xcbf29ce484222325
            for b in w.utf8 { h ^= UInt64(b); h = h &* 0x100000001b3 }
            v[Int(h % UInt64(v.count))] += (h >> 40) & 1 == 0 ? 1 : -1
        }
        let norm = v.reduce(0) { $0 + $1 * $1 }.squareRoot()
        return norm == 0 ? v : v.map { $0 / norm }
    }
}
