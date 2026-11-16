import Foundation

/// One way to run a prompt on this Mac. The engine below owns the checks every backend shares.
public protocol TextBackend: Sendable {
    var name: String { get }
    func availability() -> LocalModelAvailability
    func respond(prompt: String, maxTokens: Int) async throws -> String
}

/// Picks the first backend that is ready, refuses a prompt that cannot fit BEFORE the model is called,
/// and reports unavailable in words when no backend is ready. This is the logic Tests/Unit/OnDeviceAITests.swift proves.
public struct OnDeviceTextEngine: Sendable {
    public let backends: [any TextBackend]
    public let contextTokens: Int

    public init(backends: [any TextBackend], contextTokens: Int = OnDeviceLimits.contextTokens) {
        self.backends = backends
        self.contextTokens = contextTokens
    }

    /// The first ready backend's words, or every backend's reason, one sentence each.
    public func availability() -> LocalModelAvailability {
        if backends.contains(where: { $0.availability().isReady }) { return .ready }
        let reasons = backends.map { Words.text("%@: %@", $0.name, $0.availability().words) }
        return .unavailable(reason: reasons.isEmpty ? Words.text("No on-device model is set up on this Mac.") : reasons.joined(separator: " "))
    }

    /// A count that cannot be too low for ordinary text: characters over the pessimistic ratio.
    public static func estimatedTokens(_ text: String) -> Int {
        Int((Double(text.count) / OnDeviceLimits.charsPerTokenFloor).rounded(.up))
    }

    public func respond(prompt: String, maxTokens: Int) async throws -> String {
        guard let backend = backends.first(where: { $0.availability().isReady }) else {
            throw LocalModelError.unavailable(availability().words)
        }
        let need = Self.estimatedTokens(prompt) + maxTokens
        guard need <= contextTokens else {
            throw LocalModelError.promptTooLong(estimatedTokens: need, limit: contextTokens)
        }
        return try await backend.respond(prompt: prompt, maxTokens: maxTokens)
    }

    /// The synchronous door the LocalModel trait needs (generate is not async in the contract).
    public func respondBlocking(prompt: String, maxTokens: Int) throws -> String {
        final class Box: @unchecked Sendable { var result: Result<String, Error>? }
        let box = Box()
        let done = DispatchSemaphore(value: 0)
        let engine = self
        Task.detached {
            do { box.result = .success(try await engine.respond(prompt: prompt, maxTokens: maxTokens)) }
            catch { box.result = .failure(error) }
            done.signal()
        }
        done.wait()
        return try box.result!.get()
    }
}
