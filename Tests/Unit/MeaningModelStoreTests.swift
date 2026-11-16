import Foundation
import Testing
@testable import WhooshUI

// The search model's store (lane bg-hooks round 3) against a slow LOOPBACK server: no real network, no model, nothing in the owner's folders.

private struct Fixture {
    let server: SlowFileServer
    let spec: MlxModelSpec
    let dir: URL
    let big: Data
    /// `fast`: the same 48,000 bytes at a quicker pace (a test that does not look at the timer). `altered`: the server holds a file of the right length whose bytes differ from the spec's.
    static func make(missing: Bool = false, fast: Bool = false, altered: Bool = false) async throws -> Fixture {
        let small = Data("{\"model\":\"fixture\"}".utf8)
        let big = Data((0..<48_000).map { UInt8(truncatingIfNeeded: $0 &* 31) })
        var served = big
        if altered { served[0] ^= 0xFF }
        let held = missing ? ["config.json": small] : ["config.json": small, "model.safetensors": served]
        let server = try fast ? SlowFileServer(files: held, pause: 0.05, firstByteDelay: 0.05) : SlowFileServer(files: held)
        await server.start()
        let spec = MlxModelSpec(name: "fixture", licence: "Apache-2.0", baseURL: server.base, files: [
            ModelFile(path: "config.json", bytes: Int64(small.count), sha256: SlowFileServer.hex(small)),
            ModelFile(path: "model.safetensors", bytes: Int64(big.count), sha256: SlowFileServer.hex(big)),
        ], folderName: "fixture")
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("meaning-store-\(UUID().uuidString)", isDirectory: true)
        return Fixture(server: server, spec: spec, dir: dir, big: big)
    }
    func store(onReady: @escaping @Sendable () -> Void = {}, tickNs: UInt64 = 50_000_000) -> MeaningModelStore {
        MeaningModelStore(rig: MeaningModelRig(spec: spec, directory: dir, blocked: nil), tickNs: tickNs, onReady: onReady)
    }
}

private final class Samples: @unchecked Sendable {
    private let lock = NSLock(); private var all: [MeaningModelState] = []
    func add(_ s: MeaningModelState) { lock.withLock { if all.last != s { all.append(s) } } }
    var list: [MeaningModelState] { lock.withLock { all } }
}
private final class Counter: @unchecked Sendable { private let l = NSLock(); private var n = 0; func bump() { l.withLock { n += 1 }; }; var value: Int { l.withLock { n } } }

/// Reads the store every 10 ms until the task ends.
private func sample(_ store: MeaningModelStore, while run: Task<MeaningModelState, Never>) async -> [MeaningModelState] {
    let seen = Samples()
    seen.add(store.state)
    let poll = Task { while !Task.isCancelled { seen.add(store.refresh()); try? await Task.sleep(nanoseconds: 10_000_000) } }
    _ = await run.value
    poll.cancel(); seen.add(store.state)
    return seen.list
}

@Test func theStateIsDownloadingTheMomentTheDownloadStartsBeforeTheFirstByteAndThreeByteCountsFollow() async throws {
    let f = try await Fixture.make(); defer { f.server.stop(); try? FileManager.default.removeItem(at: f.dir) }
    let ready = Counter()
    let store = f.store(onReady: { ready.bump() })
    #expect(store.state == .notDownloaded)
    let run = Task { await store.download() }
    try await Task.sleep(nanoseconds: 20_000_000)   // before the first timer tick (50 ms) and before the first byte (the server holds it 500 ms)
    #expect(store.state == .downloading(received: 0, total: f.spec.totalBytes), "the click shows at once: \(store.state)")
    #expect(store.refresh() == store.state, "a read during a download answers with the progress, not notDownloaded")
    let seen = await sample(store, while: run)
    let counts = seen.compactMap { s -> Int64? in if case .downloading(let r, _) = s { return r } else { return nil } }
    let distinct = Set(counts.filter { $0 > 0 && $0 < f.spec.totalBytes })
    #expect(distinct.count >= 3, "three distinct byte counts during one file: \(counts)")
    #expect(counts == counts.sorted(), "the count only grows: \(counts)")
    #expect(store.state == .ready(bytes: f.spec.totalBytes))
    #expect(ready.value == 1, "onReady ran once, so the engine is handed the model with no relaunch")
    #expect(store.refresh() == .ready(bytes: f.spec.totalBytes))
}

@Test func aSecondClickDuringTheDownloadStartsNothing() async throws {
    let f = try await Fixture.make(); defer { f.server.stop(); try? FileManager.default.removeItem(at: f.dir) }
    let store = f.store()
    let run = Task { await store.download() }
    try await Task.sleep(nanoseconds: 150_000_000)
    let second = await store.download()
    #expect(second.isDownloading, "the second call reports the progress: \(second)")
    _ = await run.value
    #expect(f.server.requests.filter { $0.hasSuffix("model.safetensors") && $0.hasPrefix("GET") }.count == 1)
}

@Test func aMissingFileEndsFailedWithTheFileNamedAndRetryWorks() async throws {
    let f = try await Fixture.make(missing: true); defer { f.server.stop(); try? FileManager.default.removeItem(at: f.dir) }
    let store = f.store()
    let end = await store.download()
    guard case .failed(let why) = end else { Issue.record("expected failed, got \(end)"); return }
    #expect(why.contains("model.safetensors"), "the sentence names the file: \(why)")
    #expect(why.contains("404"), "and the server's answer: \(why)")
    #expect(store.refresh() == end, "the sentence stays until the next try")
    f.server.put("model.safetensors", f.big)
    let again = await store.download()
    #expect(again == .ready(bytes: f.spec.totalBytes), "retry after the file exists: \(again)")
}

@Test func aRigCopyReachesNothingWithoutALoopbackAddressAndWritesOnlyInItsRunFolder() async {
    let run = FileManager.default.temporaryDirectory.appendingPathComponent("rig-run-\(UUID().uuidString)").path
    let none = MeaningModelRig.current(environment: ["WHOOSH_SOCKET_DIR": run])
    #expect(none.blocked != nil && none.directory.path.hasPrefix(run))
    let remote = MeaningModelRig.current(environment: ["WHOOSH_SOCKET_DIR": run, "WHOOSH_RIG_EMBED_BASE": "https://huggingface.co/x/"])
    #expect(remote.blocked != nil, "a non-loopback address is refused")
    let local = MeaningModelRig.current(environment: ["WHOOSH_SOCKET_DIR": run, "WHOOSH_RIG_EMBED_BASE": "http://127.0.0.1:9/"])
    #expect(local.blocked == nil && local.spec.baseURL.host == "127.0.0.1" && local.directory.path.hasPrefix(run))
    let store = MeaningModelStore(rig: none)
    let end = await store.download()
    #expect(end.key == "failed", "no address, no request: \(end)")
    let owner = MeaningModelRig.current(environment: [:])
    #expect(owner.blocked == nil && owner.spec.baseURL.host == "huggingface.co" && owner.directory.path.contains("Whoosh/Models/embeddinggemma-2"))
    #expect(GemmaWeights.candidates(environment: ["WHOOSH_SOCKET_DIR": run]).first?.directory.path.hasPrefix(run) == true, "a rig copy looks in its run folder first")
}

@Test func theEngineIsHandedTheModelAfterTheDownloadWithoutARelaunch() async throws {
    // the store's onReady is BackgroundWork.recheckModel: the hand-over looks again and hands the embedder over
    final class Engine: BackgroundEngine, @unchecked Sendable {
        let l = NSLock(); var embedders: [Bool] = []
        func setConditions(batteryPercent: Int, charging: Bool, thermalSerious: Bool) {}
        func ownerActed() {}
        func setEmbedder(_ e: (any Embedder)?) { l.withLock { embedders.append(e != nil) } }
    }
    let engine = Engine(), state = BackgroundState()
    let present = Counter()
    let hand = EmbedderHandOver(engine: engine, state: state, find: { present.value > 0 ? FixtureEmbedder() : nil })
    hand.look(); try await Task.sleep(nanoseconds: 200_000_000)
    #expect(engine.l.withLock { engine.embedders } == [false] && state.snapshot().modelMissing, "no model: the engine is told so")
    let f = try await Fixture.make(); defer { f.server.stop(); try? FileManager.default.removeItem(at: f.dir) }
    let store = f.store(onReady: { present.bump(); hand.look() })
    _ = await store.download()
    try await Task.sleep(nanoseconds: 300_000_000)
    #expect(engine.l.withLock { engine.embedders } == [false, true] && state.snapshot().embedderHanded && !state.snapshot().modelMissing)
}

// A test copy or a process on the test engine never looks for the weights in the owner's folders (the installed model, the cache).
@Test func aTestProcessLooksForTheWeightsOnlyInItsOwnFolders() {
    let home = URL(fileURLWithPath: "/nonexistent-home-for-this-test", isDirectory: true)
    let engine = GemmaWeights.candidates(environment: ["WHOOSH_FIXTURE_CORE": "1"], home: home)
    #expect(engine.isEmpty)                                    // fails on the old code: the installer's folder was listed
    let given = GemmaWeights.candidates(environment: ["WHOOSH_FIXTURE_CORE": "1", "WHOOSH_EMBED_WEIGHTS": "/tmp/given-weights"], home: home)
    #expect(given.map(\.directory.path) == ["/tmp/given-weights"])
    let rig = GemmaWeights.candidates(environment: ["WHOOSH_SOCKET_DIR": "/tmp/run-x"], home: home)
    #expect(rig.count == 1)
    #expect(rig.first?.directory.path.hasPrefix("/tmp/run-x") == true)
}


// The claims of item 5c6a43eb: the weights are checksummed and resumable like the Qwen model's, and read back from the install location.
// The same installer and the same loopback server as above; the spec is the fixture's (a 48,000-byte file), never the real 1.5 GB one.

@Test func aDownloadCutHalfwayResumesFromItsPartFileAndAsksOnlyTheRest() async throws {
    let f = try await Fixture.make(fast: true); defer { f.server.stop(); try? FileManager.default.removeItem(at: f.dir) }
    let part = f.dir.appendingPathComponent("model.safetensors.part")
    let first = ModelInstaller(spec: f.spec, directory: f.dir)
    let run = Task { await first.start() }
    let deadline = Date().addingTimeInterval(10)
    while (ModelDisk.size(of: part) ?? 0) < 8_000, Date() < deadline { try await Task.sleep(nanoseconds: 10_000_000) }
    first.pause()                                       // the cut: some of the file is in, not all
    let cut = await run.value
    guard case .paused = cut else { Issue.record("expected paused after the cut, got \(cut)"); return }
    let kept = Int(ModelDisk.size(of: part) ?? -1)
    #expect(kept >= 8_000 && kept < f.big.count, "a part of the file stays on disk: \(kept) of \(f.big.count)")
    let ready = Counter()
    let store = f.store(onReady: { ready.bump() })      // a new store over the same folder: the next try
    let end = await store.download()
    #expect(end == .ready(bytes: f.spec.totalBytes))
    let weights = f.server.gets.filter { $0.path == "model.safetensors" }
    #expect(weights == [SlowFileServer.Get(path: "model.safetensors", from: 0), SlowFileServer.Get(path: "model.safetensors", from: kept)], "the second request carries Range: bytes=\(kept)-: \(weights)")
    #expect(f.server.gets.filter { $0.path == "config.json" }.count == 1, "the finished file is not asked again")
    let whole = try Data(contentsOf: f.dir.appendingPathComponent("model.safetensors"))
    #expect(whole == f.big && SlowFileServer.hex(whole) == f.spec.files[1].sha256, "the file is whole and its SHA-256 is the spec's")
    #expect(!FileManager.default.fileExists(atPath: part.path), "no part file is left")
    #expect(ready.value == 1)
}

@Test func aFileWhoseSha256DoesNotMatchTheSpecIsRefusedAndNotKept() async throws {
    let f = try await Fixture.make(fast: true, altered: true); defer { f.server.stop(); try? FileManager.default.removeItem(at: f.dir) }
    let ready = Counter()
    let store = f.store(onReady: { ready.bump() })
    let end = await store.download()
    guard case .failed(let why) = end else { Issue.record("expected failed, got \(end)"); return }
    #expect(why.contains("model.safetensors"), "the sentence names the file: \(why)")
    #expect(why.contains(String(f.spec.files[1].sha256.prefix(12))), "and the checksum it expected: \(why)")
    #expect(!FileManager.default.fileExists(atPath: f.dir.appendingPathComponent("model.safetensors").path), "nothing is kept under the final name")
    #expect(!FileManager.default.fileExists(atPath: f.dir.appendingPathComponent("model.safetensors.part").path), "and no part file")
    #expect(!ModelDisk.isReady(f.spec, in: f.dir) && store.refresh() == end, "the model is not ready and the sentence stays")
    #expect(ready.value == 0, "the engine is not handed a model that failed its checksum")
}

@Test func theWeightsReadBackFromTheInstallFolderAtTheirPublishedSizes() async throws {
    let f = try await Fixture.make(fast: true); defer { f.server.stop(); try? FileManager.default.removeItem(at: f.dir) }
    #expect(GemmaWeights.files == MlxModelSpec.embeddingGemma2.files.map(\.path), "the runtime reads the files the installer downloads")
    #expect(GemmaWeights(directory: f.dir).isIntact(spec: f.spec) == false, "nothing installed yet")
    let installed = await f.store().download()
    #expect(installed == .ready(bytes: f.spec.totalBytes))
    let weights = GemmaWeights(directory: f.dir)
    let back = weights.readBack(spec: f.spec)
    #expect(back.map(\.path) == ["config.json", "model.safetensors"] && back.allSatisfy(\.ok), "every file read back at its published size: \(back)")
    #expect(back.map(\.foundBytes) == f.spec.files.map { Optional($0.bytes) })
    #expect(weights.isIntact(spec: f.spec))
    let weightsFile = f.dir.appendingPathComponent("model.safetensors")
    let handle = try FileHandle(forWritingTo: weightsFile)
    try handle.truncate(atOffset: UInt64(f.big.count - 1)); try handle.close()     // one byte short
    #expect(!weights.isIntact(spec: f.spec), "a short file makes the install not intact")
    #expect(weights.readBack(spec: f.spec).first { $0.path == "model.safetensors" }?.foundBytes == Int64(f.big.count - 1))
    try FileManager.default.removeItem(at: weightsFile)
    #expect(weights.readBack(spec: f.spec).first { $0.path == "model.safetensors" }?.foundBytes == nil && !weights.isIntact(spec: f.spec), "a missing file reads back as nothing")
}
