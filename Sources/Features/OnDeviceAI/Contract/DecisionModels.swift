import Foundation
#if canImport(MLX)
import MLX
#endif

/// The on-device decision models of the catalogue as the app runs them (SPEC A133): which one answers, where each stands on this Mac, the door
/// that downloads one, and the port the core asks. One adapter per family: Laya keeps its own (`LayaOnDevice`, `LayaModelStore`); a label
/// reader downloads through the checksum-verified installer (`ModelInstaller`) and answers through `LabelReaderEngine`.
///
/// - The model in use is the owner's saved choice (`DecisionModel.model`, kept by the core). It is mirrored here (`choose`) at launch and at
///   every save, so the port never asks the core from inside the core's own call.
/// - Nothing loads at launch. A model is read into memory on its first question and dropped after `EMBED_IDLE_UNLOAD_S` without use.
/// - Nothing leaves the Mac: the only network use is the download, on the owner's click. A test copy downloads from this Mac only.
public final class DecisionModels: @unchecked Sendable {
    public static let shared = DecisionModels()

    /// A model read into memory, as the port asks it: its family's engine behind one call.
    private struct Loaded: @unchecked Sendable { let decide: @Sendable (DecisionState, [DecisionQuestion]) async throws -> DecisionResult }

    /// Reads a line's files into memory with its family's engine (one adapter per family). Laya never comes here: it has its own store.
    private static func load(_ entry: DecisionModelEntry, folder: URL) async throws -> Loaded {
        switch entry.family {
        case .glide:
            let engine = try await GlideEngine.load(folder: folder, name: entry.id)
            return Loaded { state, questions in try await GlidePort.decide(engine: engine, state: state, questions: questions) }
        case .labelReader, .laya:
            let engine = try await LabelReaderEngine.load(folder: folder, name: entry.id)
            return Loaded { state, questions in try await LabelReaderPort.decide(engine: engine, state: state, questions: questions) }
        }
    }
    /// An error thrown while the weights were being read into memory (not while a question was answered).
    private struct LoadFailure: Error { let error: Error }

    private let lock = NSRecursiveLock()
    private var chosenId = DecisionModelCatalogue.laya.id   // the first line of `lines` (the default) until the saved choice is read
    private var installers: [String: ModelInstaller] = [:]
    /// What each line last announced: the installer reports every read, and only a CHANGE may be announced (a listener answers the
    /// announcement by reading the state again; announcing an unchanged state made the two call each other for ever, main74).
    private var announced: [String: LayaModelState] = [:]
    private var residencies: [String: ModelResidency<Loaded>] = [:]
    /// Rig copies only: the loopback address a model's files come from, and a stand-in file list (`useRigBase`).
    private var rigSpecs: [String: MlxModelSpec] = [:]

    /// A test copy on the real engine serves no fixture verb (`fixture provider-endpoint` needs the test engine): its check names ONE line
    /// and the loopback address of that line's files in the launch environment (`WHOOSH_RIG_MODEL_ID`, `WHOOSH_RIG_MODEL_BASE`), as
    /// `WHOOSH_RIG_LAYA_BASE` does for Laya. Found on main191: the real-weights run of a second model pressed Download with nothing staged,
    /// and the download asked the refusing address. The owner's copy ignores both names (`useRigBase` returns at once).
    private init() {
        let env = ProcessInfo.processInfo.environment
        if let id = env["WHOOSH_RIG_MODEL_ID"], let base = env["WHOOSH_RIG_MODEL_BASE"], !id.isEmpty, !base.isEmpty { useRigBase(id: id, base) }
    }

    // MARK: the choice

    /// The model that answers: the saved choice, or the catalogue's first line.
    public var chosen: DecisionModelEntry { lock.lock(); defer { lock.unlock() }; return line(id: chosenId) }

    /// The saved choice changed, or was read at launch: a nil or unknown id is the default.
    public func choose(_ id: String?) {
        lock.lock(); chosenId = line(id: id).id; lock.unlock()
    }

    // MARK: the lines of the list (the catalogue's, then the models the owner added through "Other model")

    /// The models added through the line "Other model", as `<ModelsFolder.root()>/added-models.json` keeps them; nil until first read.
    private var added: [ModelHub.Found]?

    /// Every line of the list, in its order: the catalogue's, then the added ones in the order they were added.
    public var lines: [DecisionModelEntry] { DecisionModelCatalogue.all + Self.heldHere + addedModels().map(Self.entry) }

    /// The held lines this copy lists (`DecisionModelCatalogue.held`): none on the owner's copy; on a test copy, the ones its launch
    /// names in `WHOOSH_RIG_HELD_MODELS` (ids, comma separated), so a check can run a model the list does not show yet.
    static let heldHere: [DecisionModelEntry] = {
        let env = ProcessInfo.processInfo.environment
        guard CorePlace.current(env: env).rig, let named = env["WHOOSH_RIG_HELD_MODELS"], !named.isEmpty else { return [] }
        let ids = Set(named.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) })
        return DecisionModelCatalogue.held.filter { ids.contains($0.id) }
    }()

    /// The line a saved choice names: a nil or unknown id is the catalogue's first line (a removed model's choice falls back to it).
    public func line(id: String?) -> DecisionModelEntry { let all = lines; return all.first { $0.id == id } ?? all[0] }

    /// Whether a line was added through "Other model" (it can be removed; a catalogue line cannot).
    public func isAdded(_ id: String) -> Bool { addedModels().contains { $0.id == id } }

    /// Whether the model at an address is already a line: an added one with its id, or a catalogue line whose page is that address.
    public func holds(_ a: ModelHub.Address) -> Bool {
        lines.contains { $0.id.lowercased() == a.id.lowercased() || $0.page.lowercased() == a.page.lowercased() }
    }

    /// The file the added models are kept in, across launches (a test copy's is under its run folder).
    static var addedFile: URL { ModelsFolder.root().appendingPathComponent("added-models.json") }

    private func addedModels() -> [ModelHub.Found] {
        lock.lock(); defer { lock.unlock() }
        if let a = added { return a }
        let read = FileManager.default.contents(atPath: Self.addedFile.path).flatMap { try? JSONDecoder().decode([ModelHub.Found].self, from: $0) } ?? []
        added = read
        return read
    }

    /// Reads the file again (`fixture reset`: the disk decides, as it does for a downloaded model).
    public func reloadAdded() {
        lock.lock(); added = nil; lock.unlock()
    }

    /// A model the hub check passed becomes the last line, kept on disk. False when it is already a line or the file could not be written.
    @discardableResult
    public func add(_ found: ModelHub.Found) -> Bool {
        let list = addedModels()
        guard !list.contains(where: { $0.id.lowercased() == found.id.lowercased() }) else { return false }
        return save(list + [found])
    }

    /// The button Remove of an added line: its download stops, its files and its entry go.
    public func remove(_ id: String) {
        guard let found = addedModels().first(where: { $0.id == id }) else { return }
        lock.lock(); let running = installers[id]; lock.unlock()
        running?.cancel()
        lock.lock()
        installers[id] = nil; announced[id] = nil; rigSpecs[id] = nil; residencies[id] = nil
        lock.unlock()
        try? FileManager.default.removeItem(at: ModelsFolder.root().appendingPathComponent(Self.addedFolder(found), isDirectory: true))
        save(addedModels().filter { $0.id != id })
        DispatchQueue.main.async { NotificationCenter.default.post(name: LayaModelStore.didChange, object: nil) }
    }

    @discardableResult
    private func save(_ list: [ModelHub.Found]) -> Bool {
        do {
            try FileManager.default.createDirectory(at: Self.addedFile.deletingLastPathComponent(), withIntermediateDirectories: true)
            let enc = JSONEncoder(); enc.outputFormatting = [.prettyPrinted, .sortedKeys]
            try enc.encode(list).write(to: Self.addedFile, options: .atomic)
        } catch { return false }
        lock.lock(); added = list; lock.unlock()
        return true
    }

    /// The folder of an added model under the models folder: `hub/<owner>/<name>` (its revision's files in a sub-folder).
    static func addedFolder(_ f: ModelHub.Found) -> String { "hub/\(f.owner)/\(f.name)" }

    /// An added model as a line of the list: a label reader, fetched from the hub at its revision, its files checked against the sizes and checksums
    /// the hub gave. Its licence is the tag's id ("" when none: the list says so in words); no index score; the size is what the download weighs.
    static func entry(_ f: ModelHub.Found) -> DecisionModelEntry {
        DecisionModelEntry(
            id: f.id, name: f.name, indexRow: "", family: .labelReader, parametersBillions: 0,
            downloadMB: Int((f.totalBytes + 500_000) / 1_000_000), licence: f.licence,
            page: "https://huggingface.co/\(f.owner)/\(f.name)", score: nil, scoreEdition: nil,
            base: ModelHub.currentBase.appendingPathComponent("\(f.owner)/\(f.name)/resolve/\(f.revision)/", isDirectory: true),
            files: f.files.map(\.path), checked: f.files,
            folder: addedFolder(f) + "/" + String(f.revision.prefix(8)), checkpoint: "")
    }

    // MARK: on this Mac

    /// The installer of a line whose download is checked against its checksums; nil for Laya (its own store).
    public func installer(for entry: DecisionModelEntry) -> ModelInstaller? {
        guard let pinned = entry.spec else { return nil }
        lock.lock(); defer { lock.unlock() }
        if let i = installers[entry.id] { return i }
        let spec = rigSpecs[entry.id] ?? pinned
        let i = ModelInstaller(spec: spec, directory: ModelInstaller.defaultDirectory(for: spec))
        // every surface that shows a model's line listens to the one announcement (LayaModelStore.didChange), on the next turn of the main queue
        let total = spec.totalBytes
        i.onChange = { [weak self] state in self?.moved(entry, to: Self.shown(state, total: total)) }
        installers[entry.id] = i
        return i
    }

    /// Where a line stands, in the four states every model row shows.
    public func state(of entry: DecisionModelEntry) -> LayaModelState {
        guard let installer = installer(for: entry) else { return LayaOnDevice.shared.state }
        return Self.shown(installer.refresh(), total: installer.spec.totalBytes)
    }

    /// A line's installer reported: announce a change once, and tell the core when the files of the model in use have just arrived.
    private func moved(_ entry: DecisionModelEntry, to now: LayaModelState) {
        lock.lock(); let before = announced[entry.id]; announced[entry.id] = now; lock.unlock()
        guard before != now else { return }
        DispatchQueue.main.async { NotificationCenter.default.post(name: LayaModelStore.didChange, object: nil) }
        if case .ready = now { arrived(entry) }
    }

    /// The installer's state as a model row shows it: a paused download is "not downloaded" (Download resumes it), the checksum pass is the
    /// end of the download, a damaged model is a failure with its sentence.
    static func shown(_ s: ModelInstallState, total: Int64) -> LayaModelState {
        switch s {
        case .notInstalled, .paused: return .notDownloaded
        case .downloading(let r, let t): return .downloading(received: r, total: t)
        case .verifying: return .downloading(received: total, total: total)
        case .ready(let b): return .ready(bytes: b)
        case .failed(let why), .corrupt(let why): return .failed(reason: why)
        }
    }

    /// Whether a line's files are on this Mac.
    public func isReady(_ entry: DecisionModelEntry) -> Bool {
        if case .ready = state(of: entry) { return true } else { return false }
    }

    /// The door that downloads a line once; call again to retry after a failure. A second call while one runs joins it.
    @discardableResult
    public func download(_ entry: DecisionModelEntry) async -> LayaModelState {
        guard let installer = installer(for: entry) else { return await LayaOnDevice.shared.download() }
        return Self.shown(await installer.start(), total: installer.spec.totalBytes)
    }

    /// The Cancel button: the partial files go, the line says "not downloaded".
    public func cancelDownload(_ entry: DecisionModelEntry) {
        guard let installer = installer(for: entry) else { LayaOnDevice.shared.cancelDownload(); return }
        installer.cancel()
    }

    /// The folder the engine loads a line from.
    func checkpointFolder(_ entry: DecisionModelEntry, installer: ModelInstaller) -> URL {
        entry.checkpoint.isEmpty ? installer.directory : installer.directory.appendingPathComponent(entry.checkpoint, isDirectory: true)
    }

    /// A line's files have just arrived: when it is the model in use, the core is told (as for Laya, `LayaOnDevice.weightsMoved`), so the
    /// mails that waited for the weights are asked now.
    private func arrived(_ entry: DecisionModelEntry) {
        guard chosen.id == entry.id else { return }
        LayaOnDevice.tellCoreTheWeightsArrived()
    }

    /// Rig copies only (`fixture provider-endpoint model:<id> <url>`, for a catalogue line or an added one): the files come from a loopback server of the check, never from the real
    /// host; a file `decision-model-<id>.json` in the run folder (`{"files":[{"path","bytes","sha256"}]}`) swaps the file list for small
    /// stand-ins. An empty address puts the pinned one back (which a test copy's fetcher refuses). The owner's copy ignores it.
    public func useRigBase(id: String, _ raw: String) {
        let env = ProcessInfo.processInfo.environment
        guard CorePlace.current(env: env).rig, let entry = lines.first(where: { $0.id == id }), let pinned = entry.spec else { return }
        lock.lock(); defer { lock.unlock() }
        installers[id] = nil; announced[id] = nil
        guard !raw.isEmpty else { rigSpecs[id] = nil; return }
        struct Fixture: Decodable { let files: [ModelFile] }
        let run = env["WHOOSH_SOCKET_DIR"] ?? ""
        // an added line's id ("hub:<owner>/<name>") names its file with "-" for ":" and "--" for "/" (a file name holds no slash)
        let fixture = id.replacingOccurrences(of: ":", with: "-").replacingOccurrences(of: "/", with: "--")
        let files = FileManager.default.contents(atPath: run + "/decision-model-\(fixture).json").flatMap { try? JSONDecoder().decode(Fixture.self, from: $0).files } ?? pinned.files
        rigSpecs[id] = MlxModelSpec(name: pinned.name, licence: pinned.licence, baseURL: ModelsFolder.loopbackOnly(raw), files: files, folderName: pinned.folderName)
    }

    // MARK: the port

    /// The core's port for the label AI: each question goes to the model in use. Laya keeps its router (Apple's on-device model for the Labels
    /// question when the system offers it, as before); a label reader answers every question itself: the owner chose it by name.
    public var port: DecisionPort { ChosenDecisionPort(models: self, laya: LayaOnDevice.shared.layaPort) }

    /// One decision by a label reader, blocking the calling thread (the core's filing thread) until the model has answered. Refuses in words,
    /// with the code that lets the core ask again later, when the files are not on this Mac.
    func decide(with entry: DecisionModelEntry, state: DecisionState, questions: [DecisionQuestion]) throws -> DecisionResult {
        guard let installer = installer(for: entry), installer.refresh().isReady else {
            throw LayaOnDevice.refusal(LayaOnDevice.notDownloadedWords, code: LayaOnDevice.notReadyCode)
        }
        let folder = checkpointFolder(entry, installer: installer)
        let residency = residency(for: entry.id)
        final class Box: @unchecked Sendable { var result: Result<DecisionResult, Error>? }
        let box = Box(), done = DispatchSemaphore(value: 0)
        Task.detached(priority: .utility) {
            do {
                let r = try await residency.use(load: {
                    do { return try await Self.load(entry, folder: folder) } catch { throw LoadFailure(error: error) }
                }) { loaded in
                    try await loaded.decide(state, questions)
                }
                box.result = .success(r)
            } catch { box.result = .failure(error) }
            done.signal()
        }
        done.wait()
        do { return try box.result!.get() }
        catch let e as LoadFailure { throw LayaOnDevice.refusal(LayaOnDevice.words(e.error), code: LayaOnDevice.loadFailedCode) }
        catch let e as CoreError { throw e }
        catch let e as LocalModelError { throw LayaOnDevice.refusal(e.description) }
        catch { throw LayaOnDevice.refusal(Words.text("The on-device decision model could not answer: %@", error.localizedDescription)) }
    }

    private func residency(for id: String) -> ModelResidency<Loaded> {
        lock.lock(); defer { lock.unlock() }
        if let r = residencies[id] { return r }
        let r = ModelResidency<Loaded>(onUnload: {
            #if canImport(MLX)
            MLX.Memory.clearCache()
            #endif
        })
        residencies[id] = r
        return r
    }
}

/// The port the core holds for provider "local": it reads the model in use at each call.
final class ChosenDecisionPort: DecisionPort, @unchecked Sendable {
    private let models: DecisionModels
    private let laya: DecisionPort
    init(models: DecisionModels, laya: DecisionPort) { self.models = models; self.laya = laya }

    func provider() -> DecisionProviderKind { .local }

    func decide(state: DecisionState, questions: [DecisionQuestion]) throws -> DecisionResult {
        let entry = models.chosen
        switch entry.family {
        case .laya: return try laya.decide(state: state, questions: questions)
        case .labelReader: return try models.decide(with: entry, state: state, questions: questions)
        case .glide: return try models.decide(with: entry, state: state, questions: questions)
        }
    }
}

/// The core's typed questions as a label reader takes them, and its probabilities as the core's answers. The conventions are Laya's port's
/// (`LayaDecisionPort`): a choice option is "label: description" and the answer returns the label; a score question gets three plain levels.
enum LabelReaderPort {
    /// The prefix of the checkpoint of an answer by a catalogue model other than Laya (the core's `MODEL_CHECKPOINT_PREFIX`, decision_model.rs).
    static let modelCheckpointPrefix = "model:"

    static func questions(_ questions: [DecisionQuestion]) -> [LabelReaderLayout.Question] {
        questions.map { q in
            switch q {
            case .choice(let id, let prompt, let options): return .choice(id: id, instructions: prompt, options: options.map(LayaDecisionPort.split))
            case .score(let id, let prompt): return .score(id: id, instructions: prompt, levels: LayaDecisionPort.scoreLevels)
            case .noul(let id, let prompt): return .noul(id: id, instructions: prompt)
            }
        }
    }

    /// One answer per question: a choice names its most probable label with every label's probability, a score is the mean level, a yes or
    /// no is yes from one half (the key "true" is the first option).
    static func answers(_ read: [LabelReaderEngine.Answer], to questions: [LabelReaderLayout.Question]) -> [DecisionAnswer] {
        zip(read, questions).map { a, q in answer(id: a.id, kind: q.kind.rawValue, keys: a.keys, probabilities: a.probabilities) }
    }

    /// One answer of the core from the probabilities of a question's keys, in request order (the kinds are "choice", "score", "noul").
    static func answer(id: String, kind: String, keys: [String], probabilities p: [Double]) -> DecisionAnswer {
        switch kind {
        case "choice":
            let best = p.indices.max { p[$0] < p[$1] } ?? 0
            return .choice(id: id, choice: keys[best], probabilities: Dictionary(zip(keys, p)) { first, _ in first })
        case "score":
            return .score(id: id, value: p.enumerated().reduce(0) { $0 + Double($1.offset) * $1.element })
        default:
            return .noul(id: id, yes: (p.first ?? 0) >= 0.5, probability: p.first ?? 0)
        }
    }

    static func decide(engine: LabelReaderEngine, state: DecisionState, questions: [DecisionQuestion]) async throws -> DecisionResult {
        let asked = Self.questions(questions)
        let read = try await engine.answer(state: LabelReaderLayout.state(fields: state.fields, text: state.text), questions: asked)
        // the checkpoint names the model that answered, "model:<catalogue id>": the core reads the id and files the mail as decided by that model
        return DecisionResult(answers: answers(read, to: asked), answeredBy: .local, checkpoint: modelCheckpointPrefix + engine.checkpointName)
    }
}

/// The core's typed questions as GLiDE takes them, and its probabilities as the core's answers. The conventions are the label reader's
/// (`LabelReaderPort`): a choice option is "label: description" and the answer returns the label; a score question gets three plain levels;
/// the answer's checkpoint names the model, "model:<catalogue id>".
enum GlidePort {
    static func questions(_ questions: [DecisionQuestion]) -> [GlideLayout.Question] {
        questions.map { q in
            switch q {
            case .choice(let id, let prompt, let options): return .choice(id: id, instructions: prompt, options: options.map(LayaDecisionPort.split))
            case .score(let id, let prompt): return .score(id: id, instructions: prompt, levels: LayaDecisionPort.scoreLevels)
            case .noul(let id, let prompt): return .noul(id: id, instructions: prompt)
            }
        }
    }

    static func decide(engine: GlideEngine, state: DecisionState, questions: [DecisionQuestion]) async throws -> DecisionResult {
        let asked = Self.questions(questions)
        let read = try await engine.answer(state: GlideLayout.state(fields: state.fields, text: state.text), questions: asked)
        let answers = zip(read, asked).map { a, q in LabelReaderPort.answer(id: a.id, kind: q.kind.rawValue, keys: a.keys, probabilities: a.probabilities) }
        return DecisionResult(answers: answers, answeredBy: .local, checkpoint: LabelReaderPort.modelCheckpointPrefix + engine.checkpointName)
    }
}
