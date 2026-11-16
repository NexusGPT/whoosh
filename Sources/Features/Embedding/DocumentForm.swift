import Foundation

/// The document form of EmbeddingGemma 2: `title: <title or none> | text: <text>` (owner's gemma.py:41).
/// For mail the title is the subject (owner default taken 2026-11-09; it moves vectors away from the Python reference,
/// which used `none`, so the index is rebuilt). The contract trait carries it since delta 12 (`Embedder.embedTitled`). Pure Swift.
public enum DocumentForm {
    public static let noTitle = "none"

    /// One line, no ` | ` separator inside, trimmed; empty gives `none`.
    public static func cleanTitle(_ title: String?) -> String {
        guard let title else { return noTitle }
        let oneLine = title.split(whereSeparator: { $0.isNewline }).joined(separator: " ")
        let noBar = oneLine.replacingOccurrences(of: " | ", with: " / ")
        let trimmed = noBar.trimmingCharacters(in: .whitespaces)
        return trimmed.isEmpty ? noTitle : trimmed
    }

    public static func text(title: String?, text: String) -> String {
        "title: \(cleanTitle(title)) | text: \(text)"
    }
}

/// An embedder that takes documents with a title EACH in one call. The contract's `embedTitled` has one title per call, which
/// is right for the chunks of one document; the index embeds many mails (one subject each), and one call per mail loses the
/// batching (round 8: 300,000 calls of one text). `GemmaEmbedder` implements this beside the trait.
public protocol BatchTitledEmbedder: Embedder {
    func embedDocumentBatch(_ items: [(title: String?, text: String)]) throws -> [[Float]]
}

/// pieces (SPEC A151): an embedder that says the folder its weights load from (its tokenizer.json sizes the engine's pieces).
public protocol ModelFolderNaming {
    var modelFolder: String { get }
}

/// pieces (SPEC A151): the model's input cap, kept as a safety, and the count of the inputs it had to shorten. The engine cuts every
/// piece so that it fits with its title (search-vector `pieces`), so the count stays 0; a count above 0 says a piece lost its end.
/// Pure Swift (no model), so a unit test reads it; `GemmaEmbedder.cutInputs` is this count for the live model.
public final class InputCut: @unchecked Sendable {
    private let lock = NSLock()
    private var cut = 0

    public init() {}

    /// Each token list cut to `limit` tokens; the lists that were longer are counted.
    public func apply(_ ids: [[Int]], limit: Int) -> [[Int]] {
        let over = ids.filter { $0.count > limit }.count
        if over > 0 {
            lock.lock()
            cut += over
            lock.unlock()
        }
        return ids.map { Array($0.prefix(limit)) }
    }

    /// How many inputs were shortened since this object was made.
    public var cutCount: Int {
        lock.lock()
        defer { lock.unlock() }
        return cut
    }
}
