import Foundation

/// Hold the key, speak, release: words appear live, then the cleaned text lands once. One session at a time.
/// The engines, the clean-up and the recorder are ports; the controller owns the order of events and the audio file's lifetime:
/// the file is removed the moment it becomes text, and on every failure path.
@MainActor
public final class DictationController {
    public private(set) var state: DictationState = .idle
    public private(set) var liveWords = ""
    public internal(set) var line = ""
    public private(set) var lastLatencyMs: Double?
    /// The part of `lastLatencyMs` the engine took (key release to the raw words), so a slow recogniser reads apart from a slow clean-up.
    public private(set) var lastTranscribeMs: Double?
    public private(set) var lastEngine = ""

    /// Where finished text goes (the composer inserts it); called once per session, never with an empty string.
    public var sink: @MainActor (String) -> Void = { _ in }
    public var onChange: @MainActor () -> Void = {}
    public var recorder: any AudioRecorder
    public var engine: () -> any Transcriber
    public var cleaner: (any TextCleaner)?
    public var store: AudioFileStore
    public var now: @Sendable () -> Double = { Double(DispatchTime.now().uptimeNanoseconds) / 1e6 }

    private var current: URL?
    private var live: LiveSession?
    private var releasedAt = 0.0

    public init(recorder: any AudioRecorder, engine: @escaping () -> any Transcriber, cleaner: (any TextCleaner)? = nil, store: AudioFileStore = AudioFileStore()) {
        self.recorder = recorder; self.engine = engine; self.cleaner = cleaner; self.store = store
    }

    /// Key down. A second press while a session runs does nothing. Returns whether a session started.
    @discardableResult
    public func begin() async -> Bool {
        guard state == .idle else { return false }
        line = ""; liveWords = ""
        guard await recorder.authorize() else { line = DictationLines.micRefused; set(.idle); return false }
        guard state == .idle else { return false }   // a second press may have arrived while the permission question was open
        do {
            let file = try store.newFile()
            current = file
            let e = engine()
            lastEngine = e.name
            live = (e as? LiveTranscriber)?.beginLive()
            let session = live
            try recorder.start(into: file) { [weak self] pcm in
                guard let session else { return }
                let words = session.feed(pcm)
                if !words.isEmpty { Task { @MainActor in self?.append(live: words) } }
            }
            set(.listening)
            return true
        } catch {
            discardAudio(); fail(error is DictationError ? error : DictationError.recorderFailed); return false
        }
    }

    /// Key up. Returns the text that was handed to the sink, if any.
    @discardableResult
    public func end() async -> String? {
        guard state == .listening else { return nil }
        releasedAt = now()
        let audio = recorder.stop() ?? current
        let tail = live?.finish() ?? ""
        live = nil
        if !tail.isEmpty { append(live: tail) }
        set(.cleaning)
        defer { if let audio { store.remove(audio) }; if let current, current != audio { store.remove(current) }; current = nil }
        do {
            guard let audio else { throw DictationError.noAudio }
            let raw = try await engine().transcribe(audio: audio)
            lastTranscribeMs = now() - releasedAt
            store.remove(audio)   // audio is text now: gone before anything else happens
            var text = raw
            if !raw.trimmingCharacters(in: .whitespaces).isEmpty, let cleaner {
                text = (try? await cleaner.clean(raw)) ?? raw   // a failing provider never loses the owner's words
            }
            text = text.trimmingCharacters(in: .whitespacesAndNewlines)
            if !text.isEmpty { sink(text) }
            lastLatencyMs = now() - releasedAt
            set(.done); set(.idle)
            return text.isEmpty ? nil : text
        } catch {
            fail(error); return nil
        }
    }

    /// `fixture reset`, and a window closing under a session: whatever runs stops, the audio goes, the words and the line clear.
    public func reset() {
        if state != .idle { _ = recorder.stop() }
        discardAudio()
        for f in store.files { store.remove(f) }
        liveWords = ""; line = ""
        set(.idle)
    }

    private func append(live words: String) { liveWords += (liveWords.isEmpty ? "" : " ") + words.trimmingCharacters(in: .whitespaces); onChange() }
    private func set(_ s: DictationState) { state = s; onChange() }
    private func discardAudio() { if let current { store.remove(current) }; current = nil; live = nil }

    private func fail(_ error: Error) {
        switch error as? DictationError {
        case .micRefused: line = DictationLines.micRefused
        case .cloudKeyMissing: line = DictationLines.cloudKeyMissing
        case .engineUnavailable: line = DictationLines.modelMissing
        case .noAudio: line = DictationLines.noAudio
        case .recorderFailed: line = DictationLines.recorderFailed
        case .transport: line = DictationLines.serviceFailed
        case .engineFailed, nil: line = DictationLines.engineFailed
        }
        _ = recorder.stop()
        discardAudio()
        set(.idle)
    }
}
