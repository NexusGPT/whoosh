import Foundation

/// The one store for the search model, shaped like `LayaModelStore`: notDownloaded, downloading(received, total), ready, failed(sentence).
/// It reuses `ModelInstaller` (resumable, SHA-256 against the pinned spec, free-space check): that installer already knew this spec
/// (`MlxModelSpec.embeddingGemma2`), but nothing ever started it.
///  - the state is `downloading(0, total)` the moment `download()` is called, before the first byte;
///  - while a file arrives the bytes are read every `tickNs` (250 ms) from what is on disk (finished files plus the `.part` of the file in flight);
///  - one download at a time; a read during a download answers with the progress; a failure keeps its sentence until the next try;
///  - when the download ends ready, `onReady` runs (the app hands the model to the engine without a relaunch).
public final class MeaningModelStore: @unchecked Sendable {
    public static let tickNs: UInt64 = 250_000_000
    public static let shared = MeaningModelStore(rig: .current(), onReady: { BackgroundWork.recheckModel() })
    /// Posted (object: the store, any thread, on the next turn of the main queue) each time the state CHANGES, whoever started the download:
    /// every surface that shows the search model follows it and nothing polls. While nothing downloads the store is silent: its only timer
    /// lives inside `download()`. Only a change is announced (a listener that reads the state again must not wake itself).
    public static let didChange = Notification.Name("whoosh.meaning.download")

    public let spec: MlxModelSpec
    public let directory: URL
    private let blocked: String?
    private let installer: ModelInstaller
    private let tick: UInt64
    private let onReady: @Sendable () -> Void
    private let lock = NSLock()
    private var _state: MeaningModelState = .notDownloaded
    private var _running = false

    public init(rig: MeaningModelRig, tickNs: UInt64 = MeaningModelStore.tickNs, onReady: @escaping @Sendable () -> Void = {}) {
        spec = rig.spec; directory = rig.directory; blocked = rig.blocked; tick = tickNs; self.onReady = onReady
        installer = ModelInstaller(spec: rig.spec, directory: rig.directory)
        _ = refresh()
    }

    public var state: MeaningModelState { lock.withLock { _state } }
    private func set(_ s: MeaningModelState) {
        let changed = lock.withLock { () -> Bool in let c = _state != s; _state = s; return c }
        guard changed else { return }
        DispatchQueue.main.async { NotificationCenter.default.post(name: Self.didChange, object: self) }
    }
    private func begin() -> Bool { lock.withLock { if _running { return false }; _running = true; return true } }
    private func end() { lock.withLock { _running = false } }

    /// Reads the disk: ready only when the installer's manifest lists every file at its size. While a download runs, the progress.
    @discardableResult
    public func refresh() -> MeaningModelState {
        if lock.withLock({ _running }) {
            // A download is going (started by any surface): never "not downloaded". Between its start and its first state, the bytes on disk.
            let now = state
            if case .notDownloaded = now { let s = MeaningModelState.downloading(received: ModelDisk.bytesOnDisk(spec, in: directory), total: spec.totalBytes); set(s); return s }
            return now
        }
        if ModelDisk.isReady(spec, in: directory) { let s = MeaningModelState.ready(bytes: spec.totalBytes); set(s); return s }
        if case .failed = state { return state }   // the sentence that says why stays until the next try
        set(.notDownloaded); return .notDownloaded
    }

    /// Downloads what is missing. Returns .ready or .failed; call again to retry. A second call during a download starts nothing.
    @discardableResult
    public func download() async -> MeaningModelState {
        guard begin() else { return state }
        defer { end() }
        return await run()
    }

    /// The body of a download, with the one-at-a-time flag already taken (`download()` or `resumeIfInterrupted()`).
    private func run() async -> MeaningModelState {
        if let why = blocked { let s = MeaningModelState.failed(reason: why); set(s); return s }
        if ModelDisk.isReady(spec, in: directory) { markWanted(false); let s = MeaningModelState.ready(bytes: spec.totalBytes); set(s); return s }
        set(.downloading(received: ModelDisk.bytesOnDisk(spec, in: directory), total: spec.totalBytes))   // the click shows at once
        markWanted(true)                  // a quit from here on leaves the marker: the next launch continues (resumeIfInterrupted)
        let watch = Task { [weak self] in
            var last: Int64 = -1
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: self?.tick ?? MeaningModelStore.tickNs)
                guard let self, !Task.isCancelled else { return }
                let got = ModelDisk.bytesOnDisk(self.spec, in: self.directory)
                if got != last { last = got; self.set(.downloading(received: got, total: self.spec.totalBytes)) }
            }
        }
        let result = await installer.start()
        watch.cancel()
        markWanted(false)                 // ended (ready, refused or stopped): only a quit in the middle leaves the marker
        switch result {
        case .ready:
            let s = MeaningModelState.ready(bytes: spec.totalBytes); set(s); onReady(); return s
        case .failed(let why), .corrupt(let why):
            let s = MeaningModelState.failed(reason: Words.text("Could not download the search model. %@ The file was %@.", why, unfinishedFile()))
            set(s); return s
        default:   // paused or cancelled: nothing in this store asks for either
            let s = MeaningModelState.failed(reason: Words.text("The search model download stopped. Tap Retry."))
            set(s); return s
        }
    }

    /// The Download or Retry button: starts the download off the main thread and returns at once. False when nothing can start (ready, or one is running).
    @discardableResult
    public func startInBackground() -> Bool {
        let s = refresh()
        guard s.button != nil else { return false }
        Task.detached(priority: .utility) { [self] in _ = await download() }
        return true
    }

    /// The marker of a download in flight, beside the model's files (a name the installer's manifest never lists). Written when a download starts,
    /// removed when it ends in any way; still there at launch means the app quit while it ran. A download that ended (ready, a refusal, or a stop
    /// the owner asked for) leaves no marker, so it is never restarted by itself: the owner's Retry starts it again.
    private var marker: URL { directory.appendingPathComponent(".download-in-flight") }
    private func markWanted(_ on: Bool) {
        if on {
            try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            FileManager.default.createFile(atPath: marker.path, contents: Data())
        } else {
            try? FileManager.default.removeItem(at: marker)
        }
    }

    /// At launch: a download the app quit in the middle of (the marker is there, the model is not ready) continues by itself, under the same rules as
    /// a click (a test copy's loopback-only refusal, the installer's free space and checksum checks). The line reads "downloading" at once, with the
    /// bytes already on disk, before the first frame. False when nothing was interrupted, the model is ready, or a download already runs.
    @discardableResult
    public func resumeIfInterrupted() -> Bool {
        guard FileManager.default.fileExists(atPath: marker.path), !ModelDisk.isReady(spec, in: directory), begin() else { return false }
        set(.downloading(received: ModelDisk.bytesOnDisk(spec, in: directory), total: spec.totalBytes))
        Task.detached(priority: .utility) { [self] in _ = await run(); end() }
        return true
    }

    /// The first file not on disk at its size: the one a failure was about.
    private func unfinishedFile() -> String {
        spec.files.first { ModelDisk.size(of: directory.appendingPathComponent($0.path)) != $0.bytes }?.path ?? spec.files.last?.path ?? ""
    }
}
