import Foundation

/// The three calls the engine exports for its background download and indexing (core/crates/engine/src/crawl.rs).
/// One port, so the rest of this folder never names the generated functions and the check can read the one place that does.
public protocol BackgroundEngine: Sendable {
    func setConditions(batteryPercent: Int, charging: Bool, thermalSerious: Bool)
    func ownerActed()
    func setEmbedder(_ embedder: (any Embedder)?)
    /// model-late: at launch, before the look for the model's files: the answer (`setEmbedder`) follows, so a search typed meanwhile
    /// shows the word list and is ranked when the model comes, instead of "the model is not on this Mac".
    func expectEmbedder()
}

public extension BackgroundEngine {
    /// A port that never says it (a test's own engine) leaves the engine's old rule: no model handed = no model on this Mac.
    func expectEmbedder() {}
}

/// Where the Mac's battery and heat are read. The fixture verb `fixture power` swaps this for a scripted one.
public protocol PowerReading: Sendable {
    func read() -> PowerConditions
}

/// The numbers this folder owns until docs/CONSTANTS.md carries the rows (named once, here).
public enum BackgroundWorkConstants {
    /// A change of charge that crosses nothing reaches the engine at most this often (the owner's rule: never faster than once a minute).
    public static let CONDITIONS_MIN_INTERVAL_S: TimeInterval = 60
    /// The owner's key or click reaches the engine at most this often. The engine's quiet window (CRAWL_QUIET_MS 1500) is
    /// longer, so a call every 0.5 s keeps it quiet exactly as a call per key would.
    public static let OWNER_NOTE_MIN_INTERVAL_S: TimeInterval = 0.5
}
