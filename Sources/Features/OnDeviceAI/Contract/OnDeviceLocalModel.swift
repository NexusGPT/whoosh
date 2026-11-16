import Foundation

/// The LocalModel trait the core calls to generate text on this Mac: Apple Intelligence first.
/// The default is Apple Intelligence alone; `OnDeviceSetup.shared.localModel` is the one wired with the downloaded model too.
public final class OnDeviceLocalModel: LocalModel, @unchecked Sendable {
    public let engine: OnDeviceTextEngine
    public init(engine: OnDeviceTextEngine = OnDeviceTextEngine(backends: [AppleIntelligenceBackend()])) { self.engine = engine }
    public func generate(prompt: String, maxTokens: UInt32) throws -> String {
        try engine.respondBlocking(prompt: prompt, maxTokens: Int(maxTokens))
    }
}
