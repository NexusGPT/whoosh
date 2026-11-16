import Foundation

/// Matryoshka truncation: keep the first `dimension` values, then scale back to unit length.
/// EmbeddingGemma 2 is trained so that a prefix of its 768 values is still a good vector.
/// Pure Swift, no model: the unit tests run it without any weights.
public enum MatryoshkaVector {
    /// Returns the first `dimension` values of `full`, renormalised to length 1.
    /// A vector shorter than `dimension`, or one holding a non-finite value, throws.
    public static func truncate(_ full: [Float], to dimension: Int) throws -> [Float] {
        guard dimension > 0, full.count >= dimension else {
            throw GemmaEmbeddingError.badVector(reasonWords: "the model gave \(full.count) values, \(dimension) were asked for")
        }
        var head = Array(full.prefix(dimension))
        var sum: Float = 0
        for v in head {
            guard v.isFinite else { throw GemmaEmbeddingError.badVector(reasonWords: "the model gave a value that is not a number") }
            sum += v * v
        }
        guard sum > 0 else { throw GemmaEmbeddingError.badVector(reasonWords: "the model gave a vector of zeros") }
        let scale = 1 / sum.squareRoot()
        for i in head.indices { head[i] *= scale }
        return head
    }
}

public enum GemmaEmbeddingError: Error, Equatable {
    case badVector(reasonWords: String)
    case weightsMissing(reasonWords: String)
    case download(reasonWords: String)
}
