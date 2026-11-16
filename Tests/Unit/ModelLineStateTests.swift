import Foundation
import Testing
@testable import WhooshUI

/// The model lines of Settings (the decision model "Laya", the search model): sizes in the app's chosen language, and a screen that always shows
/// the store's true state. No network: a folder written by the test, never the owner's folders.
@Suite("ModelLineState") struct ModelLineStateTests {
    /// Old code: `ByteCountFormatter()` answered in the system's language, so an English choice on a French Mac read "Mo" and this test failed there.
    @Test func aSizeOnTheModelLinesIsWrittenInTheChosenLanguageNotTheSystemsOne() {
        LanguagePin.with("en") {
            #expect(LayaModelState.downloading(received: 0, total: 843_000_000).words.contains("MB"))
            #expect(!LayaModelState.downloading(received: 0, total: 843_000_000).words.contains("Mo"))
            #expect(ModelInstallState.size(843_000_000).contains("MB"))
            #expect(MeaningModelState.ready(bytes: 1_500_000_000).words.contains("GB"))
        }
        LanguagePin.with("fr") {
            #expect(LayaModelState.downloading(received: 0, total: 843_000_000).words.contains("Mo"))
            #expect(ModelInstallState.size(843_000_000).contains("Mo"))
            #expect(MeaningModelState.ready(bytes: 1_500_000_000).words.contains("Go"))
        }
    }

    // MARK: the line follows the store

    /// A model folder written by the test: the files and the store's own record (`ready.json`) with each file's size. The address is never asked.
    private static func completeFolder(_ files: [String]) throws -> (dir: URL, total: Int64) {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("model-line-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        var sizes: [String: Int64] = [:]
        for (i, f) in files.enumerated() {
            let data = Data(repeating: 0x41, count: 100 + i)
            try data.write(to: dir.appendingPathComponent(f)); sizes[f] = Int64(data.count)
        }
        try JSONEncoder().encode(sizes).write(to: dir.appendingPathComponent("ready.json"))
        return (dir, sizes.values.reduce(0, +))
    }
    private static let names = ["a.json", "b.bin"]
    private static let nowhere = URL(string: "http://127.0.0.1:9/")!   // nothing listens: a click that asked it would fail, not hang

    /// Old code: the click set "downloading" first and the call that followed returned at once with nothing announced, so the line stayed on
    /// "downloading" for ever (the owner's "Downloading Zero ko of 843 Mo"). This test reads the state right after the click, so it fails there.
    @MainActor @Test func aClickOnDownloadWithTheModelAlreadyOnDiskShowsReadyAtOnce() throws {
        let (dir, total) = try Self.completeFolder(Self.names)
        let store = LayaModelStore(directory: dir, baseURL: Self.nowhere, files: Self.names)
        let settings = AISettingsStore(modelStore: { store })
        settings.downloadLaya()
        #expect(settings.layaState == .ready(bytes: total))
    }

    /// Old code: the only listener of the store's announcement was installed by the fixture's door table, so a settings store made on its own
    /// never heard a change. This test makes no socket read and waits only for the main queue, so it fails there.
    @MainActor @Test func aChangeOfTheModelStoreReachesTheSettingsStoreWithNoSocketRead() async throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("model-line-\(UUID().uuidString)", isDirectory: true)
        let store = LayaModelStore(directory: dir, baseURL: Self.nowhere, files: Self.names)
        let settings = AISettingsStore(modelStore: { store })
        #expect(settings.layaState == .notDownloaded)
        let (written, total) = try Self.completeFolder(Self.names)
        for f in Self.names + ["ready.json"] {
            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
            try FileManager.default.copyItem(at: written.appendingPathComponent(f), to: dir.appendingPathComponent(f))
        }
        store.refresh()   // the store changes and announces; nothing else touches the settings store
        for _ in 0..<40 where settings.layaState != .ready(bytes: total) { try await Task.sleep(nanoseconds: 50_000_000) }
        #expect(settings.layaState == .ready(bytes: total))
    }

    /// Old code: the row called `layaStateChanged()` before it answered, so a read replaced the screen's state with the store's. The settings
    /// store holds "downloading" while the shared decision store knows nothing of it; two reads must leave the held state as it was.
    /// (The new code never asks the model store on a read; the old code asked the owner's models folder, so run this on the old code in a rig copy only.)
    @MainActor @Test func readingTheDecisionRowTwiceThroughTheSocketChangesNothingOnTheScreen() throws {
        let s = AISettingsStore.shared
        let held = LayaModelState.downloading(received: 5, total: -1)
        s.layaState = held
        let row = try #require(SettingsAIMacFeature.rowSources()["mac-settings-ai.decisions.local"])
        let first = try row(AppModel.shared)
        let second = try row(AppModel.shared)
        #expect(s.layaState == held)
        #expect(first.first?["state"] as? String == "downloading")
        #expect(second.first?["state"] as? String == "downloading")
    }
}

// MARK: the search model line follows a download whoever started it

/// A slow loopback server (127.0.0.1 only), a two-file spec and a folder of its own: nothing on the owner's disk, no real network.
private struct SearchFixture {
    let server: SlowFileServer
    let spec: MlxModelSpec
    let dir: URL
    static func make() async throws -> SearchFixture {
        let small = Data("{\"model\":\"fixture\"}".utf8)
        let big = Data((0..<48_000).map { UInt8(truncatingIfNeeded: $0 &* 31) })
        let server = try SlowFileServer(files: ["config.json": small, "model.safetensors": big])
        await server.start()
        let spec = MlxModelSpec(name: "fixture", licence: "Apache-2.0", baseURL: server.base, files: [
            ModelFile(path: "config.json", bytes: Int64(small.count), sha256: SlowFileServer.hex(small)),
            ModelFile(path: "model.safetensors", bytes: Int64(big.count), sha256: SlowFileServer.hex(big)),
        ], folderName: "fixture")
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("search-line-\(UUID().uuidString)", isDirectory: true)
        return SearchFixture(server: server, spec: spec, dir: dir)
    }
    func store() -> MeaningModelStore { MeaningModelStore(rig: MeaningModelRig(spec: spec, directory: dir, blocked: nil), tickNs: 50_000_000) }
}

private final class Announcements: @unchecked Sendable {
    private let lock = NSLock(); private var n = 0; private var token: NSObjectProtocol?
    init(of store: MeaningModelStore) {
        token = NotificationCenter.default.addObserver(forName: MeaningModelStore.didChange, object: store, queue: .main) { [self] _ in lock.withLock { n += 1 } }
    }
    var count: Int { lock.withLock { n } }
    func reset() { lock.withLock { n = 0 } }
    deinit { if let t = token { NotificationCenter.default.removeObserver(t) } }
}

extension ModelLineStateTests {
    /// Old code: the line re-read the store only inside a timeline that was paused unless the line itself had started the download, so a download
    /// started elsewhere (a relaunch, another surface) left "Not on this Mac yet" on screen. The download here is started on the store, never on the line.
    /// (New names: on the old code this file does not compile, so it is red by the missing `MeaningLineModel`, not by an assertion.)
    @MainActor @Test func aSearchModelDownloadTheLineDidNotStartIsShownAsDownloadingWithRisingBytes() async throws {
        let f = try await SearchFixture.make(); defer { f.server.stop(); try? FileManager.default.removeItem(at: f.dir) }
        let store = f.store()
        let line = MeaningLineModel(store: store)
        #expect(line.state == .notDownloaded)
        let run = Task { await store.download() }
        var seen: [Int64] = []
        for _ in 0..<120 where !line.state.isReady {
            if case .downloading(let r, _) = line.state, seen.last != r { seen.append(r) }
            try await Task.sleep(nanoseconds: 25_000_000)
        }
        _ = await run.value
        #expect(seen.contains { $0 > 0 }, "the line never showed bytes: \(seen)")
        #expect(Set(seen.filter { $0 > 0 }).count >= 2, "the bytes did not rise on the line: \(seen)")
        #expect(seen == seen.sorted(), "the count only grows: \(seen)")
    }

    /// Old code: nothing announced the end to the line, which was paused. Here the line is never touched after the download starts.
    @MainActor @Test func theEndOfASearchModelDownloadShowsReadyOnTheLineWithNoFurtherAct() async throws {
        let f = try await SearchFixture.make(); defer { f.server.stop(); try? FileManager.default.removeItem(at: f.dir) }
        let store = f.store()
        let line = MeaningLineModel(store: store)
        _ = await store.download()
        for _ in 0..<40 where !line.state.isReady { try await Task.sleep(nanoseconds: 50_000_000) }
        #expect(line.state == .ready(bytes: f.spec.totalBytes))
    }

    /// Old code: a timeline woke the line twice a second while a download ran, and the store said nothing. Now the store is silent when nothing
    /// downloads: no announcement, so nothing wakes the line, before a download and after one.
    @MainActor @Test func noTimerRunsOnTheSearchModelLineWhenNothingDownloads() async throws {
        let f = try await SearchFixture.make(); defer { f.server.stop(); try? FileManager.default.removeItem(at: f.dir) }
        let store = f.store()
        let heard = Announcements(of: store)
        try await Task.sleep(nanoseconds: 200_000_000)
        heard.reset()
        try await Task.sleep(nanoseconds: 500_000_000)   // ten of the store's byte ticks, had one been running
        #expect(heard.count == 0, "the idle store announced \(heard.count) times")
        _ = await store.download()
        try await Task.sleep(nanoseconds: 200_000_000)
        heard.reset()
        try await Task.sleep(nanoseconds: 500_000_000)
        #expect(heard.count == 0, "the finished store announced \(heard.count) times")
    }
}

