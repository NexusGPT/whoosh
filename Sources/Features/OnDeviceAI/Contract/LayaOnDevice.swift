import Foundation
#if canImport(MLX)
import MLX
#endif

/// The on-device decision model (Laya) as the core's label AI gets it: handed over once at launch (`CoreHost`: `openRustCoreWithDecision`), asked one
/// message at a time on the core's filing thread, never on the main actor.
///
/// - Nothing loads at launch. The weights are read into memory on the first question (`ModelResidency`) and dropped after `EMBED_IDLE_UNLOAD_S`
///   without use (the ledger's number "also the speech and on-device models"; a separate `LAYA_IDLE_UNLOAD_S` row is proposed in docs/lanes/labels-r5.md).
/// - Without the weights on this Mac the question is refused in words (`notDownloadedWords`) and nothing is loaded; `download()` is the door that
///   fetches them once (`LayaModelStore`, the lane's own download: Hugging Face, resumable, size-checked).
/// - Nothing leaves the Mac: the only network use is that one download, and it is the owner's click.
public final class LayaOnDevice: @unchecked Sendable {
    public static let shared = LayaOnDevice()

    public static var notDownloadedWords: String { Words.text("The on-device decision model is not on this Mac yet: download it in Settings, Decisions.") }

    /// Lane fix-aisort round 16: the codes on a refusal that only waits for the model, read by the core (`decision_model.rs` `LAYA_NOT_READY_CODE`,
    /// `LAYA_LOAD_FAILED_CODE`): such a failure never stops the core from asking again, so once the weights are here the next question answers
    /// (main114: a Re-run after the download skipped the AI). Any other refusal carries no code and blocks as before.
    static let notReadyCode = "laya_not_ready"
    static let loadFailedCode = "laya_load_failed"

    /// An error thrown while the weights were being read into memory (not while a question was answered).
    private struct LoadFailure: Error { let error: Error }

    private struct Loaded: @unchecked Sendable { let engine: LayaEngine }

    private let lock = NSRecursiveLock()   // re-entrant: a read of the state from inside the store's own making must not wait on itself
    private var _store: LayaModelStore?
    private let residency: ModelResidency<Loaded>
    /// Round 16: whether the weights were on this Mac at the last announcement of the store, and the watcher of those announcements.
    private var weightsWereHere = false
    private var watcher: NSObjectProtocol?

    public init() {
        residency = ModelResidency<Loaded>(onUnload: {
            #if canImport(MLX)
            MLX.Memory.clearCache()
            #endif
        })
        // Lane fix-aisort round 16 (main114: after the download a Re-run still judged nothing): every door that downloads the weights (Settings,
        // the label inspector, the AI setup list) and the first read that finds them at launch end in the store's announcement; when the weights
        // have just arrived the core is told, so the mails that waited for them are asked now, in the background (`weightsMoved`).
        watcher = NotificationCenter.default.addObserver(forName: LayaModelStore.didChange, object: nil, queue: nil) { [weak self] _ in self?.weightsMoved() }
    }

    /// The store moved: when the weights are here now and were not at the last move, the decision model in force is saved again (only when it is
    /// Laya: none saved yet, or Local), which the core answers by forgetting the earlier "not on this Mac" failure and asking the waiting mails
    /// (rust_core.rs `SaveDecisionModel`). The saved choice is the same one: an owner who chose Jev or rules is left as they chose.
    private func weightsMoved() {
        let here = isReady   // refresh(): the store announces only a change, so this read does not call back (LayaModelStore.set)
        lock.lock(); let arrived = here && !weightsWereHere; weightsWereHere = here; lock.unlock()
        guard arrived else { return }
        Self.tellCoreTheWeightsArrived()
    }

    /// The weights of an on-device decision model have just arrived on this Mac (Laya's, or the chosen model's: `DecisionModels`): the decision
    /// model in force is saved again when it is an on-device one, which the core answers by asking the mails that waited.
    static func tellCoreTheWeightsArrived() {
        DispatchQueue.global(qos: .utility).async {
            let host = CoreHost.shared
            guard host.kind == .rust else { return }
            let saved = host.core.decisionModel()
            guard saved == nil || saved?.provider == .local else { return }
            let model = saved ?? DecisionModel(provider: .local, model: nil, keychain: nil, decisionsEnabled: [], questionWords: [:], lastRunMs: nil)
            _ = try? host.core.run(command: .saveDecisionModel(model: model))
        }
    }

    /// The download store, made on first use (it reads its small manifest from disk, which launch does not need).
    public var modelStore: LayaModelStore {
        lock.lock(); defer { lock.unlock() }
        if let s = _store { return s }
        let s = LayaModelStore(); _store = s; return s
    }

    /// Rig copies only (`fixture provider-endpoint laya <url>`): the weights come from a loopback server of the check, never from the real host. An empty
    /// address puts the real one back; a non-loopback address is replaced by one that refuses. The owner's copy ignores it.
    public func useRigBase(_ raw: String) {
        guard CorePlace.current(env: ProcessInfo.processInfo.environment).rig else { return }
        lock.lock(); defer { lock.unlock() }
        _store = raw.isEmpty ? nil : LayaModelStore(baseURL: ModelsFolder.loopbackOnly(raw))
    }

    /// The Cancel button, for every surface that shows the download.
    public func cancelDownload() { modelStore.cancel() }

    /// Whether the weights are on this Mac (reads the manifest again).
    public var isReady: Bool { if case .ready = modelStore.refresh() { return true } else { return false } }

    /// The state the inspector shows: not downloaded, downloading with bytes, ready, or failed with its sentence and a retry.
    public var state: LayaModelState { modelStore.refresh() }

    /// The door that downloads the weights once, reporting progress; call again to retry after a failure.
    @discardableResult
    public func download(progress: @escaping @Sendable (LayaModelState) -> Void = { _ in }) async -> LayaModelState {
        await modelStore.download(progress: progress)
    }

    /// The core's port for the label AI. Lane fix-aisort round 8: the Labels question (per-label form) goes to Apple's on-device model when the
    /// system says it is available (`LabelJudgeRouter`, AppleLabelJudge.swift); every other question, and the Labels one when Apple's model is
    /// not available, goes to Laya as before. Laya is then not loaded for the labels.
    public var port: DecisionPort { DecisionModels.shared.port }

    /// Laya's own port with its router, the one `DecisionModels` asks when Laya is the model in use.
    var layaPort: DecisionPort { LabelJudgeRouter(laya: LayaLabelPort(owner: self)) }

    var loaded: Bool { get async { await residency.isLoaded } }

    /// One decision, blocking the calling thread (the core's filing thread) until the model has answered. Refuses in words when the weights are absent.
    func decide(state: DecisionState, questions: [DecisionQuestion]) throws -> DecisionResult {
        guard isReady else { throw Self.refusal(Self.notDownloadedWords, code: Self.notReadyCode) }
        final class Box: @unchecked Sendable { var result: Result<DecisionResult, Error>? }
        let box = Box(), done = DispatchSemaphore(value: 0)
        let folder = modelStore.modelFolder, residency = residency
        Task.detached(priority: .utility) {
            do {
                let r = try await residency.use(load: {
                    do { return Loaded(engine: try await LayaEngine.load(folder: folder)) } catch { throw LoadFailure(error: error) }
                }) { loaded in
                    try LayaDecisionPort(engine: loaded.engine).decide(state: state, questions: questions)
                }
                box.result = .success(r)
            } catch { box.result = .failure(error) }
            done.signal()
        }
        done.wait()
        do { return try box.result!.get() }
        catch let e as LoadFailure { throw Self.refusal(Self.words(e.error), code: Self.loadFailedCode) }
        catch let e as CoreError { throw e }
        catch let e as LocalModelError { throw Self.refusal(e.description) }
        catch { throw Self.refusal(Words.text("The on-device decision model could not answer: %@", error.localizedDescription)) }
    }

    static func refusal(_ words: String, code: String? = nil) -> CoreError {
        .Action(failure: ActionFailed(accountName: "Laya", reasonWords: words, retryable: false, code: code))
    }

    /// The words of a load failure, as `decide` says any other failure.
    static func words(_ error: Error) -> String {
        if let e = error as? LocalModelError { return e.description }
        if case .Action(let f)? = error as? CoreError { return f.reasonWords }
        return Words.text("The on-device decision model could not answer: %@", error.localizedDescription)
    }
}

/// The `DecisionPort` the core calls for provider "local" (the label AI): typed questions in, verdicts with probabilities out, on this Mac.
public final class LayaLabelPort: DecisionPort, @unchecked Sendable {
    private let owner: LayaOnDevice
    init(owner: LayaOnDevice) { self.owner = owner }

    public func provider() -> DecisionProviderKind { .local }

    public func decide(state: DecisionState, questions: [DecisionQuestion]) throws -> DecisionResult {
        try owner.decide(state: state, questions: questions)
    }
}
