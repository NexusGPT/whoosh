import Foundation

/// The step of an embedding that failed (lane query-embed). The engine writes the words to its log (`search: the model did not embed
/// the query: <words>`), so a failed search says which step stopped and the model's own error. These are engine words for the log and
/// Details, never a sentence shown to the owner (that one comes from the catalogue). They never hold the text that was embedded.
/// Pure Swift, no MLX: the unit tests build every case without a model.
public enum EmbedStep: String, Sendable, Equatable, CaseIterable {
    /// Reading the model's files into memory (`EmbedderModelFactory.loadContainer`).
    case load
    /// Turning the text into tokens.
    case tokenise
    /// The forward pass and the pooling.
    case run
    /// Reading the vector back (length, finite values, not all zeros: `MatryoshkaVector.truncate`).
    case check
}

public struct EmbedStepFailure: Error, Equatable, Sendable {
    public let step: EmbedStep
    /// The model library's own words (or ours for `tokenise` and `check`); never the embedded text.
    public let detail: String

    public init(step: EmbedStep, detail: String) { self.step = step; self.detail = detail }

    /// `the search model could not <step>: <detail>`.
    public var words: String { "the search model could not \(step.rawValue): \(detail)" }

    /// The error the generated `Embedder` throws to the engine for any failure of an embedding (GemmaEmbedder's sync bridge).
    /// A missing model keeps its own words; a failed step says the step; anything else keeps its full description
    /// (`String(describing:)`, which for an NSError keeps the domain, code and reason that `localizedDescription` loses).
    public static func coreError(_ error: Error?) -> CoreError {
        guard let error else { return .Unexpected(reasonWords: "embedding gave no result") }
        switch error {
        case let failure as EmbedStepFailure:
            return .Unexpected(reasonWords: failure.words)
        case GemmaEmbeddingError.weightsMissing(let words):
            return .Unexpected(reasonWords: words)
        case GemmaEmbeddingError.badVector(let words):
            return .Unexpected(reasonWords: EmbedStepFailure(step: .check, detail: words).words)
        default:
            return .Unexpected(reasonWords: "embedding failed: \(String(describing: error))")
        }
    }

    /// What `load` adds to the library's error so one hidden run tells the causes apart: the folder as given, the folder it resolves
    /// to, and how many `.safetensors` files a plain listing of the given folder finds (the listing the model library makes).
    public static func folderFacts(_ directory: URL) -> String {
        let listed = (try? FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)) ?? []
        let weights = listed.filter { $0.pathExtension == "safetensors" }.count
        let resolved = directory.resolvingSymlinksInPath().path
        return "folder \(directory.path), resolves to \(resolved), \(listed.count) entries listed, \(weights) weight files listed"
    }
}
