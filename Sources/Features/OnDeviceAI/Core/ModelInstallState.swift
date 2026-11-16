import Foundation

/// Where the on-device language model stands, in a form the install step and the Settings row both read.
public enum ModelInstallState: Equatable, Sendable {
    case notInstalled
    case downloading(received: Int64, total: Int64)
    case paused(received: Int64, total: Int64)
    case verifying
    case ready(bytes: Int64)
    case failed(reason: String)
    case corrupt(reason: String)

    public var isReady: Bool { if case .ready = self { return true } else { return false } }
    public var isBusy: Bool {
        switch self { case .downloading, .verifying: return true; default: return false }
    }

    /// 0...1, or nil while no byte count means anything.
    public var fraction: Double? {
        switch self {
        case .downloading(let r, let t), .paused(let r, let t): return t > 0 ? min(1, Double(r) / Double(t)) : nil
        case .verifying, .ready: return 1
        default: return nil
        }
    }

    public static func size(_ bytes: Int64) -> String {
        AttachmentSize.words(bytes)   // the app's chosen language, never the system's
    }

    /// One sentence for the owner. Never empty. The wording "not installed yet" is the one every AI feature repeats.
    public var words: String {
        switch self {
        case .notInstalled: return Words.text("The on-device model is not installed yet. Set it up once and it runs on this Mac.")
        case .downloading(let r, let t): return Words.text("Downloading %@ of %@.", Self.size(r), Self.size(t))
        case .paused(let r, let t): return Words.text("Paused at %@ of %@. Resume when you are ready.", Self.size(r), Self.size(t))
        case .verifying: return Words.text("Checking the download against its checksum.")
        case .ready(let b): return Words.text("Ready on this Mac (%@). Nothing leaves it.", Self.size(b))
        case .failed(let reason), .corrupt(let reason): return reason
        }
    }
}
