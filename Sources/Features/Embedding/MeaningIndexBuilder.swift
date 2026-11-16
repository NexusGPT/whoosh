import Foundation

/// Builds the meaning index in the background, one bounded batch per step, on its own serial queue (round 8).
/// The first meaning search over a large corpus used to embed every row on the caller's thread in one go; at 300,000 rows the
/// footprint passed 1 GB and the app was gone. Now `submit` only records ids (12 MB for 300,000), and each step takes at most
/// `batchRows` of them, fetches their text, embeds, stores, and lets go of the text. Queries run on the same queue between steps.
///
/// Memory plan (all counted, none assumed): vectors EMBED_DIM bytes per message in a flat int8 array (154 MB at 300,000);
/// ids 12 MB; one batch of text and its activations (batchRows x CHUNK_TOKENS tokens at most); the model itself. Before each
/// step the footprint is read: above `ceilingMB` the step first releases the framework's buffer cache, and if that is not
/// enough it pauses `pauseMs` and tries again. It never starts a step above the ceiling, so the build cannot push the app past
/// RSS_MODEL_MAX_MB by itself.
public final class MeaningIndexBuilder: @unchecked Sendable {
    public struct Status: Equatable, Sendable {
        public var indexed = 0          // messages with a vector now
        public var queued = 0           // ids still waiting for a step
        public var building = false
        public var pausedForMemory = false
        public var failed = 0           // texts the model refused alone
    }

    public typealias Fetch = @Sendable (String) -> (title: String?, text: String)?

    private let ranker: MeaningRanker
    private let batchRows: Int
    private let ceilingMB: Int
    private let pauseMs: Int
    private let notifyMs: Int
    private let footprint: @Sendable () -> Int?
    private let release: @Sendable () -> Void
    private let onUpdate: @Sendable () -> Void
    private let now: @Sendable () -> TimeInterval
    let queue: DispatchQueue
    private let lock = NSLock()
    private var waiting: [String] = []
    private var fetch: Fetch?
    private var pass: UInt32 = 0
    private var running = false
    private var lastNotify: TimeInterval = 0
    private var _status = Status()

    /// Every number comes from the ledger (docs/CONSTANTS.md): EMBED_BATCH, EMBED_INDEX_CEILING_MB, EMBED_PAUSE_MS, EMBED_NOTIFY_MS.
    public init(ranker: MeaningRanker,
                batchRows: Int = WhooshConstants.EMBED_BATCH,
                ceilingMB: Int = WhooshConstants.EMBED_INDEX_CEILING_MB,
                pauseMs: Int = WhooshConstants.EMBED_PAUSE_MS,
                notifyMs: Int = WhooshConstants.EMBED_NOTIFY_MS,
                footprint: @escaping @Sendable () -> Int? = { Footprint.physicalMB() },
                release: @escaping @Sendable () -> Void = {},
                now: @escaping @Sendable () -> TimeInterval = { ProcessInfo.processInfo.systemUptime },
                queue: DispatchQueue = DispatchQueue(label: "whoosh.meaning-index", qos: .utility),
                onUpdate: @escaping @Sendable () -> Void = {}) {
        self.ranker = ranker; self.batchRows = max(1, batchRows); self.ceilingMB = ceilingMB; self.pauseMs = pauseMs
        self.notifyMs = notifyMs; self.footprint = footprint; self.release = release; self.now = now; self.queue = queue; self.onUpdate = onUpdate
    }

    public var status: Status { lock.lock(); defer { lock.unlock() }; var s = _status; s.indexed = ranker.indexedCount; return s }

    /// Replaces the waiting list with the ids that have no vector yet and starts stepping. Returns at once. A newer submit
    /// (the corpus changed) replaces the older one; ids already embedded are not asked for again.
    public func submit(missing ids: [String], pass p: UInt32, fetch: @escaping Fetch) {
        lock.lock()
        waiting = ids; self.fetch = fetch; pass = p; _status.queued = ids.count
        let start = !running && !ids.isEmpty
        if start { running = true; _status.building = true }
        lock.unlock()
        if start { queue.async { [self] in step() } }
        notify(force: true)   // the start (and a new list) is announced at once, so a line can say "indexing" before the first batch ends
    }

    /// Drops the waiting list (the corpus was replaced, or the app is closing). A step already running finishes.
    public func cancel() { lock.lock(); waiting = []; _status.queued = 0; lock.unlock(); notify(force: true) }

    /// Runs `work` on the builder's queue, between two steps. A typed query is embedded here, so it waits for at most one batch.
    public func perform(_ work: @escaping @Sendable () -> Void) { queue.async(execute: work) }

    func step() {
        lock.lock()
        guard !waiting.isEmpty, let fetch else { running = false; _status.building = false; _status.queued = 0; _status.pausedForMemory = false; lock.unlock(); notify(force: true); return }
        lock.unlock()

        if let mb = footprint(), mb >= ceilingMB {
            release()
            if let after = footprint(), after >= ceilingMB {
                lock.lock(); _status.pausedForMemory = true; lock.unlock()
                notify(force: true)
                queue.asyncAfter(deadline: .now() + .milliseconds(pauseMs)) { [self] in step() }
                return
            }
        }

        lock.lock()
        let ids = Array(waiting.prefix(batchRows))
        waiting.removeFirst(ids.count)
        let p = pass
        _status.queued = waiting.count; _status.pausedForMemory = false
        lock.unlock()

        let rows = ids.compactMap { id in fetch(id).map { MeaningRanker.Row(id: id, title: $0.title, text: $0.text) } }
        let mark = MLXFailureGuard.failures
        let stored = (try? ranker.embedAndStore(rows, pass: p)) ?? 0
        if stored < rows.count { lock.lock(); _status.failed += rows.count - stored; lock.unlock() }
        notify(force: false)
        // the GPU failed during this step (a timed-out command buffer, recorded by MLXFailureGuard instead of ending the app): the failed batch was
        // already retried one text at a time (MeaningRanker.embedBatch); the cache is let go and the next step waits `pauseMs`. A text left without
        // a vector stays missing and is asked for again at the next search.
        if MLXFailureGuard.failures != mark {
            release()
            queue.asyncAfter(deadline: .now() + .milliseconds(pauseMs)) { [self] in step() }
            return
        }
        queue.async { [self] in step() }
    }

    private func notify(force: Bool) {
        let t = now()
        lock.lock()
        let due = force || t - lastNotify >= Double(notifyMs) / 1000
        if due { lastNotify = t }
        lock.unlock()
        if due { onUpdate() }
    }
}
