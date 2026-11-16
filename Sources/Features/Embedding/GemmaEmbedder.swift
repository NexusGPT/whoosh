// EmbeddingGemma 2 behind the contract's Embedder trait.
// Start: mlx-swift-lm (MIT, Apple) MLXEmbedders `embedding_gemma2`. The behaviour copies the owner's
// launch-search project (server/adapters/embed/gemma.py): same model and revision, document form
// "title: none | text: ", query form "task: search result | query: ", unit-length output.
// Idle unload (IdleHolder) and the batch-retry pattern follow searchbetter (MIT, Copyright (c) 2026 Krishna Goutham),
// app/Sources/Indexer.swift:247 (stop the model after idle time) and :312-313 (retry a failed batch one text at a time).
// Compiled only when the MLX packages are in the build (see docs/lanes/embedding.md, "Package.swift lines").
#if canImport(MLXEmbedders) && canImport(Tokenizers)
import Foundation
import MLX
import MLXEmbedders
import MLXLMCommon
import MLXHuggingFace
import Tokenizers

public final class GemmaEmbedder: BatchTitledEmbedder, ModelFolderNaming, @unchecked Sendable {
    public static let documentPrefix = "title: none | text: "
    public static let queryPrefix = "task: search result | query: "
    /// Texts per forward pass; padded to the longest of the batch.
    static let batchSize = 8

    private let weights: GemmaWeights
    private let outputDimension: Int
    /// The loaded model. Loads on the first embed, never at launch; drops after `EMBED_IDLE_UNLOAD_S` without use.
    private let holder: IdleHolder<ContainerBox>
    private struct ContainerBox: @unchecked Sendable { let container: EmbedderModelContainer }
    /// pieces (SPEC A151): the inputs the model had to shorten to `CHUNK_TOKENS` tokens (the cut stays, as a safety).
    private let inputCut = InputCut()

    /// pieces: how many inputs this model shortened since it was made: 0 when the engine's pieces fit (the lead's check asserts 0).
    /// The engine reads it once the contract carries it (docs/lanes/pieces.md, "contract addition").
    public var cutInputs: Int { inputCut.cutCount }

    /// pieces: the folder the weights load from (a rig's symbolic link resolved), handed to the engine with the model.
    public var modelFolder: String { weights.directory.resolvingSymlinksInPath().path }

    /// `dimension` comes from the caller (WhooshConstants.EMBED_DIM); this file holds no number of its own.
    /// `now` and `autoSweep` exist for the tests (a fake clock, no sleeping task); the app uses the defaults.
    public init(weights: GemmaWeights = .inSupportFolder(), dimension: Int,
                idleUnloadSeconds: Int = WhooshConstants.EMBED_IDLE_UNLOAD_S,
                now: @escaping @Sendable () -> TimeInterval = { ProcessInfo.processInfo.systemUptime },
                autoSweep: Bool = true) {
        self.weights = weights
        self.outputDimension = dimension
        // bg-hooks round 2: the engine is handed this object once, at launch (BackgroundWork.start), and keeps it: the object is a
        // few bytes, the model inside loads on the first embed and unloads after idleUnloadSeconds (the IdleHolder), so nothing here
        // tells the engine about loads and unloads any more.
        self.holder = IdleHolder(idleSeconds: idleUnloadSeconds, now: now, autoSweep: autoSweep, onUnload: { MLX.Memory.clearCache() })
        MLXFailureGuard.install()   // a GPU failure (a timed-out command buffer) becomes a thrown error, never the app's end
    }

    /// Drops the model now if it has been idle long enough (what the sleeping task does by itself). For the tests.
    @discardableResult public func unloadIfIdle() async -> Bool { await holder.unloadIfIdle() }

    /// True while the model sits in memory (for the memory checks).
    public var isModelLoaded: Bool { get async { await holder.isLoaded } }

    public func dimension() -> UInt32 { UInt32(outputDimension) }

    /// Document vectors without a title: `title: none | text: <text>`.
    public func embed(texts: [String]) throws -> [[Float]] { try embedTitled(title: nil, texts: texts) }

    /// Document vectors for the chunks of ONE document under its title (delta 12): the mail's subject goes in the title slot,
    /// `title: <title> | text: <chunk>`; nil or blank gives `title: none`. Blocks the calling thread: the trait is synchronous.
    public func embedTitled(title: String?, texts: [String]) throws -> [[Float]] {
        let t0 = CFAbsoluteTimeGetCurrent(); defer { EmbedTimes.note(.document, since: t0, texts: texts.count) }   // perf-archive r6: two clock reads
        return try embedSync(texts.map { DocumentForm.text(title: title, text: $0) })
    }

    /// Many mails, each under its own title, in padded forward passes of `batchSize` texts. Each text is cut to `CHUNK_TOKENS`
    /// tokens (a safety: the engine's pieces fit, and a cut is counted in `cutInputs`), so a pass holds at most batchSize x CHUNK_TOKENS tokens.
    public func embedDocumentBatch(_ items: [(title: String?, text: String)]) throws -> [[Float]] {
        let t0 = CFAbsoluteTimeGetCurrent(); defer { EmbedTimes.note(.document, since: t0, texts: items.count) }   // perf-archive r6: two clock reads
        return try embedSync(items.map { DocumentForm.text(title: $0.title, text: $0.text) })
    }

    /// Query vector (the form a search box sends): `task: search result | query: <text>`.
    /// This is the generated `Embedder.embedQuery(text:)` requirement (contracts delta 8): the vector search embeds the
    /// typed question here, not through `embed(texts:)`. Media vectors (image, audio) stay in the probe until delta 9.
    public func embedQuery(text: String) throws -> [Float] {
        let t0 = CFAbsoluteTimeGetCurrent(); defer { EmbedTimes.note(.query, since: t0, texts: 1) }   // perf-archive r6: two clock reads
        return try embedSync([Self.queryPrefix + text])[0]
    }

    // MARK: sync bridge

    private func embedSync(_ prefixed: [String]) throws -> [[Float]] {
        let box = ResultBox()
        let done = DispatchSemaphore(value: 0)
        Task.detached { [self] in
            do { box.value = .success(try await embedAsync(prefixed)) } catch { box.value = .failure(error) }
            done.signal()
        }
        done.wait()
        // query-embed: the engine logs these words; a failed step says which step (EmbedStepFailure), never the embedded text
        switch box.value {
        case .success(let vectors): return vectors
        case .failure(let error): throw EmbedStepFailure.coreError(error)
        case nil: throw EmbedStepFailure.coreError(nil)
        }
    }

    private final class ResultBox: @unchecked Sendable { var value: Result<[[Float]], Error>? }

    // MARK: model

    private static func load(_ weights: GemmaWeights) async throws -> ContainerBox {
        // No download here: the installer (checksummed, resumable, checks free space) is the one door for the weights.
        guard weights.isComplete else {
            throw GemmaEmbeddingError.weightsMissing(reasonWords: "The search model is not installed yet. Install it from Settings.")
        }
        // round 24 (perf/footprint.py on main112: the model-loaded footprint 2445 MB against RSS_MODEL_MAX_MB 2200, steady through the unload wait, while
        // the resident size read 911 MB): MLX keeps every buffer a forward pass frees in its cache, and batches of different padded lengths rarely reuse one,
        // so the cache grew to about 0.9 GB beside the 1.49 GB of weights. It is capped before the model loads; buffers past the cap go back to the system.
        MLX.Memory.cacheLimit = WhooshConstants.EMBED_MLX_CACHE_MB * 1024 * 1024
        do {
            // the folder resolved first: a rig's WHOOSH_EMBED_WEIGHTS may be a symbolic link to the folder, and the library lists the
            // folder for its weight files (Load.swift `topLevelSafetensorURLs`); the resolved folder lists the same files either way
            let made = try await EmbedderModelFactory.shared.loadContainer(
                from: weights.directory.resolvingSymlinksInPath(), using: #huggingFaceTokenizerLoader())
            return ContainerBox(container: made)
        } catch {
            // query-embed: the library's own words, and the folder facts that tell a hidden run's causes apart (only on failure)
            throw EmbedStepFailure(step: .load, detail: "\(String(describing: error)) [\(EmbedStepFailure.folderFacts(weights.directory))]")
        }
    }

    /// Mean pooling and the final projection happen inside the model (pooledOutput), already unit length at 768.
    func embedAsync(_ prefixed: [String]) async throws -> [[Float]] {
        let weights = self.weights, dim = outputDimension, cut = inputCut
        // `use` marks the model used when the embedding ENDS, so a long batch is not taken for idleness.
        return try await holder.use(load: { try await Self.load(weights) }) { box in
            try await Self.run(box.container, prefixed, dim, cut)
        }
    }

    private static func run(_ container: EmbedderModelContainer, _ prefixed: [String], _ dim: Int, _ cut: InputCut) async throws -> [[Float]] {
        var out: [[Float]] = []
        for start in stride(from: 0, to: prefixed.count, by: Self.batchSize) {
            let batch = Array(prefixed[start ..< min(start + Self.batchSize, prefixed.count)])
            let full: [[Float]] = try await container.perform { context -> [[Float]] in
                // pieces: every input is cut to the model's cap (a safety); each one that was longer is counted
                let ids = cut.apply(batch.map { context.tokenizer.encode(text: $0, addSpecialTokens: true) }, limit: WhooshConstants.CHUNK_TOKENS)
                // query-embed: each step that can fail says so in words (no text of the batch in them)
                if let empty = ids.firstIndex(where: \.isEmpty) {
                    throw EmbedStepFailure(step: .tokenise, detail: "text \(empty + 1) of \(batch.count) gave no token")
                }
                let longest = ids.map(\.count).max() ?? 1
                let pad = context.tokenizer.eosTokenId ?? 0
                do {
                    // MLX's own error comes back as a thrown error here instead of stopping the process
                    let mark = MLXFailureGuard.failures
                    let vectors = try await withError { () throws -> [[Float]] in
                        let padded = stacked(ids.map { MLXArray($0 + Array(repeating: pad, count: longest - $0.count)) })
                        let mask = stacked(ids.map { MLXArray(Array(repeating: true, count: $0.count) + Array(repeating: false, count: longest - $0.count)) })
                        let result = context.pooling(
                            context.model(padded, positionIds: nil, tokenTypeIds: nil, attentionMask: mask), normalize: true)
                        result.eval()
                        return result.map { $0.asArray(Float.self) }
                    }
                    // a failure the library reported from another thread (a command buffer that timed out on the GPU) went to the
                    // process-wide handler (MLXFailureGuard): the vectors of this pass are not trusted, the pass fails and its caller retries
                    try MLXFailureGuard.check(since: mark)
                    return vectors
                } catch {
                    throw EmbedStepFailure(step: .run, detail: String(describing: error))
                }
            }
            for v in full { out.append(try MatryoshkaVector.truncate(v, to: dim)) }   // badVector: the `check` step (EmbedStepFailure.coreError)
        }
        return out
    }
}
#endif
