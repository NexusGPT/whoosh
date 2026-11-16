import Foundation

/// Mouse uses and key uses of one action: the shape of the contract record ShortcutUsage (shortcut-usage).
public struct UsageCount: Sendable, Equatable {
    public var mouse = 0
    public var key = 0
}

/// Where the preset choice and the usage counters live. The data map puts both in the core (tables
/// shortcut_preset and shortcut_usage); the contract has no write command for them yet, so the engine runs on
/// the memory store below and a core-backed store replaces it without touching the engine.
@MainActor
public protocol ShortcutStore: AnyObject {
    var presetId: String { get set }
    /// the owner's own changes: command id -> chord, applied over the Superhuman map
    var overrides: [String: String] { get set }
    func bump(action: String, key: Bool)
    func usage() -> [String: UsageCount]
}

@MainActor
public final class MemoryShortcutStore: ShortcutStore {
    public var presetId = Presets.superhuman.id
    public var overrides: [String: String] = [:]
    private var counts: [String: UsageCount] = [:]

    public init() {}

    public func bump(action: String, key: Bool) {
        var c = counts[action] ?? UsageCount()
        if key { c.key += 1 } else { c.mouse += 1 }
        counts[action] = c
    }

    public func usage() -> [String: UsageCount] { counts }
}
