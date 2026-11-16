import Foundation

/// What the Settings row shows about the downloaded decision model.
public enum LayaModelState: Equatable, Sendable {
    case notDownloaded
    case downloading(received: Int64, total: Int64)   // total is -1 while the size is not yet known
    case ready(bytes: Int64)
    case failed(reason: String)                         // the row offers Retry next to this sentence

    public var words: String {
        switch self {
        case .notDownloaded: return Words.text("Not downloaded. The decision model is about 840 MB and downloads once to this Mac.")
        case .downloading(let r, let t):
            return t > 0 ? Words.text("Downloading %@ of %@.", AttachmentSize.words(r), AttachmentSize.words(t)) : Words.text("Downloading %@.", AttachmentSize.words(r))
        case .ready(let b): return Words.text("Ready on this Mac (%@). Nothing leaves it.", AttachmentSize.words(b))
        case .failed(let reason): return reason
        }
    }
}

/// Downloads Laya (Apache-2.0 weights, convaiinnovations/laya) once, into the app's support folder. Never committed.
public final class LayaModelStore: @unchecked Sendable {
    // the files, the address and the folders are the catalogue's line for Laya (DecisionModelCatalogue.swift): written once
    public static let checkpoint = DecisionModelCatalogue.laya.checkpoint
    public static let remoteFiles = DecisionModelCatalogue.laya.files
    public static let defaultBase = DecisionModelCatalogue.laya.base

    public let directory: URL
    public let baseURL: URL
    public let files: [String]
    private let session: URLSession
    private let lock = NSLock()
    private var inFlight = false
    private var cancelRequested = false
    private var running: ProgressDelegate?
    /// Posted (any thread) each time the state moves: every surface that shows the download re-reads `LayaOnDevice.shared.state`.
    public static let didChange = Notification.Name("whoosh.laya.download")
    private var _state: LayaModelState

    public var state: LayaModelState { lock.lock(); defer { lock.unlock() }; return _state }
    public var modelFolder: URL { directory.appendingPathComponent(Self.checkpoint) }

    public static func defaultDirectory() -> URL {
        ModelsFolder.root().appendingPathComponent(DecisionModelCatalogue.laya.folder, isDirectory: true)
    }

    public init(directory: URL = LayaModelStore.defaultDirectory(), baseURL: URL = ModelsFolder.rigLayaBase() ?? LayaModelStore.defaultBase,
                files: [String] = LayaModelStore.remoteFiles, session: URLSession = .shared) {
        self.directory = directory; self.baseURL = ModelsFolder.rigSafe(baseURL); self.files = files; self.session = session   // a rig copy never asks the real host
        _state = .notDownloaded
        _ = refresh()
    }

    private var manifestURL: URL { directory.appendingPathComponent("ready.json") }
    /// How often the bytes received are read while a file arrives (4 times a second: a progress line, not an animation).
    static let progressTickNs: UInt64 = 250_000_000

    /// Reads the disk: ready only when the manifest exists and every file is present at its recorded size.
    @discardableResult
    public func refresh() -> LayaModelState {
        lock.lock(); let busy = inFlight, now = _state; lock.unlock()
        if busy { return now }   // a download is running: the disk says "not downloaded" until it ends, and must not wipe the progress
        let new: LayaModelState
        if let data = try? Data(contentsOf: manifestURL), let sizes = try? JSONDecoder().decode([String: Int64].self, from: data),
           files.allSatisfy({ f in sizes[f] != nil && Self.size(of: directory.appendingPathComponent(f)) == sizes[f] }) {
            new = .ready(bytes: sizes.values.reduce(0, +))
        } else {
            if case .failed = now { return now }   // the sentence that says why, with Retry, stays until the next try
            new = .notDownloaded
        }
        set(new); return new
    }

    /// nil when this call starts the download; the running state when one is already going.
    private func beginDownload() -> LayaModelState? {
        lock.lock(); defer { lock.unlock() }
        if inFlight { return _state }
        inFlight = true; cancelRequested = false; return nil
    }
    private func endDownload() { lock.lock(); inFlight = false; running = nil; lock.unlock() }
    private func isCancelled() -> Bool { lock.lock(); defer { lock.unlock() }; return cancelRequested }
    private func track(_ d: ProgressDelegate) { lock.lock(); running = d; lock.unlock() }

    /// The Cancel button: stops the running download at once (the file in flight is dropped by the session, the files already finished stay,
    /// nothing half-written is left in the folder) and the state goes back to "not downloaded". Does nothing when no download runs.
    public func cancel() {
        lock.lock(); let was = inFlight, d = inFlight ? running : nil; if inFlight { cancelRequested = true }; lock.unlock()
        d?.cancel()
        // hid-ai: the row says "not downloaded" the moment Cancel is pressed (settingsaimac-check laya-cancel read "downloading" right after
        // the click: the task ended a tick later). The task, when it ends, says the same; the progress watch stops at the request.
        if was { set(.notDownloaded) }
    }

    /// The launch state again (`fixture reset`, the same reading as a relaunch): a failed download's sentence goes and the disk decides.
    /// Nothing changes while a download runs.
    public func forgetFailure() {
        lock.lock(); let busy = inFlight, failed: Bool = { if case .failed = _state { return true } else { return false } }(); lock.unlock()
        guard !busy, failed else { return }
        set(.notDownloaded)
        refresh()
    }

    /// The change is announced on the NEXT turn of the main queue, never inside this call: a listener reads `LayaOnDevice.shared.state`, and when
    /// this store is still being made (its first read of the disk runs inside `LayaOnDevice.modelStore`'s lock) a listener called from here waited
    /// on that same lock for ever (main72 and main73: every hidden copy on the fixture froze at the first label opened).
    /// Only a CHANGE is announced: every read of the state goes through `refresh()`, which ends here, and a listener answers the
    /// announcement by reading the state again. Announcing an unchanged state made the two call each other for ever
    /// (main74: 100 % of a processor core with Settings open, `idle-cpu-with-the-window-open: expected <= 1.0, got 100.07`).
    private func set(_ s: LayaModelState) {
        lock.lock(); let changed = _state != s; _state = s; lock.unlock()
        guard changed else { return }
        DispatchQueue.main.async { NotificationCenter.default.post(name: Self.didChange, object: nil) }
    }
    private static func size(of url: URL) -> Int64? {
        (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? NSNumber)?.int64Value
    }

    /// Downloads what is missing, reporting bytes as they arrive. Returns .ready or .failed(reason); call again to retry.
    @discardableResult
    public func download(progress: @escaping @Sendable (LayaModelState) -> Void = { _ in }) async -> LayaModelState {
        if Task.isCancelled { return refresh() }   // round 6: Cancel came before the download began: nothing is asked
        if case .ready = refresh() { return state }
        if let running = beginDownload() { return running }   // a second click (any surface) joins the one running download
        defer { endDownload() }
        do { let s = LayaModelState.downloading(received: 0, total: -1); set(s); progress(s) }   // the click shows at once; the sizes come next
        let fm = FileManager.default
        do {
            var sizes: [String: Int64] = [:]
            var totals: [String: Int64] = [:]
            for f in files { if isCancelled() { throw CancellationError() }; totals[f] = try await remoteSize(f) }
            let known = totals.values.allSatisfy { $0 > 0 }
            let grand: Int64 = known ? totals.values.reduce(0, +) : -1
            var done: Int64 = 0
            for f in files {
                if isCancelled() { throw CancellationError() }
                let dest = directory.appendingPathComponent(f)
                try fm.createDirectory(at: dest.deletingLastPathComponent(), withIntermediateDirectories: true)
                if let have = Self.size(of: dest), have == totals[f], have > 0 { sizes[f] = have; done += have; continue }
                let base = done
                let delegate = ProgressDelegate()
                track(delegate)
                do { let s = LayaModelState.downloading(received: base, total: grand); set(s); progress(s) }
                let watch = Task { [weak self] in
                    var last: Int64 = -1
                    while !Task.isCancelled, self?.isCancelled() == false {
                        let received = delegate.received
                        if received != last, received > 0 {
                            last = received
                            let s = LayaModelState.downloading(received: base + received, total: grand)
                            self?.set(s); progress(s)
                        }
                        try? await Task.sleep(nanoseconds: Self.progressTickNs)
                    }
                }
                defer { watch.cancel() }
                let (tmp, response) = try await session.download(for: URLRequest(url: baseURL.appendingPathComponent(f)), delegate: delegate)
                watch.cancel()
                if let http = response as? HTTPURLResponse, !(200...299).contains(http.statusCode) {
                    try? fm.removeItem(at: tmp)
                    throw LocalModelError.failed(Words.text("The download server answered %@ for %@.", http.statusCode, f))
                }
                try? fm.removeItem(at: dest)
                try fm.moveItem(at: tmp, to: dest)
                let got = Self.size(of: dest) ?? 0
                if let want = totals[f], want > 0, got != want {
                    try? fm.removeItem(at: dest)
                    throw LocalModelError.failed(Words.text("%@ arrived with %@ bytes instead of %@.", f, got, want))
                }
                sizes[f] = got; done += got
            }
            try JSONEncoder().encode(sizes).write(to: manifestURL, options: .atomic)
            let s = LayaModelState.ready(bytes: sizes.values.reduce(0, +))
            set(s); progress(s); return s
        } catch {
            // the owner pressed Cancel: not a failure, nothing half-written stays, the row offers Download again. Round 7: also when the Cancel reached
            // the task (AISettingsStore.cancelLaya cancels it) before the store marked the download as running: the request then ends `cancelled`
            if isCancelled() || Task.isCancelled || error is CancellationError || (error as? URLError)?.code == .cancelled {
                try? fm.removeItem(at: manifestURL)
                for f in files { let d = directory.appendingPathComponent(f); if let n = Self.size(of: d), n == 0 { try? fm.removeItem(at: d) } }
                let s = LayaModelState.notDownloaded
                set(s); progress(s); return s
            }
            let reason: String
            if let e = error as? LocalModelError { reason = Words.text("Could not download the decision model. %@ Tap Retry.", e.description) }
            else if (error as? URLError)?.code == .notConnectedToInternet { reason = Words.text("Could not download the decision model: this Mac is offline. Connect, then tap Retry.") }
            else { reason = Words.text("Could not download the decision model: %@ Tap Retry.", error.localizedDescription) }
            let s = LayaModelState.failed(reason: reason)
            set(s); progress(s); return s
        }
    }

    /// Size in bytes the server reports, or -1 when it does not say.
    private func remoteSize(_ file: String) async throws -> Int64 {
        var req = URLRequest(url: baseURL.appendingPathComponent(file))
        req.httpMethod = "HEAD"
        let (_, response) = try await session.data(for: req)
        if let http = response as? HTTPURLResponse {
            if !(200...299).contains(http.statusCode) { throw LocalModelError.failed(Words.text("The download server answered %@ for %@.", http.statusCode, file)) }
            let n = http.expectedContentLength
            return n > 0 ? n : -1
        }
        return -1
    }
}

/// Holds the download's task so its byte counter can be read. `didCreateTask` is a task-delegate call, so a per-task delegate
/// of the async `download(for:delegate:)` receives it; `didWriteData` is a download-delegate call and is never delivered there.
private final class ProgressDelegate: NSObject, URLSessionTaskDelegate, @unchecked Sendable {
    private let lock = NSLock()
    private var task: URLSessionTask?
    func urlSession(_ session: URLSession, didCreateTask task: URLSessionTask) { lock.lock(); self.task = task; lock.unlock() }
    func cancel() { lock.lock(); let t = task; lock.unlock(); t?.cancel() }
    var received: Int64 { lock.lock(); defer { lock.unlock() }; return task?.countOfBytesReceived ?? 0 }
}
