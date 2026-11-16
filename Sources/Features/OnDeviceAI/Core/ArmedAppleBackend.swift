import Foundation

/// Apple Intelligence as a rig arms it: ready, or unavailable for one named reason. Only a rig copy builds one
/// (the pointer file lives in `$WHOOSH_SOCKET_DIR`, which the owner's copy never has), so the check can drive every
/// state on a Mac whatever its real Apple Intelligence state is. It never loads a model: a ready one answers with the
/// ready sentence, which is enough to see that the call went through.
public struct ArmedAppleBackend: TextBackend {
    public let name = Words.text("Apple Intelligence")
    public let reason: AppleReason?
    public init(reason: AppleReason?) { self.reason = reason }

    /// "ready" or an `AppleReason` raw value; anything else counts as `.other`.
    public static func armed(_ key: String) -> ArmedAppleBackend {
        key == "ready" ? ArmedAppleBackend(reason: nil) : ArmedAppleBackend(reason: AppleReason(rawValue: key) ?? .other)
    }

    public func availability() -> LocalModelAvailability {
        reason.map { .unavailable(reason: $0.words) } ?? .ready
    }

    public func respond(prompt: String, maxTokens: Int) async throws -> String {
        if let reason { throw LocalModelError.unavailable(reason.words) }
        return LocalModelAvailability.ready.words
    }
}
