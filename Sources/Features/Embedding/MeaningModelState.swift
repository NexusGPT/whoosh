import Foundation

/// What the Settings row says about the search model (EmbeddingGemma 2), shaped like `LayaModelState` (the decision model's).
public enum MeaningModelState: Equatable, Sendable {
    case notDownloaded
    case downloading(received: Int64, total: Int64)   // total is -1 while the size is not known
    case ready(bytes: Int64)
    case failed(reason: String)                         // the row offers Retry next to this sentence

    public var isDownloading: Bool { if case .downloading = self { return true } else { return false } }
    public var isReady: Bool { if case .ready = self { return true } else { return false } }

    /// The machine word of the state, for the socket row.
    public var key: String {
        switch self {
        case .notDownloaded: return "notDownloaded"
        case .downloading: return "downloading"
        case .ready: return "ready"
        case .failed: return "failed"
        }
    }

    /// The label of the one button, or nil while nothing can be pressed.
    public var button: String? {
        switch self {
        case .notDownloaded: return Words.text("Download model")   // a button takes the verb; "Download" is the noun
        case .failed: return Words.text("Retry")
        case .downloading, .ready: return nil
        }
    }

    public var words: String {
        switch self {
        case .notDownloaded:
            return Words.text("The search model is not on this Mac. Search by meaning, and the Sources of your replies, need it. About 1.5 GB, downloaded once.")
        case .downloading(let r, let t):
            return t > 0 ? Words.text("Downloading %@ of %@.", AttachmentSize.words(r), AttachmentSize.words(t)) : Words.text("Downloading %@.", AttachmentSize.words(r))
        case .ready(let b): return Words.text("Ready on this Mac (%@). Nothing leaves it.", AttachmentSize.words(b))
        case .failed(let reason): return reason
        }
    }
}
