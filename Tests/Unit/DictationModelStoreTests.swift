import Foundation
import Testing
@testable import WhooshUI

// fn-dictation-ios (proof file checks/proofs/g2/e2dd9288.json): "the model file exists only after first use" and "the audio is deleted".
// Everything runs in a private folder under the temporary directory, with a fake downloader (no network) and the fixture recorder
// (no microphone: it never asks macOS for it). Each test is quick.
// MUTANTS (red-first broken copies, one per test; the exact edits are in the proof file and docs/lanes/gap-phone-b.md):
//   theModelFileIsAbsentBeforeFirstUseAndPresentAfter : ModelStore.swift `if isInstalled { return modelURL }` -> `if false { return modelURL }` (second ensure downloads again)
//   aFailedDownloadLeavesNeitherThePartNorTheModel    : ModelStore.swift the catch's `try? FileManager.default.removeItem(at: part)` removed
//   theAudioFileIsGoneOnceRemoved                     : AudioFileStore.swift `(try? FileManager.default.removeItem(at: url)) != nil` -> `false`
//   theControllerLeavesNoAudioAfterTheText           : the same AudioFileStore.swift edit (the controller has three removers on the text path, the inline one and the defer, so no
//                                                       single line of DictationController.swift turns it red; the shared floor, AudioFileStore.remove, does)
//   theControllerLeavesNoAudioAfterAFailure          : the same AudioFileStore.swift edit (the failure path also has three removers)

private func scratch(_ name: String) throws -> URL {
    let d = FileManager.default.temporaryDirectory.appendingPathComponent("whoosh-test-\(name)-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: d, withIntermediateDirectories: true)
    return d
}

private final class CountingDownloader: ModelDownloader, @unchecked Sendable {
    private let lock = NSLock()
    private var n = 0
    let failing: Bool
    init(failing: Bool = false) { self.failing = failing }
    var calls: Int { lock.withLock { n } }
    func download(from: URL, to: URL, progress: @Sendable (Double) -> Void) async throws {
        lock.withLock { n += 1 }
        // The half file a real download leaves while it runs.
        try Data("half".utf8).write(to: to)
        if failing { throw DictationError.transport("the fake download stopped") }
        progress(1)
        try Data("model".utf8).write(to: to)
    }
}

private func store(in dir: URL) -> ModelStore {
    ModelStore(directory: dir.appendingPathComponent("models", isDirectory: true), fileName: "speech.bin", source: URL(string: "http://127.0.0.1:9/speech.bin")!)
}

@Test func theModelFileIsAbsentBeforeFirstUseAndPresentAfter() async throws {
    let dir = try scratch("model"); defer { try? FileManager.default.removeItem(at: dir) }
    let s = store(in: dir), d = CountingDownloader()
    #expect(!s.isInstalled, "no model before first use")
    #expect(!FileManager.default.fileExists(atPath: s.directory.path), "the model folder is not even made before first use")
    let url = try await s.ensure(using: d)
    #expect(s.isInstalled && url == s.modelURL, "the model is there after ensure()")
    #expect(d.calls == 1)
    _ = try await s.ensure(using: d)
    #expect(d.calls == 1, "a second ensure() finds the file and does not download again")
    let names = try FileManager.default.contentsOfDirectory(atPath: s.directory.path)
    #expect(names == ["speech.bin"], "only the finished model is in the folder, no .part: \(names)")
}

@Test func aFailedDownloadLeavesNeitherThePartNorTheModel() async throws {
    let dir = try scratch("model-fail"); defer { try? FileManager.default.removeItem(at: dir) }
    let s = store(in: dir), d = CountingDownloader(failing: true)
    await #expect(throws: DictationError.self) { try await s.ensure(using: d) }
    #expect(d.calls == 1)
    #expect(!s.isInstalled, "a failed download is never taken for a model")
    let names = (try? FileManager.default.contentsOfDirectory(atPath: s.directory.path)) ?? []
    #expect(names.isEmpty, "no .part file stays behind: \(names)")
}

@Test func theAudioFileIsGoneOnceRemoved() throws {
    let dir = try scratch("audio"); defer { try? FileManager.default.removeItem(at: dir) }
    let store = AudioFileStore(directory: dir)
    #expect(store.files.isEmpty)
    let url = try store.newFile()
    try Data("RIFFxxxx".utf8).write(to: url)
    #expect(store.files.map { $0.resolvingSymlinksInPath() } == [url.resolvingSymlinksInPath()], "the recording exists while it is audio")   // /var is a link to /private/var: the folder listing returns the resolved path
    #expect(store.remove(url), "remove reports it removed the file")
    #expect(store.files.isEmpty && !FileManager.default.fileExists(atPath: url.path), "the audio is gone")
    #expect(!store.remove(url), "a second remove finds nothing and says so")
}

private struct FakeEngine: Transcriber {
    let name = "fake"
    let text: String?
    func transcribe(audio: URL) async throws -> String {
        // The audio must still be readable while the engine works.
        _ = try Data(contentsOf: audio)
        guard let text else { throw DictationError.engineFailed("fake") }
        return text
    }
}

@MainActor @Test func theControllerLeavesNoAudioAfterTheText() async throws {
    let dir = try scratch("ctl-ok"); defer { try? FileManager.default.removeItem(at: dir) }
    let audio = AudioFileStore(directory: dir)
    let c = DictationController(recorder: FixtureRecorder(), engine: { FakeEngine(text: "hello there") }, store: audio)
    var got: [String] = []
    c.sink = { got.append($0) }
    #expect(await c.begin())
    #expect(audio.files.count == 1, "the recording exists while the key is held")
    let text = await c.end()
    #expect(text == "hello there" && got == ["hello there"], "text arrives once")
    #expect(audio.files.isEmpty, "audio files 0 once the recording became text")
}

@MainActor @Test func theControllerLeavesNoAudioAfterAFailure() async throws {
    let dir = try scratch("ctl-fail"); defer { try? FileManager.default.removeItem(at: dir) }
    let audio = AudioFileStore(directory: dir)
    let c = DictationController(recorder: FixtureRecorder(), engine: { FakeEngine(text: nil) }, store: audio)
    var got: [String] = []
    c.sink = { got.append($0) }
    #expect(await c.begin())
    #expect(audio.files.count == 1)
    #expect(await c.end() == nil && got.isEmpty, "no text arrives when the engine fails")
    #expect(c.line == DictationLines.engineFailed, "the line says why")
    #expect(audio.files.isEmpty, "audio files 0 after a failure")
    #expect(c.state == .idle)
}
