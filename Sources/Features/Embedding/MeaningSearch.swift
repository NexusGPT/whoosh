import Foundation
#if canImport(MLXEmbedders) && canImport(Tokenizers)
import MLX
#endif

/// The meaning index as the fixture core meets it (embedding round 4). One process-wide ranker over EmbeddingGemma 2:
/// the first call finds the weights (WHOOSH_EMBED_WEIGHTS for the rig, else the support folder the installer fills) and
/// loads the model; with no weights it answers nothing and the caller falls back to its own fixture rule.
/// Messages are embedded at the first search, or when the test engine loads a small corpus (`warm`), never at launch, and kept while the process lives;
/// the model itself unloads when idle.
public enum MeaningSearch {
    /// The ranker, the query encoder and the background builder over ONE embedder: one model instance serves all three.
    /// `nil` while the weights are absent. Built at the first call that finds the weights; while they are absent it looks again
    /// at each call (a few file sizes), so a model installed during the session starts working without a restart.
    private struct Parts { let ranker: MeaningRanker; let encoder: QueryEncoder; let builder: MeaningIndexBuilder }
    private static var parts: Parts? {
        #if canImport(MLXEmbedders) && canImport(Tokenizers)
        built.lock.lock(); defer { built.lock.unlock() }
        if let made = built.value { return made }
        guard let weights = GemmaWeights.locate() else { return nil }   // rig folder, installer folder, or a Hugging Face cache copy, in place
        let embedder = GemmaEmbedder(weights: weights, dimension: WhooshConstants.EMBED_DIM, idleUnloadSeconds: EmbedRigSwitches.idleUnloadSeconds,
                                     autoSweep: EmbedRigSwitches.mutant != "keep")
        let encoder = QueryEncoder(cacheLimit: WhooshConstants.QUERY_CACHE_ENTRIES) { try embedder.embedQuery(text: $0) }
        let ranker = MeaningRanker(embedder: embedder, encoder: encoder)
        let builder = MeaningIndexBuilder(ranker: ranker, release: { MLX.Memory.clearCache() }, onUpdate: { MeaningSearch.notifyUpdate() })
        let made = Parts(ranker: ranker, encoder: encoder, builder: builder)
        built.value = made
        return made
        #else
        return nil
        #endif
    }

    private final class Built: @unchecked Sendable { let lock = NSLock(); var value: Parts?; var handler: (@Sendable () -> Void)? }
    private static let built = Built()

    /// bg-hooks round 2: the ONE embedder (the same instance the ranker uses), for the shell's hand-over to the engine at launch.
    /// Building it loads nothing (the model loads on its first embed); `nil` while the weights are absent.
    public static var sharedEmbedder: (any Embedder)? { parts?.ranker.embedder }

    /// The weights are on this Mac (a few file sizes, no load).
    public static var weightsPresent: Bool {
        #if canImport(MLXEmbedders) && canImport(Tokenizers)
        return GemmaWeights.locate() != nil
        #else
        return false
        #endif
    }

    /// The ranker (tests and the old callers). `nil` while the weights are absent.
    public static var ranker: MeaningRanker? { parts?.ranker }

    /// One shared encoder for the search box: newest typed query wins, finished queries are cached. `nil` while the weights are absent.
    public static var queryEncoder: QueryEncoder? { parts?.encoder }

    /// How far the background build is: indexed, queued, building, paused for memory. Empty while there are no weights.
    public static var status: MeaningIndexBuilder.Status { parts?.builder.status ?? .init() }

    /// Called (off the main thread, at most every EMBED_NOTIFY_MS) when the build advanced or a typed query's vector arrived.
    /// The search screen sets it to ask the same question again, which then answers with the hits found so far.
    public static var onUpdate: (@Sendable () -> Void)? {
        get { built.lock.lock(); defer { built.lock.unlock() }; return built.handler }
        set { built.lock.lock(); built.handler = newValue; built.lock.unlock() }
    }
    /// The builder's announcement: the search screens' handler, and the one notification the index lines listen to (`IndexProgress.didChange`),
    /// carrying the status read here, on the builder's queue. Nothing is built or read when no builder exists.
    fileprivate static func notifyUpdate() {
        onUpdate?()
        built.lock.lock(); let builder = built.value?.builder; built.lock.unlock()
        guard let status = builder?.status else { return }
        NotificationCenter.default.post(name: IndexProgress.didChange, object: nil, userInfo: [IndexProgress.statusKey: status])
    }

    /// Kept for the shell's launch call. It loads NOTHING: the model loads on the first search and unloads after
    /// `EMBED_IDLE_UNLOAD_S` without use (docs/CONSTANTS.md), so an idle app holds no model (`RSS_IDLE_MAX_MB`).
    public static func preload() {
        // a TEST COPY under perf/footprint.py's broken copy `--mutant launch` only: the model loads at launch, so the idle reading must go red. The owner's
        // copy never reads the switch (EmbedRigSwitches.isTestCopy) and loads nothing here.
        guard EmbedRigSwitches.mutant == "launch" else { return }
        DispatchQueue.global(qos: .utility).async { _ = try? MeaningSearch.sharedEmbedder?.embedQuery(text: "launch") }
    }

    /// The test engine's index step: its one caller is `fixture corpus` (FixtureCore.loadCorpus), never the launch, so an idle app still holds no model.
    /// A corpus small enough for the first search to embed inline (at most EMBED_BATCH new vectors, the rule of `rank`) is embedded here, when it
    /// arrives, as the real engine's indexer embeds new mail before anyone searches it (J2.1 starts with the vector index built, J4.7). A larger corpus
    /// is left to the first search's background build, which the re-ask case reads. hid-search round 16: J2.1/first-screen-ms read 436 to 536 ms
    /// (main105 again-005, -006, -024), the model's load and 11 embeds inline in the first search of tests-j2.json; Return on a corpus already
    /// searched read 35.4 ms (again-023 par-first-result-rows-under-50-ms-after-return).
    public static func warm(_ rows: [(id: String, title: String?, body: String)]) {
        guard !rows.isEmpty, let parts else { return }
        let p = parts.ranker.beginPass()
        let missing = Set(parts.ranker.missing(in: rows.map(\.id), pass: p))
        guard !missing.isEmpty, missing.count <= WhooshConstants.EMBED_BATCH else { return }
        let todo = rows.filter { missing.contains($0.id) }.map { MeaningRanker.Row(id: $0.id, title: $0.title, text: MeaningRanker.text(body: $0.body)) }
        _ = try? parts.ranker.embedAndStore(todo, pass: p)
    }
    public static func warm(_ rows: [(id: String, body: String)]) {}

    /// EVERY embedded message among `ids`, closest in meaning to the typed question first, down to the farthest: no score cut-off and no
    /// count limit (SPEC A141: the search lists every mail, ranked). A message with no vector yet is not in the answer; the caller puts it after these.
    /// `ids` are the messages that exist now; `text` gives one message's subject and body ON DEMAND, so only one batch of text is
    /// alive at a time (the caller does not build a body string for every message).
    ///
    /// - A corpus that needs at most EMBED_BATCH new vectors is embedded here and now (a few messages: the answer is complete).
    /// - A larger one is embedded in the background in batches of EMBED_BATCH; this call returns the hits among the messages
    ///   embedded so far (none at first) without waiting for the model, and `onUpdate` fires as vectors arrive. The caller shows
    ///   its word-index hits and merges the meaning hits when it asks again.
    /// Empty when there is no model or it fails (the caller then shows its text matches alone, never an error for a missing helper).
    public static func rank(_ query: String, ids: [String],
                            text: @escaping @Sendable (String) -> (title: String?, body: String)?) -> [(id: String, score: Float)] {
        guard let parts else { return [] }
        let p = parts.ranker.beginPass()
        let missing = parts.ranker.missing(in: ids, pass: p)
        let fetch: MeaningIndexBuilder.Fetch = { id in text(id).map { (title: $0.title, text: MeaningRanker.text(body: $0.body)) } }
        let scored: [MeaningRanker.Scored]
        if missing.count <= WhooshConstants.EMBED_BATCH {
            let rows = missing.compactMap { id in fetch(id).map { MeaningRanker.Row(id: id, title: $0.title, text: $0.text) } }
            _ = try? parts.ranker.embedAndStore(rows, pass: p)
            guard let s = try? parts.ranker.score(query: query, pass: p, limit: nil) else { return [] }
            scored = s
        } else {
            parts.builder.submit(missing: missing, pass: p, fetch: fetch)
            if let q = parts.encoder.cachedVector(for: query) {
                scored = parts.ranker.scoreVector(q, pass: p, limit: nil)
            } else {
                // The vector comes from the builder's queue, between two batches; the caller is told when it is there.
                let ranker = parts.ranker
                parts.builder.perform { _ = try? ranker.score(query: query, pass: p, limit: 1); notifyUpdate() }   // embeds it and fills the encoder's cache
                return []
            }
        }
        return scored.map { ($0.id, $0.score) }
    }

    /// Rows that carry their text (the fixture core's shape today). The text of every row is alive while the call runs and
    /// while a background build is using it; prefer `rank(_:ids:text:)`, which asks for one message at a time.
    public static func rank(_ query: String, rows: [(id: String, title: String?, body: String)]) -> [(id: String, score: Float)] {
        let index = Dictionary(rows.enumerated().map { ($0.element.id, $0.offset) }, uniquingKeysWith: { first, _ in first })
        return rank(query, ids: rows.map(\.id)) { id in index[id].map { (title: rows[$0].title, body: rows[$0].body) } }
    }

    public static func rank(_ query: String, rows: [(id: String, body: String)]) -> [(id: String, score: Float)] {
        rank(query, rows: rows.map { (id: $0.id, title: nil, body: $0.body) })
    }
}

/// perf/footprint.py's test-only switches (round 25, item 7a30058b), read on a TEST COPY only (the run folder WHOOSH_SOCKET_DIR is set, as
/// AttachmentPlaces.isRig and MeaningModelRig read it); the owner's copy reads none of them and keeps the ledger's numbers.
/// WHOOSH_EMBED_IDLE_UNLOAD_S: the model unloads after this many seconds idle instead of EMBED_IDLE_UNLOAD_S (the gate's unload wait in seconds, not minutes).
/// WHOOSH_EMBED_MUTANT=keep: the model never unloads (no idle sweep), so the gate's "memory, model unloaded" goes red.
/// WHOOSH_EMBED_MUTANT=launch: the model loads at launch (`MeaningSearch.preload`), so the gate's "memory at idle" goes red.
enum EmbedRigSwitches {
    static var env: [String: String] { ProcessInfo.processInfo.environment }
    static var isTestCopy: Bool { !(env["WHOOSH_SOCKET_DIR"] ?? "").isEmpty }
    static var mutant: String { isTestCopy ? (env["WHOOSH_EMBED_MUTANT"] ?? "") : "" }
    static var idleUnloadSeconds: Int {
        guard isTestCopy, let s = env["WHOOSH_EMBED_IDLE_UNLOAD_S"], let n = Int(s), n > 0 else { return WhooshConstants.EMBED_IDLE_UNLOAD_S }
        return n
    }
}
