import Foundation

/// Holds one expensive value (the loaded model) and lets it go after `idleSeconds` without use.
/// Pure Swift, no MLX: the unit tests drive it with a fake clock and no weights.
/// `idleSeconds` comes from the caller (WhooshConstants.EMBED_IDLE_UNLOAD_S); this file holds no number of its own.
/// Nothing loads at launch: the first `get` loads. While nothing is loaded no timer runs, so an idle app spends no CPU here.
public actor IdleHolder<Value: Sendable> {
    private var value: Value?
    private var loading: Task<Value, Error>?
    private var lastUse: TimeInterval = 0
    private var sweeper: Task<Void, Never>?
    private let idleSeconds: TimeInterval
    private let now: @Sendable () -> TimeInterval
    private let onUnload: @Sendable () -> Void
    private let onLoad: @Sendable () -> Void
    private let autoSweep: Bool
    public private(set) var loadCount = 0
    public private(set) var unloadCount = 0

    /// - Parameters:
    ///   - now: the clock; tests pass a fake one.
    ///   - onUnload: runs right after the value is dropped (the app clears the GPU buffer cache here).
    ///   - autoSweep: false in tests, which call `unloadIfIdle()` themselves.
    ///   - onLoad: runs right after a load finished (the app hands the loaded model to the background indexer here).
    public init(idleSeconds: Int,
                now: @escaping @Sendable () -> TimeInterval = { ProcessInfo.processInfo.systemUptime },
                autoSweep: Bool = true,
                onUnload: @escaping @Sendable () -> Void = {},
                onLoad: @escaping @Sendable () -> Void = {}) {
        self.idleSeconds = TimeInterval(idleSeconds)
        self.now = now
        self.autoSweep = autoSweep
        self.onUnload = onUnload
        self.onLoad = onLoad
    }

    public var isLoaded: Bool { value != nil }

    /// Returns the value, loading it on first use. Two callers arriving during one load share that load.
    public func get(load: @escaping @Sendable () async throws -> Value) async throws -> Value {
        if let value { lastUse = now(); return value }
        if let loading { return try await loading.value }
        let task = Task { try await load() }
        loading = task
        do {
            let made = try await task.value
            loading = nil
            value = made
            loadCount += 1
            lastUse = now()
            scheduleSweep()
            onLoad()
            return made
        } catch {
            loading = nil
            throw error
        }
    }

    /// Loads if needed, runs `body` with the value, and marks the value used when `body` ENDS, so a use that outlasts the
    /// idle time is not taken for idleness. This is the door the embedder uses.
    public func use<R: Sendable>(load: @escaping @Sendable () async throws -> Value,
                                 _ body: @Sendable (Value) async throws -> R) async throws -> R {
        let v = try await get(load: load)
        defer { touch() }
        return try await body(v)
    }

    /// Marks the value as used just now. Call it when a long use ends, so a slow embed is not taken for idleness.
    public func touch() { lastUse = now() }

    /// Drops the value when it has been idle for `idleSeconds` or more. Returns true when it did.
    @discardableResult
    public func unloadIfIdle() -> Bool {
        guard value != nil, now() - lastUse >= idleSeconds else { return false }
        value = nil
        unloadCount += 1
        sweeper?.cancel()
        sweeper = nil
        onUnload()
        return true
    }

    /// One sleeping task, woken when the idle time could be up; it sleeps again for the remainder if the value was used meanwhile.
    private func scheduleSweep() {
        guard autoSweep, sweeper == nil else { return }
        sweeper = Task { [weak self] in
            while !Task.isCancelled {
                guard let wait = await self?.secondsUntilIdle() else { return }
                try? await Task.sleep(nanoseconds: UInt64(max(wait, 1) * 1_000_000_000))
                if Task.isCancelled { return }
                if await self?.unloadIfIdle() == true { return }
            }
        }
    }

    private func secondsUntilIdle() -> TimeInterval? {
        guard value != nil else { return nil }
        return idleSeconds - (now() - lastUse)
    }
}
