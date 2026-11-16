import Foundation

/// Whether a local text model can answer right now, in words the Settings screen shows as they are.
public enum LocalModelAvailability: Equatable, Sendable {
    case ready
    case unavailable(reason: String)

    public var isReady: Bool { if case .ready = self { return true } else { return false } }
    /// One sentence for the owner. Never empty.
    public var words: String {
        switch self {
        case .ready: return Words.text("Ready. The model runs on this Mac and nothing leaves it.")
        case .unavailable(let reason): return reason
        }
    }
}

public enum LocalModelError: Error, Equatable, CustomStringConvertible, LocalizedError {
    case unavailable(String)
    case promptTooLong(estimatedTokens: Int, limit: Int)
    case failed(String)

    public var description: String {
        switch self {
        case .unavailable(let words): return words
        case .promptTooLong(let n, let limit):
            return Words.text("This prompt is about %@ tokens and the on-device model reads at most %@ (prompt and answer together). Shorten it and try again.", n, limit)
        case .failed(let words): return words
        }
    }
    public var errorDescription: String? { description }
}
