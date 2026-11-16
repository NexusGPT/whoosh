import Foundation

/// How long the on-device model stays in memory: it loads on the first question, never at launch, and goes
/// after `WhooshConstants.EMBED_IDLE_UNLOAD_S` without use (docs/CONSTANTS.md holds the number; the ledger has no separate
/// on-device row, so the embedding one governs). Generic so the tests drive it with a fake value and a fake clock, no weights.
public struct ModelResidency<Value: Sendable>: Sendable {
    public let holder: IdleHolder<Value>

    public init(idleSeconds: Int = WhooshConstants.EMBED_IDLE_UNLOAD_S,
                now: @escaping @Sendable () -> TimeInterval = { ProcessInfo.processInfo.systemUptime },
                autoSweep: Bool = true,
                onUnload: @escaping @Sendable () -> Void = {}) {
        holder = IdleHolder(idleSeconds: idleSeconds, now: now, autoSweep: autoSweep, onUnload: onUnload)
    }

    /// Loads on first use, marks the model used again when `body` ends.
    public func use<R: Sendable>(load: @escaping @Sendable () async throws -> Value,
                                 _ body: @Sendable (Value) async throws -> R) async throws -> R {
        try await holder.use(load: load, body)
    }
    public var isLoaded: Bool { get async { await holder.isLoaded } }
    @discardableResult public func unloadIfIdle() async -> Bool { await holder.unloadIfIdle() }
}
