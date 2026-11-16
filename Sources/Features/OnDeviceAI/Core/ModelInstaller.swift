import Foundation

/// The install step for the on-device language model: resumable, checksummed, pausable, cancellable.
/// Files land in `directory` (Application Support, never the repo). Only the pinned spec's URLs are ever asked.
public final class ModelInstaller: @unchecked Sendable {
    private enum Stop { case none, pause, cancel }
    public let spec: MlxModelSpec
    public let directory: URL
    public var onChange: (@Sendable (ModelInstallState) -> Void)?
    private let lock = NSLock()
    private var _state: ModelInstallState = .notInstalled
    private var stop = Stop.none, running = false, current: FileFetcher?, lastPercent = -1

    public init(spec: MlxModelSpec = .qwen25, directory: URL = ModelInstaller.defaultDirectory()) {
        self.spec = spec; self.directory = directory
        refresh()
    }

    public static func defaultDirectory(for spec: MlxModelSpec = .qwen25) -> URL {
        ModelsFolder.root().appendingPathComponent(spec.folderName, isDirectory: true)
    }

    public var state: ModelInstallState { lock.lock(); defer { lock.unlock() }; return _state }
    private func set(_ s: ModelInstallState) { lock.lock(); _state = s; lock.unlock(); onChange?(s) }

    /// Reads the disk. Ready, or paused with the bytes already there, or not installed.
    @discardableResult
    public func refresh() -> ModelInstallState {
        lock.lock(); let busy = running; lock.unlock()
        if busy { return state }
        let s: ModelInstallState
        if ModelDisk.isReady(spec, in: directory) { s = .ready(bytes: spec.totalBytes) }
        else {
            let have = ModelDisk.bytesOnDisk(spec, in: directory)
            s = have > 0 ? .paused(received: have, total: spec.totalBytes) : .notInstalled
        }
        set(s); return s
    }

    public func pause() { request(.pause) }
    /// Cancel throws away the partial files; the next start begins at zero.
    public func cancel() { request(.cancel) }
    private func request(_ s: Stop) {
        lock.lock(); stop = s; let f = current; let idle = !running; lock.unlock()
        f?.stop()
        if idle && s == .cancel { discardPartials(); refresh() }
    }

    /// Marks an installed model as unusable (a load found it damaged). The next start downloads it again.
    public func markCorrupt(_ reason: String) {
        try? FileManager.default.removeItem(at: ModelDisk.manifestURL(directory))
        set(.corrupt(reason: reason))
    }

    private func discardPartials() {
        for f in spec.files { try? FileManager.default.removeItem(at: directory.appendingPathComponent(f.path + ".part")) }
    }

    /// Downloads what is missing. Returns when ready, paused, cancelled or failed; call again to resume or retry.
    @discardableResult
    public func start() async -> ModelInstallState {
        guard begin() else { return state }
        defer { end() }
        if case .corrupt = state { removeInstalledFiles() }
        if ModelDisk.isReady(spec, in: directory) { let s = ModelInstallState.ready(bytes: spec.totalBytes); set(s); return s }
        let fm = FileManager.default
        do {
            try fm.createDirectory(at: directory, withIntermediateDirectories: true)
            let need = spec.totalBytes - ModelDisk.bytesOnDisk(spec, in: directory)
            if let free = ModelDisk.freeBytes(near: directory), free < need {
                return fail(Words.text("Not enough space: the model needs %@ more and this Mac has %@ free.", ModelInstallState.size(need), ModelInstallState.size(free)))
            }
            var done: Int64 = 0
            resetPercent()
            for file in spec.files {
                let dest = directory.appendingPathComponent(file.path)
                if ModelDisk.size(of: dest) == file.bytes { done += file.bytes; continue }
                try? fm.removeItem(at: dest)
                let part = directory.appendingPathComponent(file.path + ".part")
                // round 12: a file in a sub-folder of the model ("typed-decisions/rl_agent_config.json") needs that folder before its .part can be
                // made: FileFetcher's createFile failed in silence and the read of the .part threw "doesn't exist" before any request (main93 to
                // main103 settingsaimac-check J5.9 meaning cases: the stub was asked nothing)
                try fm.createDirectory(at: part.deletingLastPathComponent(), withIntermediateDirectories: true)
                let base = done, total = spec.totalBytes
                let fetcher = FileFetcher(url: spec.url(for: file), part: part) { [self] got in
                    let pct = Int(Double(base + got) / Double(max(1, total)) * 100)
                    if percentChanged(pct) { set(.downloading(received: base + got, total: total)) }
                }
                if arm(fetcher) { throw CancellationError() }
                set(.downloading(received: base, total: spec.totalBytes))
                let got = try await fetcher.run()
                set(.verifying)
                // a file with no SHA-256 (a small file of a model added through "Other model": the hub gives a checksum for its large files only)
                // is checked by its size alone (model-door, SPEC A133)
                guard got.bytes == file.bytes, file.sha256.isEmpty || got.sha256 == file.sha256 else {
                    try? fm.removeItem(at: part)
                    return fail(Words.text("The checksum of %@ does not match (expected %@, got %@). The file was deleted. Tap Try again.", file.path, file.sha256.prefix(12), got.sha256.prefix(12)))
                }
                try fm.moveItem(at: part, to: dest)
                done += file.bytes
            }
            try ModelDisk.writeManifest(spec, in: directory)
            let s = ModelInstallState.ready(bytes: spec.totalBytes); set(s); return s
        } catch {
            let why = stopRequested
            if why == .cancel { discardPartials(); set(.notInstalled); return .notInstalled }
            if why == .pause || error is CancellationError {
                let s = ModelInstallState.paused(received: ModelDisk.bytesOnDisk(spec, in: directory), total: spec.totalBytes); set(s); return s
            }
            return fail(Self.words(for: error))
        }
    }

    // Synchronous lock helpers: NSLock may not be held across an await, so the async code calls these.
    private func begin() -> Bool { lock.withLock { if running { return false }; running = true; stop = .none; return true } }
    private func end() { lock.withLock { running = false; current = nil } }
    private func resetPercent() { lock.withLock { lastPercent = -1 } }
    private func percentChanged(_ p: Int) -> Bool { lock.withLock { defer { lastPercent = p }; return p != lastPercent } }
    private func arm(_ f: FileFetcher) -> Bool { lock.withLock { current = f; return stop != .none } }
    private var stopRequested: Stop { lock.withLock { stop } }

    private func removeInstalledFiles() { for f in spec.files { try? FileManager.default.removeItem(at: directory.appendingPathComponent(f.path)) } }
    private func fail(_ reason: String) -> ModelInstallState { let s = ModelInstallState.failed(reason: reason); set(s); return s }
}
