import Foundation
#if canImport(FoundationModels)
import FoundationModels
#endif

/// Lane fix-aisort round 11: Apple's model refused the MAIL it was given, not the request as such: its content (the guardrail), its language,
/// or its size. Asking the same text again gives the same refusal. The raw values are ids written to core.log, never words for the owner.
public enum AppleMailRefusal: String, Error, Sendable {
    case unsafe
    case tooLong = "too_long"
    case language
}

#if canImport(FoundationModels)

/// Apple Foundation Models (macOS 26): the system language model Apple Intelligence ships, 4,096-token window.
public struct AppleIntelligenceBackend: TextBackend {
    public let name = Words.text("Apple Intelligence")
    public init() {}

    public func availability() -> LocalModelAvailability {
        guard #available(macOS 26.0, iOS 26.0, *) else {
            return .unavailable(reason: AppleReason.osTooOld.words)
        }
        return Self.systemAvailability()
    }

    /// Lane fix-aisort round 8: why the system model cannot answer now, nil when it can (the label judge's fallback reads the reason).
    public static func reason() -> AppleReason? {
        guard #available(macOS 26.0, iOS 26.0, *) else { return .osTooOld }
        switch SystemLanguageModel.default.availability {
        case .available: return nil
        case .unavailable(let r):
            switch r {
            case .deviceNotEligible: return .deviceNotEligible
            case .appleIntelligenceNotEnabled: return .notEnabled
            case .modelNotReady: return .modelNotReady
            @unknown default: return .other
            }
        @unknown default: return .other
        }
    }

    /// Lane fix-aisort round 8: one request with the session's own instructions (the label judge's measured form).
    public static func respond(instructions: String, prompt: String, maxTokens: Int) async throws -> String {
        guard #available(macOS 26.0, iOS 26.0, *) else { throw LocalModelError.unavailable(AppleReason.osTooOld.words) }
        let session = LanguageModelSession(instructions: instructions)
        do {
            return try await session.respond(to: prompt, options: GenerationOptions(maximumResponseTokens: maxTokens)).content
        } catch let e as LanguageModelSession.GenerationError {
            // round 11: the refusals that belong to the mail itself are typed, so the judge sets that mail aside instead of waiting for ever
            switch e {
            case .guardrailViolation: throw AppleMailRefusal.unsafe
            case .exceededContextWindowSize: throw AppleMailRefusal.tooLong
            case .unsupportedLanguageOrLocale: throw AppleMailRefusal.language
            default: throw LocalModelError.failed(Words.text("Apple Intelligence could not answer: %@", e.localizedDescription))
            }
        } catch {
            throw LocalModelError.failed(Words.text("Apple Intelligence could not answer: %@", error.localizedDescription))
        }
    }

    @available(macOS 26.0, iOS 26.0, *)
    static func systemAvailability() -> LocalModelAvailability {
        switch SystemLanguageModel.default.availability {
        case .available:
            return .ready
        case .unavailable(let reason):
            switch reason {
            case .deviceNotEligible:
                return .unavailable(reason: AppleReason.deviceNotEligible.words)
            case .appleIntelligenceNotEnabled:
                return .unavailable(reason: AppleReason.notEnabled.words)
            case .modelNotReady:
                return .unavailable(reason: AppleReason.modelNotReady.words)
            @unknown default:
                return .unavailable(reason: AppleReason.other.words)
            }
        @unknown default:
            return .unavailable(reason: AppleReason.other.words)
        }
    }

    public func respond(prompt: String, maxTokens: Int) async throws -> String {
        guard #available(macOS 26.0, iOS 26.0, *) else { throw LocalModelError.unavailable(availability().words) }
        return try await Self.run(prompt: prompt, maxTokens: maxTokens)
    }

    @available(macOS 26.0, iOS 26.0, *)
    static func run(prompt: String, maxTokens: Int) async throws -> String {
        let session = LanguageModelSession()
        let options = GenerationOptions(maximumResponseTokens: maxTokens)
        do {
            return try await session.respond(to: prompt, options: options).content
        } catch {
            throw LocalModelError.failed(Words.text("Apple Intelligence could not answer: %@", error.localizedDescription))
        }
    }
}
#else
public struct AppleIntelligenceBackend: TextBackend {
    public let name = Words.text("Apple Intelligence")
    public init() {}
    public func availability() -> LocalModelAvailability {
        .unavailable(reason: Words.text("This build of Whoosh cannot reach Apple Intelligence (the system framework is missing)."))
    }
    public func respond(prompt: String, maxTokens: Int) async throws -> String {
        throw LocalModelError.unavailable(availability().words)
    }
    public static func reason() -> AppleReason? { .other }
    public static func respond(instructions: String, prompt: String, maxTokens: Int) async throws -> String {
        throw LocalModelError.unavailable(Words.text("This build of Whoosh cannot reach Apple Intelligence (the system framework is missing)."))
    }
}
#endif
