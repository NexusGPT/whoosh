import Foundation
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// The app's side of the engine's background download and indexing (core/crates/engine/src/crawl.rs, docs/lanes/sync-full-r1.md):
/// the battery and heat on each change, "the owner just acted" on each command, and the meaning model handed over while it is
/// loaded and taken back when it unloads. Every entry is thread-safe and returns at once; the engine calls run on utility queues.
public enum BackgroundWork {
    private final class Parts: @unchecked Sendable {
        let engine: any BackgroundEngine
        let conditions: BackgroundConditions
        let activity: OwnerActivity
        let handOver: EmbedderHandOver
        init(engine: any BackgroundEngine) {
            self.engine = engine
            conditions = BackgroundConditions(engine: engine)
            activity = OwnerActivity(engine: engine)
            handOver = EmbedderHandOver(engine: engine)
        }
    }
    private static let parts = Parts(engine: LiveBackgroundEngine())

    /// Reads the battery and heat, tells the engine, and listens for changes. Idempotent; the feature's launch hook calls it.
    public static func start() { _ = launched }
    // a search model download the app quit in the middle of continues at launch (MeaningModelStore.resumeIfInterrupted; lane hid-search round 14)
    private static let launched: Void = { parts.conditions.start(); parts.handOver.start(); MeaningModelStore.shared.resumeIfInterrupted() }()

    /// The owner pressed a key or clicked and a command ran. Cheap and never blocking: a counter, one clock read, and at most one
    /// hop per OWNER_NOTE_MIN_INTERVAL_S to a utility queue that calls the engine.
    public static func noteOwnerActivity() { parts.activity.note() }

    /// The model's files may have arrived (an install finished): look again and hand the embedder over if they are there. Cheap.
    public static func recheckModel() { parts.handOver.look() }

    /// The meaning model is in memory: the engine may embed with it. Used by `fixture embedder loaded` (the app hands the embedder over at launch).
    public static func embedderLoaded(_ embedder: any Embedder) { parts.handOver.loaded(embedder) }

    /// The engine gets the embedder back. Used by `fixture embedder unloaded`; the app keeps the embedder handed (the object holds no model).
    public static func embedderUnloaded() { parts.handOver.unloaded() }

    /// `fixture power <percent> <charging|battery> <nominal|fair|serious|critical>` and `fixture power real`.
    public static func simulatePower(_ reading: PowerConditions?) { parts.conditions.script(reading) }

    /// `fixture embedder loaded|unloaded`: the same hand-over path with the fixture's stub embedder.
    public static func simulateEmbedder(loaded: Bool) {
        if loaded { embedderLoaded(FixtureEmbedder()) } else { embedderUnloaded() }
    }
}

/// `note_owner_activity`, throttled. The engine's quiet window is longer than the throttle, so no activity is lost.
final class OwnerActivity: @unchecked Sendable {
    private let engine: any BackgroundEngine
    private let state: BackgroundState
    private let minInterval: TimeInterval
    private let clock: @Sendable () -> TimeInterval
    private let queue = DispatchQueue(label: "whoosh.background.activity", qos: .utility)
    private let lock = NSLock()
    private var lastCall: TimeInterval = -.infinity

    init(engine: any BackgroundEngine, state: BackgroundState = .shared,
         minInterval: TimeInterval = BackgroundWorkConstants.OWNER_NOTE_MIN_INTERVAL_S,
         clock: @escaping @Sendable () -> TimeInterval = { ProcessInfo.processInfo.systemUptime }) {
        self.engine = engine
        self.state = state
        self.minInterval = minInterval
        self.clock = clock
    }

    func note() {
        let now = clock()
        lock.lock()
        let due = now - lastCall >= minInterval
        if due { lastCall = now }
        lock.unlock()
        state.update { $0.ownerNotes += 1; if due { $0.ownerCalls += 1 } }
        guard due else { return }
        queue.async { [engine] in engine.ownerActed() }
    }
}

/// Hands the embedder to the engine ONCE, at launch, and says so when the model's files are not on this Mac. The embedder is an object
/// of a few bytes: the model inside loads when the engine first embeds a stored body and unloads after EMBED_IDLE_UNLOAD_S, so handing it
/// over loads nothing and an idle app holds no model. The engine calls it only in small steps while it has stored mail to embed, and not
/// at all while paused for battery or heat. All calls run on one utility queue, in order.
final class EmbedderHandOver: @unchecked Sendable {
    private let engine: any BackgroundEngine
    private let state: BackgroundState
    private let queue = DispatchQueue(label: "whoosh.background.embedder", qos: .utility)
    private let find: @Sendable () -> (any Embedder)?
    private let delayMs: @Sendable () -> Int
    private var observing = false

    /// `find` returns the app's one embedder when the weights are present (MeaningSearch.sharedEmbedder), else nil.
    init(engine: any BackgroundEngine, state: BackgroundState = .shared,
         find: @escaping @Sendable () -> (any Embedder)? = { MeaningSearch.sharedEmbedder },
         delayMs: @escaping @Sendable () -> Int = { EmbedderHandOver.rigDelayMs() }) {
        self.engine = engine
        self.state = state
        self.find = find
        self.delayMs = delayMs
    }

    /// Launch: look once, off the main thread, and look again when the app comes to the front (an install may have finished).
    /// model-late: the engine first hears, on the caller's thread and before the look, that the answer is coming: a search typed
    /// before the hand-over then shows the word list and is ranked when the model comes (one atomic store in the engine).
    func start() {
        engine.expectEmbedder()
        queue.async { [self] in
            if !observing {
                observing = true
                #if os(macOS)
                let becameActive = NSApplication.didBecomeActiveNotification
                #else
                let becameActive = UIApplication.didBecomeActiveNotification
                #endif
                NotificationCenter.default.addObserver(forName: becameActive, object: nil, queue: nil) { [weak self] _ in self?.look() }
            }
            let waitMs = delayMs()
            if waitMs > 0 { Thread.sleep(forTimeInterval: Double(waitMs) / 1000) }   // test copy only: the queue is held, so no look (the front-app one included) hands over early
            lookHere()
        }
    }

    /// The longest a test copy may hold the hand-over (a typo must not leave a test copy without its model for good).
    static let rigDelayMaxMs = WhooshConstants.HANDOVER_RIG_DELAY_MAX_MS   // Int; the ledger's row

    /// `WHOOSH_RIG_EMBED_HANDOVER_DELAY_MS` (item 26a6f568): how long a TEST COPY waits, on the utility queue, before it looks for the model, so a hidden
    /// run can type a search while the model has not arrived. Read from the environment of a test copy only (`CorePlace.isRig`): the owner's app
    /// gives 0 whatever the variable says, and so does an absent, empty, negative or non-numeric value. Capped at `rigDelayMaxMs`.
    static func rigDelayMs(env: [String: String] = ProcessInfo.processInfo.environment,
                           bundleId: String? = Bundle.main.bundleIdentifier, arguments: [String] = CommandLine.arguments) -> Int {
        guard CorePlace.isRig(env: env, bundleId: bundleId, arguments: arguments),
              let text = env["WHOOSH_RIG_EMBED_HANDOVER_DELAY_MS"], let ms = Int(text), ms > 0 else { return 0 }
        return min(ms, rigDelayMaxMs)
    }

    func look() { queue.async { [self] in lookHere() } }

    /// Runs on `queue` only.
    private func lookHere() {
        guard !state.snapshot().embedderHanded else { return }
        if let embedder = find() {
            state.update { $0.embedderHanded = true; $0.modelMissing = false; $0.handovers += 1 }
            engine.setEmbedder(embedder)
        } else if !state.snapshot().modelMissing {
            state.update { $0.modelMissing = true }
            engine.setEmbedder(nil)   // "no model on this Mac": the engine's Sources rows say so in words
        }
    }

    func loaded(_ embedder: any Embedder) {
        state.update { $0.embedderHanded = true; $0.modelMissing = false; $0.handovers += 1 }
        queue.async { [engine] in engine.setEmbedder(embedder) }
    }

    func unloaded() {
        state.update { $0.embedderHanded = false; $0.takebacks += 1 }
        queue.async { [engine] in engine.setEmbedder(nil) }
    }
}
